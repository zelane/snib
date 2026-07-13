# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `examples/colors.css.noctalia`: a noctalia template for regenerating the palette from your active color scheme.

### Changed
- Moved the palette out of `style.css` into a separate `colors.css` (imported via `@import`), which is now the matugen / pywal / noctalia templating target. Copy `colors.css` alongside `style.css` when overriding in `~/.config/snib/`.
- Renamed the title CSS class from `.title` to `.heading` (affects custom stylesheets targeting `.snib .title`).

### Fixed
- Corrected the crate `edition` from the invalid `2026` to `2024`.

## [0.4.0] - 2026-07-10

### Added
- Config file support via `~/.config/snib/config.toml` (see `config.example.toml`).
- Rebindable keybindings through a `[keybinds]` section: `cancel`, `search`, `windows`, `displays`, `next`, and `prev`.
- `-c, --extra-cmd` / `SNIB_EXTRA_CMD`: merge fields from a command's JSON output (keyed by foreign-toplevel identifier) into every source's template, exposing data such as `pid`, `app_id`, and `con_id` to `--output-format`.
- Example jq templates: `examples/sway-tree.jq` and `examples/hyprland-clients.jq`.
- Shell completions.

### Changed
- Renamed the `{caption}` template placeholder to `{title}` (breaking).

### Fixed
- Corrected display/monitor `kind` reporting for the xdg portal path.

### Internal
- Split `src/capture.rs` into a `capture/` module (`mod.rs`, `protocol.rs`, `image.rs`).
- Extracted CLI, config, source, and UI logic out of `main.rs` into new `cli.rs`, `config.rs`, `source.rs`, and `ui.rs` modules.
