# rexit

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)

A TUI power menu for Linux. Supports Hyprland, Sway, i3, BSPWM, and AwesomeWM with automatic detection.

Built with [Ratatui](https://github.com/ratatui/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm).

## Features

- Keyboard navigation (arrow keys, vim keys) and mouse support
- Per-action shortcut keys (e.g. `s` for Shutdown, `r` for Reboot)
- Automatic window manager detection with appropriate lock/logout commands
- 70+ background animations (matrix, rain, snow, stars, game of life, ...)
- 35+ built-in themes (catppuccin, dracula, nord, gruvbox, tokyo-night, ...)
- Fully configurable: colors, icons, text, keybindings, layout
- Four layout modes: vertical, horizontal, grid, compact
- Grace period with countdown for critical actions (shutdown/reboot)
- Nerd Font icons with emoji fallback
- Responsive layout that adapts to terminal size

## Installation

### Quick Install

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/Ninso112/rexit/main/install.sh | sh
```

### AUR (Arch Linux)

```bash
yay -S rexit-git
```

### From Source

Requires Rust 1.70+.

```bash
git clone https://github.com/Ninso112/rexit.git
cd rexit
cargo build --release
sudo cp target/release/rexit /usr/local/bin/
```

## Usage

```bash
rexit                              # Launch power menu
rexit --init                       # Generate default config
rexit --config /path/to/config.toml  # Custom config file
rexit --theme dracula              # Use a theme
rexit --list-themes                # List available themes
rexit --check-config               # Validate configuration
rexit --emoji                      # Use emoji icons (no Nerd Fonts needed)
```

### Keybindings

| Key | Action |
|-----|--------|
| `Up` / `k` | Move up |
| `Down` / `j` | Move down |
| `Enter` | Execute |
| `Esc` / `q` | Quit |
| `a` | Open animation selector |

Default action shortcuts: `s` Shutdown, `r` Reboot, `u` Suspend, `l` Lock, `o` Logout, `c` Cancel. All configurable.

### Hyprland Keybinding Example

```conf
bind = $mainMod SHIFT, E, exec, kitty --class floating -e rexit
```

## Window Manager Support

rexit auto-detects your WM and sets the right commands. You can override with `wm_type` in the config.

| WM | Detection | Lock | Logout |
|----|-----------|------|--------|
| Hyprland | `HYPRLAND_INSTANCE_SIGNATURE` | `hyprlock` | `hyprctl dispatch exit` |
| Sway | `SWAYSOCK` | `swaylock` | `swaymsg exit` |
| i3 | `XDG_SESSION_DESKTOP` | `i3lock` | `i3-msg exit` |
| BSPWM | `XDG_SESSION_DESKTOP` | — | `bspc quit` |
| AwesomeWM | `XDG_SESSION_DESKTOP` | — | `awesome-client "awesome.quit()"` |

Falls der Standard-Lockscreen nicht installiert ist, wird automatisch nach Alternativen gesucht (hyprlock, swaylock, i3lock, betterlockscreen, etc.).

## Configuration

Generate the default config:

```bash
rexit --init
# Creates ~/.config/rexit/config.toml
```

Config is loaded from `--config` flag or `$XDG_CONFIG_HOME/rexit/config.toml`.

### Overview

```toml
title = " rexit "
title_alignment = "center"
layout_mode = "vertical"     # vertical, horizontal, grid, compact
wm_type = "auto"             # auto, hyprland, sway, i3, bspwm, awesome

[border]
enabled = true
style = "rounded"            # plain, rounded, double, thick

[colors]
foreground = "white"
background = "black"
border = "cyan"
selected_fg = "black"
selected_bg = "white"
selected_modifier = ["bold"]
icon_color = "white"
help_fg = "gray"
help_key_fg = "cyan"
help_key_modifier = ["bold"]

[keys]
up = ["Up", "k"]
down = ["Down", "j"]
select = ["Enter"]
quit = ["Esc", "q"]

[animation]
enabled = true
animation_type = "matrix"
speed_ms = 80
color = "green"
density = 50

[grace_period]
enabled = true
duration_secs = 5
show_countdown = true
message_template = "{action} in {seconds}s... Press any key to cancel"

[layout]
auto_scale = true
min_width = 30
max_width = 60
padding = 1

[responsive]
enabled = true
compact_threshold = 80
minimal_threshold = 40

[performance]
auto_degrade = true
target_fps = 30

[help_text]
enabled = true
```

Colors support named values (`red`, `cyan`, `lightblue`, ...) and hex (`#RRGGBB`).

### Actions

Each action is independently configurable:

```toml
[actions.shutdown]
icon = ""           # Nerd Font icon (or emoji like "⏻")
label = "Shutdown"
command = "systemctl"
args = ["poweroff"]
enabled = true
shortcut = "s"
```

Available actions: `shutdown`, `reboot`, `suspend`, `lock`, `logout`, `cancel`.

### Animations

70+ animation types grouped by category:

- **Classic**: `matrix`, `digital_rain`, `rain`, `snow`, `stars`, `fireflies`, `bubbles`, `confetti`
- **Nature**: `aurora`, `autumn`, `butterflies`, `vine_growth`, `moss`, `spider_web`, `ocean`, `fog`
- **Fire/Energy**: `flames`, `sparks`, `lava_lamp`, `sun`, `plasma`
- **Cosmic**: `galaxy`, `meteor_shower`, `satellite`, `pulsar`, `constellation`
- **Retro**: `synthwave`, `scanlines`, `pong`, `snake`, `tetris`, `invaders`
- **Math**: `fibonacci`, `mandelbrot`, `hex_grid`, `rose`, `lissajous`, `game_of_life`
- **Technical**: `radar`, `binary_clock`, `signal`, `wifi`, `circuit`, `flow_field`
- **Artistic**: `paint_splatter`, `ink_bleed`, `mosaic`, `stained_glass`
- **Effects**: `hologram`, `glitch`, `old_film`, `thermal`, `vortex`, `smoke`
- **Other**: `wave`, `particles`, `heartbeat`, `gradient_flow`, `fish_tank`, `typing_code`, `morse`, `dna`, `ripple`, `thunder`, `none`

Use `a` during runtime to switch animations interactively.

## Themes

35+ built-in themes. List them with `rexit --list-themes`, use one with `--theme <name>`.

Set a default theme in your config:

```toml
theme = "catppuccin-mocha"
```

### Custom Themes

Create a `.toml` file in `~/.config/rexit/themes/`:

```toml
# ~/.config/rexit/themes/mytheme.toml
[colors]
foreground = "#cdd6f4"
background = "#1e1e2e"
border = "#b4befe"
selected_fg = "#1e1e2e"
selected_bg = "#b4befe"
selected_modifier = ["bold"]
icon_color = "#f38ba8"
help_fg = "#6c7086"
help_key_fg = "#89b4fa"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"

[animation]
animation_type = "matrix"
speed_ms = 80
color = "#a6e3a1"
density = 50
```

```bash
rexit --theme mytheme
```

## Easter Eggs

Try the Konami code while rexit is running.

## Building

```bash
cargo build --release
cargo test
cargo clippy
cargo fmt
```

## Contributing

Contributions welcome. For major changes, open an issue first.

## License

[GPL-3.0](LICENSE)

## Author

[Ninso112](https://github.com/Ninso112)
