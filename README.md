# rexit

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org/)
[![Hyprland](https://img.shields.io/badge/Hyprland-compatible-green.svg)](https://hyprland.org/)
[![Sway](https://img.shields.io/badge/Sway-compatible-blue.svg)](https://swaywm.org/)
[![i3](https://img.shields.io/badge/i3-compatible-orange.svg)](https://i3wm.org/)

A **rice-ready** TUI (Text User Interface) power menu for Linux, supporting multiple window managers including Hyprland, Sway, i3, BSPWM, and AwesomeWM.

## Description

`rexit` is a lightweight, terminal-based power menu designed for Linux systems, with special optimizations for Hyprland window manager users. It provides a clean interface to quickly execute common power management commands without leaving your terminal.

### Features

- 🖥️ **Clean TUI Interface** - Built with Ratatui for a beautiful terminal experience
- ⌨️ **Keyboard Navigation** - Navigate with Arrow keys or Vim keys (j/k)
- 🔑 **Action Shortcuts** - Each action has its own configurable shortcut (e.g., `s` for Shutdown, `r` for Reboot)
- 🚀 **Fast & Lightweight** - Low resource usage, instant startup
- 🎯 **Focused Functionality** - Six essential power options in one place
- 🔒 **Multi-WM Support** - Native support for Hyprland, Sway, i3, BSPWM, and AwesomeWM with automatic detection
- 📐 **Flexible Layouts** - Vertical, horizontal, grid, and compact layout modes
- ⚡ **Zero Configuration** - Works out of the box
- 🎨 **Fully Riceable** - Customize everything: colors, icons, text, keybindings, layout
- ✨ **Background Animations** - Matrix rain, thunderstorm, snow, stars, fireflies, bubbles, confetti, waves, and particles
- 🥚 **Easter Eggs** - Hidden surprises like the Konami code (try it!)

## Installation

### Quick Install

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/Ninso112/rexit/main/install.sh | sh
```

This will download and install `rexit` using Cargo.

### Prerequisites

- Rust 1.70 or higher
- systemd (for power management)
- hyprlock (for lock functionality)
- hyprctl (for logout functionality)

### From Source

```bash
# Clone the repository
git clone https://github.com/Ninso112/rexit.git
cd rexit

# Build in release mode
cargo build --release

# Install to your local bin directory
sudo cp target/release/rexit /usr/local/bin/
# or for user-only installation
cp target/release/rexit ~/.local/bin/
```

### Using Cargo

```bash
cargo install --git https://github.com/Ninso112/rexit
```

## Usage

Launch `rexit` from your terminal:

```bash
rexit
```

### Command Line Options

```bash
# Generate default configuration file
rexit --init

# Use custom config file
rexit --config /path/to/config.toml
```

### Keyboard Controls (Default)

| Key           | Action                      |
|---------------|-----------------------------|
| `↑` / `k`     | Move selection up           |
| `↓` / `j`     | Move selection down         |
| `Enter`       | Execute selected command    |
| `Esc` / `q`   | Cancel and exit             |

### Action Shortcuts

Each action can have its own shortcut key. The default shortcuts are:

| Action   | Shortcut |
|----------|----------|
| Shutdown | `s`      |
| Reboot   | `r`      |
| Suspend  | `u`      |
| Lock     | `l`      |
| Logout   | `o`      |
| Cancel   | `c`      |

Shortcuts are configurable in the config file.

### Hidden Features

| Key | Action |
|-----|--------|
| `a` | Open animation selector menu |

The animation menu allows you to switch between all available background animations in real-time without restarting the application.

### Hyprland Keybinding

Add this to your Hyprland configuration (`~/.config/hypr/hyprland.conf`):

```conf
# Power menu
bind = $mainMod SHIFT, E, exec, kitty --class floating -e rexit
```

Or for any terminal:

```conf
bind = $mainMod SHIFT, E, exec, alacritty -e rexit
```

## Ricing / Configuration

`rexit` is designed to be fully customizable. Everything can be configured through a TOML configuration file.

### Layout Modes

`rexit` supports multiple layout modes to fit your preferences:

| Mode       | Description                                           |
|------------|-------------------------------------------------------|
| `vertical` | Default vertical list layout                          |
| `horizontal` | Horizontal layout with icons and labels side-by-side |
| `grid`     | 2-column grid layout                                  |
| `compact`  | Clean layout showing only icons                       |

Set the layout mode in your config:
```toml
layout_mode = "grid"  # Options: "vertical", "horizontal", "grid", "compact"
```

### Window Manager Auto-Detection

`rexit` automatically detects your window manager and sets the appropriate logout command. Supported WMs:

| WM        | Detection Method                           | Logout Command                         |
|-----------|-------------------------------------------|----------------------------------------|
| Hyprland  | `HYPRLAND_INSTANCE_SIGNATURE` env var     | `hyprctl dispatch exit`                |
| Sway      | `SWAYSOCK` env var                        | `swaymsg exit`                         |
| i3        | `XDG_SESSION_DESKTOP`                     | `i3-msg exit`                          |
| BSPWM     | `XDG_SESSION_DESKTOP`                     | `bspc quit`                            |
| AwesomeWM | `XDG_SESSION_DESKTOP`                     | `awesome-client "awesome.quit()"`      |

You can also manually set your WM:
```toml
wm_type = "sway"  # Options: "auto", "hyprland", "sway", "i3", "bspwm", "awesome"
```

### Creating a Config File

Generate the default configuration:

```bash
rexit --init
```

This creates `~/.config/rexit/config.toml` with all default values commented.

### Config Location

`rexit` looks for configuration in the following order:
1. Path specified with `--config`
2. `$XDG_CONFIG_HOME/rexit/config.toml` (usually `~/.config/rexit/config.toml`)

### Configuration Options

#### Background Animations

`rexit` supports animated backgrounds in the empty space around the menu. The Matrix animation is enabled by default.

```toml
[animation]
enabled = true          # Enable/disable background animation
animation_type = "matrix"  # Options: see table below
speed_ms = 80           # Animation speed in milliseconds (lower = faster)
color = "green"         # Animation color (for single-color animations)
density = 50            # Particle density (0-100, higher = more particles)
```

**Available Animation Types:**

| Animation | Description |
|-----------|-------------|
| `matrix` | Classic Matrix digital rain effect with Japanese katakana characters |
| `rain` | Gentle rain falling from top to bottom |
| `thunder` | Dark stormy background with random lightning flashes |
| `snow` | Gentle snowfall with drifting flakes |
| `stars` | Twinkling stars in the night sky |
| `fireflies` | Glowing fireflies drifting around the screen |
| `bubbles` | Rising bubbles with wobble effect |
| `confetti` | Falling colorful confetti shapes |
| `wave` | Sine wave patterns across the screen |
| `particles` | Floating particles that bounce around |
| `digital_rain` | Binary/hexadecimal falling characters (Matrix-style) |
| `heartbeat` | Pulsing heartbeat rhythm effect |
| `plasma` | Liquid plasma color blobs |
| `scanlines` | Retro CRT monitor scanlines with occasional glitch |
| `aurora` | Aurora borealis (northern lights) wave effect |
| `autumn` | Falling autumn leaves |
| `dna` | Rotating DNA double helix |
| `synthwave` | Retro 80s synthwave grid with sun |
| `smoke` | Rising smoke particles |
| `gradient_flow` | Flowing rainbow gradients |
| `constellation` | Connected nodes forming constellation patterns |
| `fish_tank` | Swimming fish with bubbles |
| `typing_code` | Rust code being typed in real-time |
| `vortex` | Spiraling vortex tunnel effect |
| `circuit` | Electronic circuit board traces |
| `flow_field` | Perlin noise flow field particles |
| `morse` | GNU/Linux copypasta in Morse code |
| `lissajous` | Mathematical Lissajous curves |
| `game_of_life` | Conway's Game of Life simulation |
| `none` | No animation (static background) |

**Easter Egg 🥚:** Try entering the Konami code (↑↑↓↓←→←→BA) while `rexit` is running to activate rainbow mode for compatible animations!

#### Window Title and Layout

```toml
title = " rexit "
title_alignment = "center"  # Options: "left", "center", "right"

## Layout mode: "vertical", "horizontal", "grid", "compact"
layout_mode = "vertical"

## Window manager type: "auto", "hyprland", "sway", "i3", "bspwm", "awesome"
wm_type = "auto"
```

#### Border Style

```toml
[border]
enabled = true
style = "rounded"  # Options: "plain", "rounded", "double", "thick"
```

#### Colors

All colors support:
- Named colors: `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `gray`, `white`
- Light variants: `lightred`, `lightgreen`, `lightyellow`, `lightblue`, `lightmagenta`, `lightcyan`
- Dark variants: `darkgray`
- Hex colors: `#RRGGBB` (e.g., `#ff0000`, `#1a1a2e`)

```toml
[colors]
foreground = "white"
background = "black"
border = "cyan"
selected_fg = "black"
selected_bg = "white"
selected_modifier = ["bold"]  # Options: bold, italic, underlined, slowblink, rapidblink, reversed, hidden, crossedout
icon_color = "white"
help_fg = "gray"
help_key_fg = "cyan"
help_key_modifier = ["bold"]
```

#### Keybindings

```toml
[keys]
# Key names: Use crossterm KeyCode names
# Examples: "q", "Esc", "Enter", "Up", "Down", "Left", "Right", "Tab", "Backspace"
# Modifiers can be added with format: "Ctrl-q", "Alt-q", "Shift-Up"
up = ["Up", "k"]
down = ["Down", "j"]
select = ["Enter"]
quit = ["Esc", "q"]
```

#### Actions

Each action can be customized with its own icon, label, and command:

```toml
[actions.shutdown]
icon = "⏻"
label = "Shutdown"
command = "systemctl"
args = ["poweroff"]
enabled = true
```

**Note on Icons**: Icons can be specified as:
- Direct Unicode characters: `icon = "⏻"`
- Unicode escape sequences: `icon = "\u{23FB}"` (useful for special characters)
- Nerd Fonts icons (e.g., `󰐥`) if your terminal uses a Nerd Font

```toml
[actions.shutdown]
icon = "⏻"
label = "Shutdown"
command = "systemctl"
args = ["poweroff"]
enabled = true

[actions.reboot]
icon = "↻"
label = "Reboot"
command = "systemctl"
args = ["reboot"]
enabled = true

[actions.suspend]
icon = "⏾"
label = "Suspend"
command = "systemctl"
args = ["suspend"]
enabled = true

[actions.lock]
icon = "🔒"
label = "Lock"
command = "hyprlock"
args = []
enabled = true

[actions.logout]
icon = "⇥"
label = "Logout"
command = "hyprctl"
args = ["dispatch", "exit"]
enabled = true

[actions.cancel]
icon = "✕"
label = "Cancel"
command = ""  # Empty command just exits
args = []
enabled = true
```

#### Help Text

```toml
[help_text]
enabled = true
template = "{keys} {action} | "
separator = " | "
```

#### Layout

```toml
[layout]
auto_scale = true         # Automatically scale menu to fit content
vertical_margin = 30      # Percentage of vertical space around the menu (when auto_scale = false)
horizontal_margin = 30    # Percentage of horizontal space around the menu (when auto_scale = false)
min_width = 30           # Minimum width of the menu
min_height = 10          # Minimum height of the menu
max_width = 60           # Maximum width when auto_scale is enabled (0 = unlimited)
padding = 1              # Padding inside the menu box
```

#### Layout Modes

**Auto-Scale Mode (Default)** - Menu automatically sizes to fit content:
```toml
[layout]
auto_scale = true
min_width = 30           # Minimum width
max_width = 60           # Maximum width (prevents overly wide menu)
padding = 1              # Inner padding
```

**Fixed Mode** - Use percentage-based margins:
```toml
[layout]
auto_scale = false
vertical_margin = 30     # 30% of terminal height as margin
horizontal_margin = 30   # 30% of terminal width as margin
min_width = 30
min_height = 10
```

### Example Rices

#### Matrix Theme (Default)

The default Matrix theme with green digital rain:

```toml
title = " rexit "
title_alignment = "center"
layout_mode = "vertical"

[colors]
foreground = "#00ff41"
background = "black"
border = "#00ff41"
selected_fg = "black"
selected_bg = "#00ff41"
selected_modifier = ["bold"]
icon_color = "#00ff41"
help_fg = "#008f11"
help_key_fg = "#00ff41"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"

[animation]
enabled = true
animation_type = "matrix"
speed_ms = 80
color = "green"
density = 50
```

#### Thunderstorm Theme

Dark and moody with lightning flashes:

```toml
title = " rexit "
title_alignment = "center"
layout_mode = "vertical"

[colors]
foreground = "#d4d4d4"
background = "#0a0a0f"
border = "#4a4a5a"
selected_fg = "#0a0a0f"
selected_bg = "#6a6a8a"
selected_modifier = ["bold"]
icon_color = "#8a8aaa"
help_fg = "#3a3a4a"
help_key_fg = "#5a5a7a"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"

[animation]
enabled = true
animation_type = "thunder"
speed_ms = 100
color = "white"
density = 30
```

#### Winter Theme

Peaceful snowfall:

```toml
title = " rexit "
title_alignment = "center"
layout_mode = "vertical"

[colors]
foreground = "#e0f7fa"
background = "#001529"
border = "#81d4fa"
selected_fg = "#001529"
selected_bg = "#81d4fa"
selected_modifier = ["bold"]
icon_color = "#b3e5fc"
help_fg = "#4a6572"
help_key_fg = "#81d4fa"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"

[animation]
enabled = true
animation_type = "snow"
speed_ms = 120
color = "white"
density = 40
```

#### Starry Night Theme

Twinkling stars in a peaceful night:

```toml
title = " rexit "
title_alignment = "center"
layout_mode = "vertical"

[colors]
foreground = "#f5f5f5"
background = "#0d1b2a"
border = "#778da9"
selected_fg = "#0d1b2a"
selected_bg = "#e0e1dd"
selected_modifier = ["bold"]
icon_color = "#e0e1dd"
help_fg = "#415a77"
help_key_fg = "#778da9"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"

[animation]
enabled = true
animation_type = "stars"
speed_ms = 150
color = "yellow"
density = 30
```

#### Fireflies Theme

Warm summer evening atmosphere:

```toml
title = " rexit "
title_alignment = "center"
layout_mode = "vertical"

[colors]
foreground = "#f4e4c1"
background = "#1a1a2e"
border = "#e9c46a"
selected_fg = "#1a1a2e"
selected_bg = "#e9c46a"
selected_modifier = ["bold"]
icon_color = "#f4a261"
help_fg = "#6b5b4f"
help_key_fg = "#e9c46a"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"

[animation]
enabled = true
animation_type = "fireflies"
speed_ms = 100
color = "yellow"
density = 20
```

#### Dracula Theme

```toml
title = "  󰐥 Power Menu  "
title_alignment = "center"
layout_mode = "vertical"

[colors]
foreground = "#f8f8f2"
background = "#282a36"
border = "#bd93f9"
selected_fg = "#282a36"
selected_bg = "#50fa7b"
selected_modifier = ["bold"]
icon_color = "#f8f8f2"
help_fg = "#6272a4"
help_key_fg = "#ff79c6"
help_key_modifier = ["bold"]

[border]
enabled = true
style = "rounded"
```

#### Nord Theme

```toml
title = " POWER "
title_alignment = "left"
layout_mode = "grid"

[colors]
foreground = "#d8dee9"
background = "#2e3440"
border = "#88c0d0"
selected_fg = "#2e3440"
selected_bg = "#88c0d0"
selected_modifier = ["bold"]
icon_color = "#d8dee9"
help_fg = "#4c566a"
help_key_fg = "#81a1c1"
help_key_modifier = ["bold"]
```

#### Gruvbox Dark Theme

```toml
title = " ⏻ Menu "
title_alignment = "center"
layout_mode = "horizontal"

[colors]
foreground = "#ebdbb2"
background = "#282828"
border = "#d79921"
selected_fg = "#282828"
selected_bg = "#d79921"
selected_modifier = ["bold"]
icon_color = "#ebdbb2"
help_fg = "#928374"
help_key_fg = "#b8bb26"
help_key_modifier = ["bold"]

[actions.shutdown]
icon = "󰐥"
label = "Shutdown"
command = "systemctl"
args = ["poweroff"]
enabled = true
shortcut = "s"

[actions.reboot]
icon = "󰜉"
label = "Reboot"
command = "systemctl"
args = ["reboot"]
enabled = true
shortcut = "r"

[actions.suspend]
icon = "󰒲"
label = "Suspend"
command = "systemctl"
args = ["suspend"]
enabled = true
shortcut = "u"

[actions.lock]
icon = "󰌾"
label = "Lock"
command = "hyprlock"
args = []
enabled = true
shortcut = "l"

[actions.logout]
icon = "󰍃"
label = "Logout"
command = "hyprctl"
args = ["dispatch", "exit"]
enabled = true
shortcut = "o"

[actions.cancel]
icon = "󰜺"
label = "Cancel"
command = ""
args = []
enabled = true
shortcut = "c"
```

#### Clean (No Border, No Help)

```toml
title = ""
title_alignment = "center"
layout_mode = "compact"

[border]
enabled = false

[help_text]
enabled = false

[colors]
foreground = "#ffffff"
background = "#000000"
border = "#000000"
selected_fg = "#000000"
selected_bg = "#ffffff"
selected_modifier = []
icon_color = "#ffffff"
help_fg = "#808080"
help_key_fg = "#ffffff"
help_key_modifier = []
```

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run directly
cargo run
```

## Commands Executed

### Default Commands (Hyprland)

| Action     | Command                        |
|------------|--------------------------------|
| Shutdown   | `systemctl poweroff`           |
| Reboot     | `systemctl reboot`             |
| Suspend    | `systemctl suspend`            |
| Lock       | `hyprlock`                     |
| Logout     | `hyprctl dispatch exit`        |

### Auto-Detected Commands by Window Manager

| WM         | Lock Command    | Logout Command                    |
|------------|-----------------|-----------------------------------|
| Hyprland   | `hyprlock`      | `hyprctl dispatch exit`           |
| Sway       | `swaylock`      | `swaymsg exit`                    |
| i3         | `i3lock`        | `i3-msg exit`                     |
| BSPWM      | -               | `bspc quit`                       |
| AwesomeWM  | -               | `awesome-client "awesome.quit()"` |

*Note: Lock commands for i3, BSPWM, and AwesomeWM should be configured in your config file as these WMs don't have a default lock utility.*

## Dependencies

### Runtime Dependencies

- **systemd** - For system power management
- **hyprlock** / **swaylock** / **i3lock** - For screen locking (depending on your WM)
- **hyprctl** / **swaymsg** / **i3-msg** / **bspc** / **awesome-client** - For session management (auto-detected)

### Build Dependencies

- **ratatui** (0.28) - TUI framework
- **crossterm** (0.28) - Terminal manipulation
- **anyhow** (1.0) - Error handling
- **clap** (4.5) - Command-line argument parsing
- **serde** (1.0) - Serialization
- **toml** (0.8) - TOML parsing
- **directories** (5.0) - Config directory detection
- **rand** (0.8) - Random number generation (for animations)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development

```bash
# Clone the repository
git clone https://github.com/Ninso112/rexit.git
cd rexit

# Make your changes
# ...

# Format code
cargo fmt

# Check for issues
cargo clippy

# Run tests
cargo test

# Build and test
cargo run
```

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Author

**Ninso112** - [GitHub](https://github.com/Ninso112)

## Acknowledgments

- [Ratatui](https://github.com/ratatui/ratatui) - Terminal UI framework
- [Crossterm](https://github.com/crossterm-rs/crossterm) - Cross-platform terminal manipulation
- [Hyprland](https://github.com/hyprwm/Hyprland) - Dynamic tiling Wayland compositor

## Screenshots

The interface features a centered menu with a clean design:

```
┌─────────────────────┐
│       rexit         │
├─────────────────────┤
│ ⏻ Shutdown          │
│ ↻ Reboot            │
│ ⏾ Suspend           │
│ 🔒 Lock             │
│ ⇥ Logout            │
│ ✕ Cancel            │
└─────────────────────┘
↑↓/jk Navigate | Enter Select | q/Esc Quit
```

## Support

If you encounter any issues or have questions:

1. Check the [Issues](https://github.com/Ninso112/rexit/issues) page
2. Open a new issue with detailed information
3. Include your system information and error messages

---
