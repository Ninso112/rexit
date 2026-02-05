# rexit

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org/)
[![Hyprland](https://img.shields.io/badge/Hyprland-compatible-green.svg)](https://hyprland.org/)

A minimalist TUI (Text User Interface) power menu for Linux, specifically optimized for Hyprland users.

**Repository**: [https://github.com/Ninso112/rexit](https://github.com/Ninso112/rexit)

## Description

`rexit` is a lightweight, terminal-based power menu designed for Linux systems, with special optimizations for Hyprland window manager users. It provides a clean, minimal interface to quickly execute common power management commands without leaving your terminal.

### Features

- 🖥️ **Clean TUI Interface** - Built with Ratatui for a beautiful terminal experience
- ⌨️ **Keyboard Navigation** - Navigate with Arrow keys or Vim keys (j/k)
- 🚀 **Fast & Lightweight** - Minimal resource usage, instant startup
- 🎯 **Focused Functionality** - Six essential power options in one place
- 🔒 **Hyprland Integration** - Native support for hyprlock and hyprctl
- ⚡ **Zero Configuration** - Works out of the box

### Available Actions

- **Shutdown** ⏻ - Powers off the system
- **Reboot** ↻ - Restarts the system
- **Suspend** ⏾ - Suspends to RAM
- **Lock** 🔒 - Locks the session (Hyprland)
- **Logout** ⇥ - Exits the current session (Hyprland)
- **Cancel** ✕ - Closes the menu without action

## Installation

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

### Keyboard Controls

| Key           | Action                      |
|---------------|-----------------------------|
| `↑` / `k`     | Move selection up           |
| `↓` / `j`     | Move selection down         |
| `Enter`       | Execute selected command    |
| `Esc` / `q`   | Cancel and exit             |

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

## Commands Executed

| Action     | Command                        |
|------------|--------------------------------|
| Shutdown   | `systemctl poweroff`           |
| Reboot     | `systemctl reboot`             |
| Suspend    | `systemctl suspend`            |
| Lock       | `hyprlock`                     |
| Logout     | `hyprctl dispatch exit`        |

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

## Configuration

`rexit` works out of the box with sensible defaults. No configuration file is needed. All commands use standard system utilities.

### Customization

If you need to customize the commands, you can fork the repository and modify the `execute()` method in `src/main.rs`:

```rust
fn execute(&self) -> Result<()> {
    match self {
        PowerAction::Lock => {
            // Change this to your preferred lock command
            Command::new("your-lock-command")
                .spawn()
                .context("Failed to execute lock command")?;
        }
        // ... other actions
    }
    Ok(())
}
```

## Dependencies

### Runtime Dependencies

- **systemd** - For system power management
- **hyprlock** - For screen locking (Hyprland)
- **hyprctl** - For session management (Hyprland)

### Build Dependencies

- **ratatui** (0.28) - TUI framework
- **crossterm** (0.28) - Terminal manipulation
- **anyhow** (1.0) - Error handling
- **clap** (4.5) - Command-line argument parsing

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

The interface features a centered menu with a clean, minimal design:

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

**Repository**: [https://github.com/Ninso112/rexit](https://github.com/Ninso112/rexit)