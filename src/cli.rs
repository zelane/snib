//! Command-line surface: flags, the screen edge to anchor to, and the two
//! kinds of capturable source.

use std::path::PathBuf;
use std::sync::OnceLock;

use clap::{Parser, ValueEnum, Subcommand};
use clap_complete::{Shell};

#[derive(Parser)]
#[command(
    name = "snib",
    version,
    about = "Thumbnail window/display picker for xdg-desktop-portal-wlr"
)]
pub struct Cli {
    /// Screen edge to anchor the picker bar to.
    #[arg(short, long, value_enum, default_value = "bottom", env = "SNIB_EDGE")]
    pub edge: Side,

    /// Source list to show on launch.
    #[arg(short, long, value_enum, default_value = "window", env = "SNIB_MODE")]
    pub mode: Kind,

    /// Maximum thumbnail dimension, in pixels.
    #[arg(
        short = 'w',
        long,
        default_value_t = 320,
        value_parser = clap::value_parser!(u32).range(64..=4096),
        env = "SNIB_THUMB_WIDTH"
    )]
    pub thumb_width: u32,

    /// Extra stylesheet to layer on top of the built-in theme.
    #[arg(short, long, value_name = "PATH", env = "SNIB_STYLE")]
    pub style: Option<PathBuf>,

    /// Line printed to stdout for the chosen source. Placeholders:
    /// {type}, {id}, {title}, {app_id}.
    #[arg(short = 'f', long, default_value = "{type}: {id}")]
    pub output_format: String,

    /// A command to run, that returns extra template fields as a json object keyed by foreign_toplevel_id
    #[arg(short = 'c', long = "extra-cmd", env = "SNIB_EXTRA_CMD")]
    pub extra_cmd: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}


pub fn cli() -> &'static Cli {
    static CLI: OnceLock<Cli> = OnceLock::new();
    CLI.get_or_init(Cli::parse)
}

// --- edges ---

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    pub fn horizontal_bar(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Side::Top => "edge-top",
            Side::Bottom => "edge-bottom",
            Side::Left => "edge-left",
            Side::Right => "edge-right",
        }
    }
}

// --- sources ---

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Kind {
    Window,
    Display,
}

impl Kind {
    /// Value substituted for the `{type}` placeholder.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Window => "Window",
            // xdg-desktop-portal-wlr expects "Monitor", not "Display".
            Kind::Display => "Monitor",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Kind::Window => "Select a window to share",
            Kind::Display => "Select a display to share",
        }
    }
}
