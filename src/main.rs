//! snib — a Catppuccin-themed thumbnail window/display chooser for
//! xdg-desktop-portal-wlr, drawn as an edge-anchored layer-shell bar.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use clap::{Parser, ValueEnum};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use relm4::gtk::{self, gdk, glib, pango, prelude::*};
use relm4::prelude::*;
use strfmt::strfmt;

mod capture;

#[derive(Parser)]
#[command(
    name = "snib",
    version,
    about = "Thumbnail window/display picker for xdg-desktop-portal-wlr"
)]
struct Cli {
    /// Screen edge to anchor the picker bar to.
    #[arg(short, long, value_enum, default_value = "bottom", env = "SNIB_EDGE")]
    edge: Side,

    /// Source list to show on launch.
    #[arg(short, long, value_enum, default_value = "window", env = "SNIB_MODE")]
    mode: Kind,

    /// Maximum thumbnail dimension, in pixels.
    #[arg(
        short = 'w',
        long,
        default_value_t = 320,
        value_parser = clap::value_parser!(u32).range(64..=4096),
        env = "SNIB_THUMB_WIDTH"
    )]
    thumb_width: u32,

    /// Extra stylesheet to layer on top of the built-in theme.
    #[arg(short, long, value_name = "PATH", env = "SNIB_STYLE")]
    style: Option<PathBuf>,

    /// Line printed to stdout for the chosen source. Placeholders:
    /// {type}, {id}, {caption}, {app_id}.
    #[arg(short = 'f', long, default_value = "{type}: {id}")]
    output_format: String,
}

fn cli() -> &'static Cli {
    static CLI: OnceLock<Cli> = OnceLock::new();
    CLI.get_or_init(Cli::parse)
}

// --- edges ---

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    fn horizontal_bar(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }

    fn css_class(self) -> &'static str {
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
enum Kind {
    Window,
    Display,
}

struct Thumb {
    kind: Kind,
    identifier: String,
    app_id: String,
    caption: String,
    haystack: String,
    size: [u32; 2],
    rgba: Vec<u8>,
}

fn user_css_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|base| base.join("snib/style.css"))
}

fn load_css(display: &gdk::Display) {
    let base = gtk::CssProvider::new();
    base.load_from_data(include_str!("../style.css"));
    gtk::style_context_add_provider_for_display(
        display,
        &base,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let extra = cli().style.clone().or_else(user_css_path);
    if let Some(path) = extra.filter(|p| p.exists()) {
        let user = gtk::CssProvider::new();
        user.load_from_path(&path);
        gtk::style_context_add_provider_for_display(
            display,
            &user,
            gtk::STYLE_PROVIDER_PRIORITY_USER,
        );
    }
}

// --- app ---

struct Entry {
    kind: Kind,
    haystack: String,
    button: gtk::Button,
}

struct App {
    mode: Kind,
    query: String,
    entries: Vec<Entry>,
    search: gtk::Text,
    search_row: gtk::Box,
    title: gtk::Label,
}

impl App {
    fn matches(&self, e: &Entry) -> bool {
        e.kind == self.mode && (self.query.is_empty() || e.haystack.contains(&self.query))
    }

    fn apply_filter(&self) {
        for e in &self.entries {
            e.button.set_visible(self.matches(e));
        }
    }

    fn focus_first(&self) {
        if let Some(e) = self.entries.iter().find(|e| self.matches(e)) {
            e.button.grab_focus();
        }
    }

    fn set_mode(&mut self, mode: Kind) {
        self.mode = mode;
        self.title.set_text(match mode {
            Kind::Window => "Select a window to share",
            Kind::Display => "Select a display to share",
        });
        self.apply_filter();
        if !self.search_row.get_visible() {
            self.focus_first();
        }
    }
}

#[derive(Debug)]
enum Msg {
    Pick(String),
    Cancel,
    SetMode(Kind),
    OpenSearch,
    Query(String),
    CloseSearch,
    ActivateFirst,
}

impl SimpleComponent for App {
    type Init = (String, Vec<Thumb>);
    type Input = Msg;
    type Output = ();
    type Root = gtk::Window;
    type Widgets = ();

    fn init_root() -> Self::Root {
        let window = gtk::Window::builder().css_classes(["snib"]).build();
        let side = cli().edge;

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.add_css_class(side.css_class());

        let (edge, span_a, span_b) = match side {
            Side::Bottom => (Edge::Bottom, Edge::Left, Edge::Right),
            Side::Top => (Edge::Top, Edge::Left, Edge::Right),
            Side::Left => (Edge::Left, Edge::Top, Edge::Bottom),
            Side::Right => (Edge::Right, Edge::Top, Edge::Bottom),
        };

        window.set_anchor(edge, true);
        window.set_anchor(span_a, true);
        window.set_anchor(span_b, true);

        window
    }

    fn init(
        args: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        load_css(&WidgetExt::display(&root));

        let (template, thumbs) = args;
        let horizontal = cli().edge.horizontal_bar();

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[strong] sender,
            move |ctrl, key, _code, _mods| {
                match key {
                    gdk::Key::Escape => {
                        sender.input(Msg::Cancel);
                        glib::Propagation::Stop
                    }
                    gdk::Key::slash => {
                        sender.input(Msg::OpenSearch);
                        glib::Propagation::Stop
                    }
                    gdk::Key::w => {
                        sender.input(Msg::SetMode(Kind::Window));
                        glib::Propagation::Stop
                    }
                    gdk::Key::d => {
                        sender.input(Msg::SetMode(Kind::Display));
                        glib::Propagation::Stop
                    }
                    gdk::Key::l => {
                        if let Some(w) = ctrl.widget() {
                            w.child_focus(gtk::DirectionType::TabForward);
                        }
                        glib::Propagation::Stop
                    }
                    gdk::Key::h => {
                        if let Some(w) = ctrl.widget() {
                            w.child_focus(gtk::DirectionType::TabBackward);
                        }
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            }
        ));
        root.add_controller(keys);

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let title = gtk::Label::new(None);
        title.add_css_class("title");
        title.set_halign(gtk::Align::Start);
        vbox.append(&title);

        let search = gtk::Text::new();
        search.add_css_class("search-field");
        search.set_hexpand(true);

        search.connect_changed(glib::clone!(
            #[strong] sender,
            move |e| sender.input(Msg::Query(e.text().to_string()))
        ));
        search.connect_activate(glib::clone!(
            #[strong] sender,
            move |_| sender.input(Msg::ActivateFirst)
        ));

        let esc = gtk::EventControllerKey::new();
        esc.connect_key_pressed(glib::clone!(
            #[strong] sender,
            move |_, key, _code, _mods| match key {
                gdk::Key::Escape => {
                    sender.input(Msg::CloseSearch);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        ));
        search.add_controller(esc);

        let prompt = gtk::Label::new(Some("/"));
        prompt.add_css_class("search-prompt");

        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        search_row.add_css_class("search");
        search_row.set_visible(false);
        search_row.append(&prompt);
        search_row.append(&search);
        vbox.append(&search_row);

        let mut entries = Vec::with_capacity(thumbs.len());

        if thumbs.is_empty() {
            let empty = gtk::Label::new(Some("No capturable windows found."));
            empty.add_css_class("empty");
            vbox.append(&empty);
            root.set_child(Some(&vbox));
        } else {
            let orientation = if horizontal {
                gtk::Orientation::Horizontal
            } else {
                gtk::Orientation::Vertical
            };
            let strip = gtk::Box::new(orientation, 0);

            for thumb in thumbs {
                let [w, h] = thumb.size;
                let bytes = glib::Bytes::from_owned(thumb.rgba);
                let texture = gdk::MemoryTexture::new(
                    w as i32,
                    h as i32,
                    gdk::MemoryFormat::R8g8b8a8,
                    &bytes,
                    (w * 4) as usize,
                );

                let picture = gtk::Picture::for_paintable(&texture);
                picture.add_css_class("thumb-image");
                picture.set_can_shrink(false);

                let caption = gtk::Label::new(Some(&thumb.caption));
                caption.add_css_class("caption");
                caption.set_ellipsize(pango::EllipsizeMode::End);
                caption.set_max_width_chars(30);

                let cell = gtk::Box::new(gtk::Orientation::Vertical, 0);
                cell.append(&caption);
                cell.append(&picture);

                let vars = HashMap::from([
                    ("id".to_string(), thumb.identifier.clone()),
                    ("caption".to_string(), thumb.caption.clone()),
                    ("app_id".to_string(), thumb.app_id.to_string()),
                    (
                        "type".to_string(),
                        match thumb.kind {
                            Kind::Window => "Window",
                            Kind::Display => "Monitor",
                        }
                        .to_string(),
                    ),
                ]);
                let line = strfmt(&template, &vars).unwrap();

                let button = gtk::Button::new();
                button.add_css_class("thumb");
                button.set_child(Some(&cell));
                button.set_valign(gtk::Align::Start);
                button.connect_clicked(glib::clone!(
                    #[strong] sender,
                    move |_| sender.input(Msg::Pick(line.clone()))
                ));

                entries.push(Entry {
                    kind: thumb.kind,
                    haystack: thumb.haystack,
                    button: button.clone(),
                });
                strip.append(&button);
            }

            let viewport = gtk::Viewport::builder()
                .scroll_to_focus(true)
                .child(&strip)
                .build();

            let mut scrolled = gtk::ScrolledWindow::builder().child(&viewport);
            if horizontal {
                scrolled = scrolled
                    .hscrollbar_policy(gtk::PolicyType::Automatic)
                    .vscrollbar_policy(gtk::PolicyType::Never)
                    .propagate_natural_height(true)
                    .hexpand(true);
            } else {
                scrolled = scrolled
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .propagate_natural_width(true)
                    .vexpand(true);
            }
            vbox.append(&scrolled.build());
            root.set_child(Some(&vbox));
        }

        let mut model = App {
            mode: Kind::Window,
            query: String::new(),
            entries,
            search,
            search_row,
            title,
        };
        model.set_mode(cli().mode);

        ComponentParts { model, widgets: () }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::Pick(line) => {
                println!("{line}");
                std::process::exit(0);
            }
            Msg::Cancel => std::process::exit(0),
            Msg::SetMode(mode) => self.set_mode(mode),
            Msg::OpenSearch => {
                self.search_row.set_visible(true);
                self.search.grab_focus();
            }
            Msg::Query(q) => {
                self.query = q.to_lowercase();
                self.apply_filter();
            }
            Msg::CloseSearch => {
                self.search.set_text("");
                self.search_row.set_visible(false);
                self.query.clear();
                self.apply_filter();
                self.focus_first();
            }
            Msg::ActivateFirst => {
                self.focus_first();
            }
        }
    }
}

fn main() {
    let cli = cli();

    let thumbs: Vec<Thumb> = capture::capture_thumbnails(cli.thumb_width, cli.edge.horizontal_bar())
        .into_iter()
        .map(|c| Thumb {
            kind: if c.kind == "Window" { Kind::Window } else { Kind::Display },
            app_id: c.app_id,
            identifier: c.identifier,
            caption: c.caption,
            haystack: c.haystack,
            size: [c.width, c.height],
            rgba: c.rgba,
        })
        .collect();

    let app = RelmApp::new("com.zelane.snib");
    app.allow_multiple_instances(true);
    app.with_args(Vec::new()).run::<App>((cli.output_format.clone(), thumbs));
}