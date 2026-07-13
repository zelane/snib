# snib

A thumbnail window/display picker for Wayland screen sharing, drawn as an
edge-anchored layer-shell bar. `snib` enumerates every capturable window and
output, grabs a live thumbnail of each, and lets you pick one with the mouse or
keyboard. The choice is printed to stdout, so it slots in as the chooser for
[`xdg-desktop-portal-wlr`](https://github.com/emersion/xdg-desktop-portal-wlr).

![snib](docs/screenshot.png)

## Features

- Live thumbnails of both windows and displays, captured natively over a single
  Wayland connection.
- Occlusion-correct window capture via `ext-image-copy-capture-v1` driven
  straight from `ext-foreign-toplevel-list-v1` handles.
- Keyboard-driven: fuzzy-ish substring search, vi-style navigation, and
  one-key switching between the window and display lists.
- Anchor the bar to any screen edge (top/bottom → horizontal, left/right →
  vertical).
- Catppuccin Mocha theme built in, fully restylable with your own CSS and
  ready for `matugen` / `pywal` templating.

## Requirements

- A `wlroots`-based compositor (Sway, Hyprland, river, …) that implements
  `ext-foreign-toplevel-list-v1`, `ext-image-capture-source-v1`, and
  `ext-image-copy-capture-v1`.
- GTK 4.14+ and the `gtk4-layer-shell` library.

On Arch:

```sh
sudo pacman -S gtk4 gtk4-layer-shell
```

## Installation

```sh
cargo build --release
install -Dm755 target/release/snib ~/.local/bin/snib
```

## Usage

Run `snib` and it opens the picker bar. Select a source and its formatted line
is printed to stdout:

```sh
$ snib
Window: <toplevel-identifier>
```

### Keybindings

| Key        | Action                                  |
| ---------- | --------------------------------------- |
| `w`        | Show windows                            |
| `d`        | Show displays                           |
| `h` / `l`  | Move selection left / right             |
| `/`        | Open search                             |
| `Enter`    | Confirm the focused/first match         |
| `Esc`      | Close search, or cancel and exit        |

The `w`, `d`, `h`/`l`, `/`, and cancel keys are rebindable — see
[Keybindings](#keybindings-1) under Configuration.

## Integrating with xdg-desktop-portal-wlr

`xdg-desktop-portal-wlr` runs a chooser command and reads the selected output's
name from stdout. Point it at `snib` in
`~/.config/xdg-desktop-portal-wlr/config`:

```ini
[screencast]
chooser_type=simple
chooser_cmd=snib
```

## Examples

```bash
# screenshot a specific window
grim -T $(snib -m window -f "{id}") window.png
# screenshot a specific display
grim -o $(snib -m display -f "{id}") display.png

# record the selected display
wf-recorder -o $(snib -m display -f "{id}") -f recording.mp4
```

## Configuration

Every flag has an environment-variable equivalent, so you can set defaults once
and override per-invocation.

| Flag                     | Env var             | Default        | Description                                         |
| ------------------------ | ------------------- | -------------- | --------------------------------------------------- |
| `-e, --edge <EDGE>`      | `SNIB_EDGE`         | `bottom`       | Screen edge to anchor to (`top`/`bottom`/`left`/`right`). |
| `-m, --mode <MODE>`      | `SNIB_MODE`         | `window`       | List shown on launch (`window`/`display`).          |
| `-w, --thumb-width <N>`  | `SNIB_THUMB_WIDTH`  | `320`          | Max thumbnail dimension in pixels (64–4096).        |
| `-s, --style <PATH>`     | `SNIB_STYLE`        | —              | Extra stylesheet layered over the built-in theme.   |
| `-f, --output-format <F>`| —                   | `{type}: {id}` | Line printed for the chosen source.                 |
| `-c, --extra-cmd <CMD>`  | `SNIB_EXTRA_CMD`    | —              | Command whose JSON output adds extra template fields. |

The `--output-format` string supports the placeholders `{type}`, `{id}`,
`{title}`, and `{app_id}`, plus any extra fields provided by `--extra-cmd`
(see below).

Set `SNIB_DEBUG=1` to print capture diagnostics to stderr.

### Keybindings

Navigation keys can be remapped in `~/.config/snib/config.toml`. See
[`config.example.toml`](config.example.toml) for the full set of defaults.

```toml
[keybinds]
cancel   = "Escape"  # cancel and exit
search   = "slash"   # open the search row
windows  = "w"       # show the window list
displays = "d"       # show the display list
next     = "l"       # move the selection forward
prev     = "h"       # move the selection backward
```

[gdk-keys]: https://gitlab.gnome.org/GNOME/gtk/-/blob/main/gdk/gdkkeysyms.h

## Extra template fields

To template on anything not provided by wayland itself, you can use `--extra-cmd`. `snib` runs the
command once, parses its output, and merges the fields into every source's
template, matched by foreign-toplevel identifier.

The command must print JSON shaped as `{ "<identifier>": { "<field>": value } }`:

```json
{
  "abc123": { "pid": 4242, "app_id": "firefox", "con_id": "97" }
}
```

Each inner key becomes a placeholder for use with`--output-format`.

The [`examples/`](examples/) directory has a ready-made [`sway-tree.jq`](examples/sway-tree.jq)
that reshapes `swaymsg -t get_tree` into the expected form. Copy it alongside
your other config:

```sh
mkdir -p ~/.config/snib/templates
cp examples/sway-tree.jq ~/.config/snib/templates/
```

This opens up additional wm specific functionality such as:

```sh
# Switch focus to the selected ewindow
swaymsg [con_id=$(snib -m window --extra-cmd="swaymsg -t get_tree | jq -f ~/.config/snib/templates/sway-tree.jq" -f "{con_id}")] focus

# Kill the selected window process
kill -9 $(snib -m window --extra-cmd="swaymsg -t get_tree | jq -f ~/.config/snib/templates/sway-tree.jq" -f "{pid}")
```

## Theming

The built-in [`style.css`](style.css) is compiled into the binary. To restyle,
copy it and edit, the user file is loaded at a higher priority:

```sh
mkdir -p ~/.config/snib
cp style.css ~/.config/snib/style.css
cp colors.css ~/.config/snib/colors.css
```

`snib` loads, in increasing priority: the built-in theme, then
`~/.config/snib/style.css` (or `$XDG_CONFIG_HOME/snib/style.css`), then any file
passed via `--style`.

## License

[GPL-3.0-or-later](LICENSE).
