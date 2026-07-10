//! Optional user configuration loaded from `~/.config/snib/config.toml`.
//!
//! The file only ever *overrides* the built-in defaults, so a missing or
//! partial config is fine — every field falls back to the same value the code
//! shipped with.

use std::path::PathBuf;
use std::sync::OnceLock;

use relm4::gtk::gdk;
use relm4::gtk::glib::translate::FromGlib;
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub keybinds: Keybinds,
}

/// Each action maps to a key *name*. Names are matched with
/// [`gdk::Key::from_name`] (e.g. `Escape`, `slash`, `w`, `Left`); a bare
/// single character (e.g. `/`) is accepted too. An unparseable name simply
/// disables that binding.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keybinds {
    /// Cancel and exit.
    pub cancel: String,
    /// Open the search row.
    pub search: String,
    /// Show the window list.
    pub windows: String,
    /// Show the display list.
    pub displays: String,
    /// Move the selection forward (toward the end of the strip).
    pub next: String,
    /// Move the selection backward.
    pub prev: String,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            cancel: "Escape".into(),
            search: "slash".into(),
            windows: "w".into(),
            displays: "d".into(),
            next: "l".into(),
            prev: "h".into(),
        }
    }
}

/// Resolve a configured key name to a `gdk::Key`, or `None` if it can't be
/// parsed (which leaves the binding inactive).
pub fn parse_key(name: &str) -> Option<gdk::Key> {
    gdk::Key::from_name(name).or_else(|| {
        let mut chars = name.chars();
        match (chars.next(), chars.next()) {
            // A bare character (e.g. `/`) -> the keyval it produces.
            (Some(c), None) => {
                let keyval = gdk::unicode_to_keyval(c as u32);
                Some(unsafe { gdk::Key::from_glib(keyval) })
            }
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_keeps_defaults() {
        let c: Config = toml::from_str("[keybinds]\ncancel = \"q\"\n").unwrap();
        assert_eq!(c.keybinds.cancel, "q"); // overridden
        assert_eq!(c.keybinds.windows, "w"); // default preserved
        assert_eq!(c.keybinds.prev, "h");
    }

    #[test]
    fn empty_config_is_all_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.keybinds.search, "slash");
    }

    #[test]
    fn unknown_field_is_rejected() {
        assert!(toml::from_str::<Config>("[keybinds]\nbogus = \"x\"\n").is_err());
    }
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .map(|base| base.join("snib/config.toml"))
}

pub fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let Some(path) = config_path().filter(|p| p.exists()) else {
            return Config::default();
        };
        match std::fs::read_to_string(&path).map(|s| toml::from_str::<Config>(&s)) {
            Ok(Ok(config)) => config,
            Ok(Err(e)) => {
                eprintln!("snib: ignoring {}: {e}", path.display());
                Config::default()
            }
            Err(e) => {
                eprintln!("snib: ignoring {}: {e}", path.display());
                Config::default()
            }
        }
    })
}
