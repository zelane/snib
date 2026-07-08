//! snib — a Catppuccin-themed thumbnail window/display chooser for
//! xdg-desktop-portal-wlr, drawn as an edge-anchored layer-shell bar.

use relm4::RelmApp;

mod capture;
mod cli;
mod source;
mod ui;

fn main() {
    let cli = cli::cli();
    let mut extras = cli
        .extra_cmd
        .as_deref()
        .map(source::fetch_extras)
        .unwrap_or_default();

    let mut sources = capture::capture_thumbnails(cli.thumb_width, cli.edge.horizontal_bar());
    for s in &mut sources {
        if let Some(extra) = extras.remove(&s.identifier) {
            s.extra = extra;
        }
    }

    let app = RelmApp::new("com.zelane.snib");
    app.allow_multiple_instances(true);
    app.with_args(Vec::new())
        .run::<ui::App>((cli.output_format.clone(), sources));
}
