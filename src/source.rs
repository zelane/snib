//! A capturable source and the template fields rendered for it.

use std::collections::HashMap;
use std::process::Command;

use serde_json::Value;
use strfmt::strfmt;

use crate::cli::Kind;

/// Extra template fields for one source, as returned by `--extra-cmd`.
pub type Extra = Vec<(String, String)>;
/// Extra fields for every source, keyed by foreign-toplevel identifier.
pub type Extras = HashMap<String, Extra>;

/// One capturable window or display, with its thumbnail already scaled.
pub struct Source {
    pub kind: Kind,
    pub identifier: String,
    pub app_id: String,
    pub title: String,
    /// Lowercased title + app_id, matched against the search query.
    pub haystack: String,
    pub size: [u32; 2],
    pub rgba: Vec<u8>,
    pub extra: Extra,
}

impl Source {
    pub fn render_line(&self, template: &str) -> String {
        let vars: HashMap<_, _> = self
            .extra
            .iter()
            .cloned()
            .chain([
                ("type".into(), self.kind.label().to_string()),
                ("id".into(), self.identifier.clone()),
                ("title".into(), self.title.clone()),
                ("app_id".into(), self.app_id.clone()),
            ])
            .collect();
        strfmt(template, &vars).unwrap_or_default()
    }
}

/// Run `cmd` and parse its stdout as `{ identifier: { field: value } }`.
pub fn fetch_extras(cmd: &str) -> Extras {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .output()
        .expect("failed to execute process");

    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Extras::new();
    }

    let raw: HashMap<String, HashMap<String, Value>> =
        serde_json::from_slice(&output.stdout).unwrap();

    raw.into_iter()
        .map(|(k, v)| {
            let sub: Extra = v.into_iter().map(|(k2, v2)| (k2, v2.to_string())).collect();
            (k, sub)
        })
        .collect()
}
