use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEvent,
    },
    execute,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use directories::ProjectDirs;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Window title shown in the border
    pub title: String,
    /// Alignment of the title: "left", "center", "right"
    pub title_alignment: String,

    /// Border style configuration
    pub border: BorderConfig,

    /// Color configuration
    pub colors: ColorConfig,

    /// Keybindings configuration
    pub keys: KeyConfig,

    /// Action definitions (icons, labels, commands)
    pub actions: HashMap<String, ActionConfig>,

    /// Help text at the bottom
    pub help_text: HelpConfig,

    /// Layout configuration
    pub layout: LayoutConfig,

    /// Background animation configuration
    pub animation: AnimationConfig,

    /// Responsive layout settings
    pub responsive: ResponsiveConfig,

    /// Layout mode: "vertical", "horizontal", "grid", "compact"
    pub layout_mode: String,

    /// Window manager type: "auto", "hyprland", "sway", "i3", "bspwm", "awesome"
    pub wm_type: String,

    /// Grace period configuration for critical actions
    pub grace_period: GracePeriodConfig,

    /// Theme file to load (optional)
    pub theme: Option<String>,

    /// Whether to use emoji icons as fallback (auto-detected if not set)
    pub use_emoji_icons: Option<bool>,

    /// Performance settings
    pub performance: PerformanceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Enable automatic quality reduction under high CPU load
    pub auto_degrade: bool,
    /// Target frame time in milliseconds (higher = less CPU usage)
    pub target_fps: u32,
    /// Disable animations when battery is low (laptops)
    pub disable_on_low_battery: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            auto_degrade: true,
            target_fps: 30,
            disable_on_low_battery: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderConfig {
    pub enabled: bool,
    pub style: String, // "plain", "rounded", "double", "thick"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorConfig {
    pub foreground: String,
    pub background: String,
    pub border: String,
    pub selected_fg: String,
    pub selected_bg: String,
    pub selected_modifier: Vec<String>, // "bold", "italic", "underlined"
    pub icon_color: String,
    pub help_fg: String,
    pub help_key_fg: String,
    pub help_key_modifier: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub select: Vec<String>,
    pub quit: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionConfig {
    pub icon: String,
    /// Fallback icon using emoji (used when Nerd Fonts are not available)
    pub icon_fallback: Option<String>,
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
    /// Whether this action requires confirmation before executing
    pub confirm: bool,
    /// Whether this action is a favorite (shown at top)
    pub favorite: bool,
    /// Optional keyboard shortcut for quick access (e.g., "s", "1", "Ctrl-s")
    pub shortcut: String,
}

/// Theme configuration for loading themes from files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
    pub colors: ColorConfig,
    pub border: BorderConfig,
    pub animation: AnimationConfig,
}

/// System monitoring for graceful degradation
#[derive(Debug, Clone)]
pub struct PerformanceMonitor {
    last_check: Instant,
    frame_times: Vec<u64>,
    degraded_mode: bool,
    last_frame_time: u64,
}

impl PerformanceMonitor {
    pub fn new() -> Self {
        Self {
            last_check: Instant::now(),
            frame_times: Vec::with_capacity(30),
            degraded_mode: false,
            last_frame_time: 0,
        }
    }

    pub fn record_frame(&mut self, frame_time_ms: u64) {
        self.last_frame_time = frame_time_ms;
        self.frame_times.push(frame_time_ms);
        if self.frame_times.len() > 30 {
            self.frame_times.remove(0);
        }
    }

    pub fn update(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_check).as_secs();

        if elapsed >= 2 {
            // Calculate average frame time
            let avg_frame_time = if !self.frame_times.is_empty() {
                self.frame_times.iter().sum::<u64>() / self.frame_times.len() as u64
            } else {
                0
            };

            // Enable degraded mode if frame times are consistently long (>100ms)
            self.degraded_mode = avg_frame_time > 100 || self.last_frame_time > 150;

            // Clear frame times for next measurement period
            self.frame_times.clear();
            self.last_check = now;
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded_mode
    }

    pub fn should_skip_frame(&self, frame_counter: u64) -> bool {
        if self.degraded_mode {
            // Skip every other frame in degraded mode
            frame_counter % 2 == 0
        } else {
            false
        }
    }
}

/// Check if Nerd Fonts are available in the terminal
pub fn has_nerd_fonts() -> bool {
    // Check environment variable override
    if let Ok(val) = std::env::var("REXIT_USE_EMOJI") {
        return val != "1" && val != "true";
    }

    // Try to detect by checking common Nerd Font indicators
    if let Ok(term_program) = std::env::var("TERM_PROGRAM") {
        match term_program.as_str() {
            "Apple_Terminal" => return false, // macOS Terminal doesn't support Nerd Fonts well
            _ => {}
        }
    }

    // Check if we're in a Linux console (no Nerd Fonts support)
    if let Ok(term) = std::env::var("TERM") {
        if term == "linux" {
            return false;
        }
    }

    // Default to assuming Nerd Fonts are available on modern terminals
    true
}

/// Get the appropriate icon based on Nerd Font availability
pub fn get_icon(config: &ActionConfig) -> &str {
    if has_nerd_fonts() {
        &config.icon
    } else {
        config.icon_fallback.as_deref().unwrap_or_else(|| {
            // Default emoji fallbacks
            match config.icon.as_str() {
                "⏻" => "⏻",
                "🔄" => "🔄",
                "🌙" => "🌙",
                "🔒" => "🔒",
                "🚪" => "🚪",
                "❌" => "❌",
                _ => "•", // default bullet
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpConfig {
    pub enabled: bool,
    pub template: String,
    pub separator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Auto-scale menu to fit content (default: true)
    /// When enabled, the menu size is calculated based on content
    /// When disabled, uses percentage-based margins
    pub auto_scale: bool,
    pub vertical_margin: u16,
    pub horizontal_margin: u16,
    pub min_width: u16,
    pub min_height: u16,
    /// Maximum width of the menu (0 = unlimited, default: 60)
    pub max_width: u16,
    /// Padding inside the menu box (default: 1)
    pub padding: u16,
}

/// Responsive layout configuration for adapting to terminal size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsiveConfig {
    /// Enable responsive layout adjustments (default: true)
    pub enabled: bool,
    /// Switch to compact layout when terminal width is below this threshold (default: 80)
    pub compact_threshold: u16,
    /// Switch to minimal layout when terminal width is below this threshold (default: 40)
    pub minimal_threshold: u16,
    /// Adjust font size or spacing (when supported by terminal)
    pub auto_adjust_spacing: bool,
    /// Hide border when terminal is very small
    pub hide_border_when_small: bool,
    /// Minimum terminal dimensions to show the UI (default: 20x5)
    pub min_terminal_width: u16,
    pub min_terminal_height: u16,
}

impl Default for ResponsiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            compact_threshold: 80,
            minimal_threshold: 40,
            auto_adjust_spacing: true,
            hide_border_when_small: true,
            min_terminal_width: 20,
            min_terminal_height: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    /// Enable background animation
    pub enabled: bool,
    /// Animation type: "matrix", "matrix_cjk", "rain", "thunder", "snow", "stars", "fireflies", "fireworks", "neon_grid", "perlin_flow", "cube_3d", "fractals", "bubbles", "confetti", "wave", "particles", "digital_rain", "heartbeat", "plasma", "scanlines", "aurora", "autumn", "dna", "synthwave", "smoke", "gradient_flow", "constellation", "fish_tank", "typing_code", "vortex", "circuit", "flow_field", "morse", "lissajous", "game_of_life", "ocean", "ripple", "fog", "flames", "sparks", "lava_lamp", "sun", "galaxy", "meteor_shower", "satellite", "pulsar", "pong", "snake", "tetris", "invaders", "fibonacci", "mandelbrot", "hex_grid", "rose", "butterflies", "spider_web", "vine_growth", "moss", "radar", "binary_clock", "signal", "wifi", "paint_splatter", "ink_bleed", "mosaic", "stained_glass", "hologram", "glitch", "old_film", "thermal", "none"
    pub animation_type: String,
    /// Animation speed in milliseconds (lower = faster)
    pub speed_ms: u64,
    /// Animation color (for single-color animations)
    pub color: String,
    /// Animation density (0-100, higher = more particles)
    pub density: u8,
    /// Reduce animation quality when CPU is high (default: true)
    pub adaptive_quality: bool,
    /// Minimum animation speed in degraded mode (default: 200ms)
    pub min_speed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracePeriodConfig {
    /// Enable grace period for critical actions (default: true)
    pub enabled: bool,
    /// Duration of grace period in seconds (default: 5)
    pub duration_secs: u64,
    /// Show countdown in center of screen (default: true)
    pub show_countdown: bool,
    /// Text shown during grace period countdown
    pub message_template: String,
}

impl Default for Config {
    fn default() -> Self {
        let mut actions = HashMap::new();

        actions.insert(
            "shutdown".to_string(),
            ActionConfig {
                icon: "⏻".to_string(),
                icon_fallback: Some("⏻".to_string()),
                label: "Shutdown".to_string(),
                command: "systemctl".to_string(),
                args: vec!["poweroff".to_string()],
                enabled: true,
                confirm: true,
                favorite: true,
                shortcut: "s".to_string(),
            },
        );

        actions.insert(
            "reboot".to_string(),
            ActionConfig {
                icon: "🔄".to_string(),
                icon_fallback: Some("🔄".to_string()),
                label: "Reboot".to_string(),
                command: "systemctl".to_string(),
                args: vec!["reboot".to_string()],
                enabled: true,
                confirm: true,
                favorite: true,
                shortcut: "r".to_string(),
            },
        );

        actions.insert(
            "suspend".to_string(),
            ActionConfig {
                icon: "🌙".to_string(),
                icon_fallback: Some("🌙".to_string()),
                label: "Suspend".to_string(),
                command: "systemctl".to_string(),
                args: vec!["suspend".to_string()],
                enabled: true,
                confirm: false,
                favorite: false,
                shortcut: "u".to_string(),
            },
        );

        actions.insert(
            "lock".to_string(),
            ActionConfig {
                icon: "🔒".to_string(),
                icon_fallback: Some("🔒".to_string()),
                label: "Lock".to_string(),
                command: "hyprlock".to_string(),
                args: vec![],
                enabled: true,
                confirm: false,
                favorite: false,
                shortcut: "l".to_string(),
            },
        );

        actions.insert(
            "logout".to_string(),
            ActionConfig {
                icon: "🚪".to_string(),
                icon_fallback: Some("🚪".to_string()),
                label: "Logout".to_string(),
                command: "hyprctl".to_string(),
                args: vec!["dispatch".to_string(), "exit".to_string()],
                enabled: true,
                confirm: true,
                favorite: false,
                shortcut: "o".to_string(),
            },
        );

        actions.insert(
            "cancel".to_string(),
            ActionConfig {
                icon: "❌".to_string(),
                icon_fallback: Some("❌".to_string()),
                label: "Cancel".to_string(),
                command: "".to_string(),
                args: vec![],
                enabled: true,
                confirm: false,
                favorite: false,
                shortcut: "c".to_string(),
            },
        );

        let key_up = vec!["Up".to_string(), "k".to_string()];

        let key_down = vec!["Down".to_string(), "j".to_string()];

        let key_select = vec!["Enter".to_string()];

        let key_quit = vec!["Esc".to_string(), "q".to_string()];

        Config {
            title: " rexit ".to_string(),
            title_alignment: "center".to_string(),
            border: BorderConfig {
                enabled: true,
                style: "rounded".to_string(),
            },
            colors: ColorConfig {
                foreground: "white".to_string(),
                background: "black".to_string(),
                border: "cyan".to_string(),
                selected_fg: "black".to_string(),
                selected_bg: "white".to_string(),
                selected_modifier: vec!["bold".to_string()],
                icon_color: "white".to_string(),
                help_fg: "gray".to_string(),
                help_key_fg: "cyan".to_string(),
                help_key_modifier: vec!["bold".to_string()],
            },
            keys: KeyConfig {
                up: key_up,
                down: key_down,
                select: key_select,
                quit: key_quit,
            },
            actions,
            help_text: HelpConfig {
                enabled: true,
                template: "{keys} {action} | ".to_string(),
                separator: " | ".to_string(),
            },
            layout: LayoutConfig {
                auto_scale: true,
                vertical_margin: 30,
                horizontal_margin: 30,
                min_width: 30,
                min_height: 10,
                max_width: 60,
                padding: 1,
            },
            animation: AnimationConfig {
                enabled: true,
                animation_type: "matrix".to_string(),
                speed_ms: 80,
                color: "green".to_string(),
                density: 50,
                adaptive_quality: true,
                min_speed_ms: 200,
            },
            responsive: ResponsiveConfig::default(),
            layout_mode: "vertical".to_string(),
            wm_type: "auto".to_string(),
            grace_period: GracePeriodConfig {
                enabled: true,
                duration_secs: 5,
                show_countdown: true,
                message_template: "⏱️  {action} in {seconds}s... Press any key to cancel"
                    .to_string(),
            },
            theme: None,
            use_emoji_icons: None,
            performance: PerformanceSettings::default(),
        }
    }
}

// ============================================================================
// COLOR PARSING
// ============================================================================

fn parse_color(color_str: &str) -> Color {
    match color_str.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        // RGB format: #RRGGBB
        hex if hex.starts_with('#') && hex.len() == 7 => {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[1..3], 16),
                u8::from_str_radix(&hex[3..5], 16),
                u8::from_str_radix(&hex[5..7], 16),
            ) {
                Color::Rgb(r, g, b)
            } else {
                Color::White
            }
        }
        _ => Color::White,
    }
}

fn parse_modifier(modifiers: &[String]) -> Modifier {
    let mut result = Modifier::empty();
    for modifier in modifiers {
        match modifier.to_lowercase().as_str() {
            "bold" => result |= Modifier::BOLD,
            "italic" => result |= Modifier::ITALIC,
            "underlined" => result |= Modifier::UNDERLINED,
            "slowblink" => result |= Modifier::SLOW_BLINK,
            "rapidblink" => result |= Modifier::RAPID_BLINK,
            "reversed" => result |= Modifier::REVERSED,
            "hidden" => result |= Modifier::HIDDEN,
            "crossedout" => result |= Modifier::CROSSED_OUT,
            _ => {}
        }
    }
    result
}

// ============================================================================
// CONFIG LOADING
// ============================================================================

fn get_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "rexit").map(|dirs| dirs.config_dir().join("config.toml"))
}

fn get_last_executed_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "rexit").map(|dirs| dirs.config_dir().join("last_executed"))
}

fn load_last_executed() -> Option<String> {
    if let Some(path) = get_last_executed_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn save_last_executed(label: &str) {
    if let Some(path) = get_last_executed_path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, label);
    }
}

fn load_config() -> Config {
    if let Some(config_path) = get_config_path() {
        if config_path.exists() {
            match fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str::<Config>(&content) {
                    Ok(config) => {
                        return config;
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse config file: {}", e);
                        eprintln!("Using default configuration.");
                    }
                },
                Err(e) => {
                    eprintln!("Warning: Failed to read config file: {}", e);
                    eprintln!("Using default configuration.");
                }
            }
        }
    }
    Config::default()
}

fn generate_default_config() -> String {
    String::from(
        r##"## rexit configuration file
## Place this file at ~/.config/rexit/config.toml
## All fields are optional - defaults will be used for missing values

## Window title
title = " rexit "
title_alignment = "center"  ## Options: "left", "center", "right"

## Layout mode: "vertical", "horizontal", "grid", "compact"
layout_mode = "vertical"

## Window manager: "auto", "hyprland", "sway", "i3", "bspwm", "awesome"
## When set to "auto", rexit will detect your WM automatically
wm_type = "auto"

## Theme file (optional)
## Load a theme from ~/.config/rexit/themes/<name>.toml
## theme = "dracula"

## Use emoji icons instead of Nerd Fonts (auto-detected if not set)
## Set to true if your terminal doesn't support Nerd Fonts
## use_emoji_icons = false

[border]
enabled = true
style = "rounded"  ## Options: "plain", "rounded", "double", "thick"

[colors]
## Available colors:
## Standard: black, red, green, yellow, blue, magenta, cyan, gray, white
## Light variants: lightred, lightgreen, lightyellow, lightblue, lightmagenta, lightcyan
## Dark variants: darkgray
## Hex: "#RRGGBB" (e.g., "#ff0000" for red)
foreground = "white"
background = "black"
border = "cyan"
selected_fg = "black"
selected_bg = "white"
selected_modifier = ["bold"]  ## Options: bold, italic, underlined, slowblink, rapidblink, reversed, hidden, crossedout
icon_color = "white"
help_fg = "gray"
help_key_fg = "cyan"
help_key_modifier = ["bold"]

[keys]
## Key names: Use crossterm KeyCode names
## Examples: "q", "Esc", "Enter", "Up", "Down", "Left", "Right", "Tab", "Backspace"
## Modifiers can be added with format: "Ctrl-q", "Alt-q", "Shift-Up"
up = ["Up", "k"]
down = ["Down", "j"]
select = ["Enter"]
quit = ["Esc", "q"]

[actions.shutdown]
icon = "⏻"  # Power symbol (was: \u{f011})
icon_fallback = "⏻"  ## Emoji fallback when Nerd Fonts are not available
label = "Shutdown"
command = "systemctl"
args = ["poweroff"]
enabled = true
confirm = true      ## Require confirmation before executing
favorite = true     ## Show at top of list
shortcut = "s"      ## Press s to select

[actions.reboot]
icon = "🔄"  # Refresh symbol (was: \u{f021})
icon_fallback = "🔄"
label = "Reboot"
command = "systemctl"
args = ["reboot"]
enabled = true
confirm = true
favorite = true
shortcut = "r"

[actions.suspend]
icon = "🌙"  # Moon symbol (was: \u{f186})
icon_fallback = "🌙"
label = "Suspend"
command = "systemctl"
args = ["suspend"]
enabled = true
confirm = false
favorite = false
shortcut = "u"

[actions.lock]
icon = "🔒"  # Lock symbol (was: \u{f023})
icon_fallback = "🔒"
label = "Lock"
command = "hyprlock"
args = []
enabled = true
confirm = false
favorite = false
shortcut = "l"

[actions.logout]
icon = "🚪"  # Door symbol (was: \u{f08b})
icon_fallback = "🚪"
label = "Logout"
command = "hyprctl"
args = ["dispatch", "exit"]
enabled = true
confirm = true
favorite = false
shortcut = "o"

[actions.cancel]
icon = "❌"  # X mark (was: \u{f00d})
icon_fallback = "❌"
label = "Cancel"
command = ""
args = []
enabled = true
confirm = false
favorite = false
shortcut = "c"

[help_text]
enabled = true
template = "{keys} {action} | "
separator = " | "

[layout]
## Auto-scale menu to fit content (default: true)
## When true, menu size is calculated based on content length
## When false, uses percentage-based margins from vertical/horizontal_margin
auto_scale = true
vertical_margin = 30
horizontal_margin = 30
min_width = 30
min_height = 10
## Maximum width when auto_scale is enabled (0 = unlimited, default: 60)
max_width = 60
## Padding inside the menu box (default: 1)
padding = 1

[responsive]
## Responsive layout settings
enabled = true                    ## Enable responsive layout adjustments
compact_threshold = 80            ## Switch to compact below this width
minimal_threshold = 40            ## Switch to minimal below this width
auto_adjust_spacing = true        ## Adjust spacing automatically
hide_border_when_small = true     ## Hide border when terminal is small
min_terminal_width = 20           ## Minimum terminal width required
min_terminal_height = 5           ## Minimum terminal height required

[animation]
## Background animation settings
## Animation types: "matrix", "matrix_cjk", "rain", "thunder", "snow", "stars", "fireflies", "fireworks", "neon_grid", "perlin_flow", "cube_3d", "fractals", "bubbles", "confetti", "wave", "particles", "digital_rain", "heartbeat", "plasma", "scanlines", "aurora", "autumn", "dna", "synthwave", "smoke", "gradient_flow", "constellation", "fish_tank", "typing_code", "vortex", "circuit", "flow_field", "morse", "lissajous", "game_of_life", "ocean", "ripple", "fog", "flames", "sparks", "lava_lamp", "sun", "galaxy", "meteor_shower", "satellite", "pulsar", "pong", "snake", "tetris", "invaders", "fibonacci", "mandelbrot", "hex_grid", "rose", "butterflies", "spider_web", "vine_growth", "moss", "radar", "binary_clock", "signal", "wifi", "paint_splatter", "ink_bleed", "mosaic", "stained_glass", "hologram", "glitch", "old_film", "thermal", "none"
enabled = true
animation_type = "matrix"
speed_ms = 80
color = "green"
density = 50
adaptive_quality = true           ## Reduce quality under high CPU load
min_speed_ms = 200                ## Minimum animation speed in degraded mode

[grace_period]
## Grace period configuration for critical actions (shutdown, reboot)
## Allows canceling the action during the countdown
enabled = true
duration_secs = 5
show_countdown = true
message_template = "⏱️  {action} in {seconds}s... Press any key to cancel"

[performance]
## Performance settings
auto_degrade = true               ## Enable automatic quality reduction under high CPU
target_fps = 30                   ## Target frame rate (higher = smoother but more CPU)
disable_on_low_battery = false    ## Disable animations when battery is low (laptops)
"##,
    )
}

// ============================================================================
// THEME LOADING
// ============================================================================

fn get_themes_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "rexit").map(|dirs| dirs.config_dir().join("themes"))
}

fn get_theme_path(theme_name: &str) -> Option<PathBuf> {
    get_themes_dir().map(|dir| dir.join(format!("{}.toml", theme_name)))
}

fn load_theme(theme_name: &str) -> Option<ThemeConfig> {
    let theme_path = get_theme_path(theme_name)?;

    if !theme_path.exists() {
        eprintln!(
            "Warning: Theme '{}' not found at {}",
            theme_name,
            theme_path.display()
        );
        return None;
    }

    match fs::read_to_string(&theme_path) {
        Ok(content) => match toml::from_str::<ThemeConfig>(&content) {
            Ok(theme) => {
                return Some(theme);
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse theme '{}': {}", theme_name, e);
            }
        },
        Err(e) => {
            eprintln!("Warning: Failed to read theme '{}': {}", theme_name, e);
        }
    }
    None
}

fn list_available_themes() -> Vec<String> {
    let mut themes = Vec::new();

    if let Some(themes_dir) = get_themes_dir() {
        if let Ok(entries) = fs::read_dir(themes_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".toml") {
                        themes.push(name[..name.len() - 5].to_string());
                    }
                }
            }
        }
    }

    themes
}

fn merge_theme_into_config(config: &mut Config, theme: ThemeConfig) {
    config.colors = theme.colors;
    config.border = theme.border;
    config.animation.animation_type = theme.animation.animation_type;
    config.animation.speed_ms = theme.animation.speed_ms;
    config.animation.color = theme.animation.color;
    config.animation.density = theme.animation.density;
    config.animation.adaptive_quality = theme.animation.adaptive_quality;
    config.animation.min_speed_ms = theme.animation.min_speed_ms;
}

fn check_command_exists(command: &str) -> bool {
    if command.is_empty() {
        return true;
    }

    // Handle commands with paths
    if command.contains('/') {
        return PathBuf::from(command).exists();
    }

    // Check in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for path in path_var.split(':') {
            let full_path = PathBuf::from(path).join(command);
            if full_path.exists() {
                return true;
            }
        }
    }

    false
}

fn find_lock_command() -> (String, Vec<String>) {
    // Try common lock commands in order of preference
    let lock_commands = [
        ("hyprlock", vec![]),
        ("swaylock", vec![]),
        ("i3lock", vec![]),
        ("i3lock-fancy", vec![]),
        ("betterlockscreen", vec!["--lock".to_string()]),
        ("xlock", vec![]),
        ("slock", vec![]),
        ("xflock4", vec![]),
        ("gnome-screensaver-command", vec!["--lock".to_string()]),
        ("xscreensaver-command", vec!["--lock".to_string()]),
        ("loginctl", vec!["lock-session".to_string()]),
    ];

    for (cmd, args) in lock_commands {
        if check_command_exists(cmd) {
            return (cmd.to_string(), args);
        }
    }

    // Fallback to loginctl as it should work on most systemd systems
    ("loginctl".to_string(), vec!["lock-session".to_string()])
}

// ============================================================================
// KEY PARSING
// ============================================================================

#[derive(Debug, Clone)]
struct KeyBinding {
    key: KeyCode,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

fn parse_key(key_str: &str) -> Option<KeyBinding> {
    let parts: Vec<&str> = key_str.split('-').collect();

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut key_part = key_str;

    if parts.len() > 1 {
        for modifier in &parts[..parts.len() - 1] {
            match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                _ => {}
            }
        }
        key_part = parts.last().unwrap();
    }

    let key = match key_part {
        "Esc" | "esc" | "Escape" => KeyCode::Esc,
        "Enter" | "enter" | "Return" => KeyCode::Enter,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" | "Del" => KeyCode::Delete,
        "Insert" | "Ins" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "F1" => KeyCode::F(1),
        "F2" => KeyCode::F(2),
        "F3" => KeyCode::F(3),
        "F4" => KeyCode::F(4),
        "F5" => KeyCode::F(5),
        "F6" => KeyCode::F(6),
        "F7" => KeyCode::F(7),
        "F8" => KeyCode::F(8),
        "F9" => KeyCode::F(9),
        "F10" => KeyCode::F(10),
        "F11" => KeyCode::F(11),
        "F12" => KeyCode::F(12),
        c if c.len() == 1 => {
            let ch = c.chars().next().unwrap();
            KeyCode::Char(ch)
        }
        _ => return None,
    };

    Some(KeyBinding {
        key,
        ctrl,
        alt,
        shift,
    })
}

fn matches_key(key: &KeyBinding, event: &crossterm::event::KeyEvent) -> bool {
    if key.key != event.code {
        return false;
    }

    let modifiers = event.modifiers;
    let ctrl = modifiers.contains(crossterm::event::KeyModifiers::CONTROL);
    let alt = modifiers.contains(crossterm::event::KeyModifiers::ALT);
    let shift = modifiers.contains(crossterm::event::KeyModifiers::SHIFT);

    key.ctrl == ctrl && key.alt == alt && key.shift == shift
}

// ============================================================================
// ACTION DEFINITION
// ============================================================================

#[derive(Debug, Clone)]
struct Action {
    icon: String,
    label: String,
    command: String,
    args: Vec<String>,
    confirm: bool,
    favorite: bool,
    shortcut: String,
}

impl Action {
    fn display_text(&self, show_shortcut: bool) -> String {
        if show_shortcut && !self.shortcut.is_empty() {
            format!("{} [{}] {}", self.icon, self.shortcut, self.label)
        } else {
            format!("{} {}", self.icon, self.label)
        }
    }

    fn is_critical(&self) -> bool {
        // Auto-detect critical actions if confirm is not explicitly set
        let lower = self.label.to_lowercase();
        lower.contains("shutdown")
            || lower.contains("reboot")
            || lower.contains("poweroff")
            || lower.contains("halt")
    }

    fn execute(&self) -> Result<()> {
        if self.command.is_empty() {
            return Ok(());
        }

        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);

        cmd.status()
            .with_context(|| format!("Failed to execute command: {}", self.command))?;

        Ok(())
    }
}

// ============================================================================
// APPLICATION STATE
// ============================================================================

enum AppState {
    Selecting,
    Confirming {
        action_index: usize,
    },
    GracePeriod {
        action_index: usize,
        remaining_secs: u64,
        last_tick: std::time::Instant,
    },
    AnimationMenu,
}

/// Tracks easter egg state for Konami code
#[derive(Debug)]
struct EasterEggState {
    sequence: Vec<KeyCode>,
    konami_code: Vec<KeyCode>,
    rainbow_mode: bool,
}

struct App {
    actions: Vec<Action>,
    selected_index: usize,
    should_quit: bool,
    config: Config,
    animation_state: AnimationState,
    state: AppState,
    last_executed: Option<String>, // label of last executed action
    easter_egg: EasterEggState,
    animation_menu_index: usize,
    grace_period_cancelled: bool, // Track if grace period was cancelled
    performance_monitor: PerformanceMonitor,
}

const ANIMATION_TYPES: &[&str; 71] = &[
    "matrix",
    "matrix_cjk",
    "rain",
    "thunder",
    "snow",
    "stars",
    "fireflies",
    "fireworks",
    "neon_grid",
    "perlin_flow",
    "cube_3d",
    "fractals",
    "bubbles",
    "confetti",
    "wave",
    "particles",
    "digital_rain",
    "heartbeat",
    "plasma",
    "scanlines",
    "aurora",
    "autumn",
    "dna",
    "synthwave",
    "smoke",
    "gradient_flow",
    "constellation",
    "fish_tank",
    "typing_code",
    "vortex",
    "circuit",
    "flow_field",
    "morse",
    "lissajous",
    "game_of_life",
    // New animations v1.1.5
    "ocean",
    "ripple",
    "fog",
    "flames",
    "sparks",
    "lava_lamp",
    "sun",
    "galaxy",
    "meteor_shower",
    "satellite",
    "pulsar",
    "pong",
    "snake",
    "tetris",
    "invaders",
    "fibonacci",
    "mandelbrot",
    "hex_grid",
    "rose",
    "butterflies",
    "spider_web",
    "vine_growth",
    "moss",
    "radar",
    "binary_clock",
    "signal",
    "wifi",
    "paint_splatter",
    "ink_bleed",
    "mosaic",
    "stained_glass",
    "hologram",
    "glitch",
    "old_film",
    "thermal",
    "none",
];

/// Animation state for background effects
struct AnimationState {
    /// Current animation frame/tick
    tick: u64,
    /// Matrix rain columns (x position, y position, speed, char)
    matrix_columns: Vec<MatrixColumn>,
    /// Rain drops (x position, y position, speed)
    rain_drops: Vec<RainDrop>,
    /// Snow flakes (x position, y position, speed, size)
    snow_flakes: Vec<SnowFlake>,
    /// Stars (x position, y position, brightness, twinkle speed)
    stars: Vec<Star>,
    /// Fireflies (x position, y position, dx, dy, brightness)
    fireflies: Vec<Firefly>,
    /// Bubbles (x position, y position, speed, size)
    bubbles: Vec<Bubble>,
    /// Confetti particles (x position, y position, dx, dy, color, rotation)
    confetti: Vec<Confetti>,
    /// Wave offset
    wave_offset: f32,
    /// Particles
    particles: Vec<Particle>,
    /// Plasma cells
    plasma: Vec<PlasmaCell>,
    /// Scanline position
    scanline_pos: u16,
    /// Aurora phase
    aurora_phase: f32,
    /// Autumn leaves
    leaves: Vec<Leaf>,
    /// DNA helix
    dna: Vec<DnaBase>,
    /// Synthwave grid offset
    synthwave_offset: f32,
    /// Smoke particles
    smoke: Vec<Particle>,
    /// Gradient phase
    gradient_phase: f32,
    /// Constellation nodes
    nodes: Vec<ConstellationNode>,
    /// Fish tank
    fish: Vec<Fish>,
    /// Typing code
    code_lines: Vec<String>,
    code_line_idx: usize,
    code_char_idx: usize,
    /// Vortex angle
    vortex_angle: f32,
    /// Circuit traces
    traces: Vec<CircuitTrace>,
    /// Flow field particles
    flow_particles: Vec<FlowParticle>,
    /// Morse code
    morse_message: String,
    morse_idx: usize,
    morse_timer: u8,
    morse_display: String,
    /// Lissajous curves
    lissajous: Vec<LissajousCurve>,
    /// Game of life grid
    gol_grid: Vec<GameOfLifeCell>,
    gol_width: usize,
    gol_height: usize,
    /// Thunder flash state
    thunder_flash: u8,
    /// Heartbeat phase
    heartbeat_phase: f32,
    /// Fireworks particles
    fireworks: Vec<Firework>,
    /// Neon grid offset
    neon_offset: f32,
    /// Perlin flow field
    perlin_offset: f32,
    /// 3D cube rotation
    cube_rotation: CubeRotation,
    /// Fractal zoom/offset
    fractal_offset: (f32, f32),
    /// Ocean wave phase
    ocean_phase: f32,
    /// Ripple center and radius
    ripple_radius: f32,
    /// Fog density
    fog_density: f32,
    /// Flame particles
    flames: Vec<FlameParticle>,
    /// Spark particles
    sparks: Vec<Spark>,
    /// Lava lamp blobs
    lava_blobs: Vec<LavaBlob>,
    /// Sun pulse phase
    sun_phase: f32,
    /// Galaxy rotation
    galaxy_angle: f32,
    /// Meteor shower particles
    meteors: Vec<Meteor>,
    /// Satellite position
    satellite: Satellite,
    /// Pulsar rotation
    pulsar_angle: f32,
    /// Pong game state
    pong: PongGame,
    /// Snake game state
    snake: SnakeGame,
    /// Tetris game state
    tetris: TetrisGame,
    /// Space invaders
    invaders: Vec<Invader>,
    /// Fibonacci spiral angle
    fibonacci_angle: f32,
    /// Mandelbrot offset
    mandelbrot_offset: (f32, f32),
    /// Hex grid phase
    hex_phase: f32,
    /// Rose curve parameters
    rose_angle: f32,
    /// Butterflies
    butterflies: Vec<Butterfly>,
    /// Spider web strands
    web_strands: Vec<WebStrand>,
    /// Vines
    vines: Vec<Vine>,
    /// Moss cells
    moss: Vec<MossCell>,
    /// Radar sweep angle
    radar_angle: f32,
    /// Binary clock time
    binary_time: u64,
    /// Signal waves
    signals: Vec<SignalWave>,
    /// Wifi waves
    wifi_waves: Vec<WifiWave>,
    /// Paint splatters
    splatters: Vec<PaintSplatter>,
    /// Ink drops
    ink_drops: Vec<InkDrop>,
    /// Mosaic tiles
    mosaic_tiles: Vec<MosaicTile>,
    /// Stained glass
    glass_panels: Vec<GlassPanel>,
    /// Hologram scanline
    hologram_line: u16,
    /// Glitch timer
    glitch_timer: u8,
    /// Old film scratches
    scratches: Vec<FilmScratch>,
    /// Thermal noise
    thermal_noise: Vec<u8>,
    /// Last update time
    last_update: std::time::Instant,
}

struct MatrixColumn {
    x: u16,
    y: f32,
    speed: f32,
    char_idx: usize,
}

struct RainDrop {
    x: u16,
    y: f32,
    speed: f32,
    length: u16,
}

struct SnowFlake {
    x: f32,
    y: f32,
    speed: f32,
    size: u8,
}

struct Star {
    x: u16,
    y: u16,
    brightness: u8,
    twinkle_speed: f32,
    twinkle_offset: f32,
}

struct Firefly {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    brightness: u8,
}

struct Bubble {
    x: f32,
    y: f32,
    speed: f32,
    size: u8,
    wobble: f32,
}

struct Confetti {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    color: u8, // hue 0-255
    rotation: f32,
    rotation_speed: f32,
    character: char,
}

struct Particle {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    life: u8,
    max_life: u8,
    color: Color,
}

struct PlasmaCell {
    x: u16,
    y: u16,
    value: f32,
}

struct Leaf {
    x: f32,
    y: f32,
    rotation: f32,
    rotation_speed: f32,
    speed: f32,
    color: u8,
}

struct DnaBase {
    y: f32,
    left_char: char,
    right_char: char,
    connection: bool,
}

struct Fish {
    x: f32,
    y: f32,
    dx: f32,
    direction: bool, // true = right, false = left
    color: u8,
}

struct ConstellationNode {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
}

struct CircuitTrace {
    x: u16,
    y: u16,
    direction: u8, // 0=up, 1=right, 2=down, 3=left
    life: u8,
}

struct FlowParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    color: u8,
}

struct LissajousCurve {
    a: f32,
    b: f32,
    delta: f32,
    t: f32,
    color: u8,
}

// New animation structs for v1.1.5
struct FlameParticle {
    x: f32,
    _y: f32,
    height: f32,
    intensity: u8,
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u8,
    brightness: u8,
}

struct LavaBlob {
    x: f32,
    y: f32,
    size: f32,
    dy: f32,
    color_phase: f32,
}

struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    tail_length: u8,
    brightness: u8,
}

struct Satellite {
    x: f32,
    y: f32,
    angle: f32,
    orbit_radius: f32,
    signal_timer: u8,
}

struct PongGame {
    ball_x: f32,
    ball_y: f32,
    ball_vx: f32,
    ball_vy: f32,
    paddle1_y: f32,
    paddle2_y: f32,
    score1: u8,
    score2: u8,
}

struct SnakeGame {
    segments: Vec<(u16, u16)>,
    direction: u8, // 0=up, 1=right, 2=down, 3=left
    food: (u16, u16),
    tick_count: u8,
}

struct TetrisGame {
    pieces: Vec<(u16, u16, u8)>, // x, y, piece_type
    falling_piece: Option<(u16, u16, u8)>,
    tick_count: u8,
}

struct Invader {
    x: f32,
    y: f32,
    invader_type: u8,
    direction: i8,
    anim_frame: bool,
}

struct Butterfly {
    x: f32,
    y: f32,
    target_x: f32,
    target_y: f32,
    wing_open: bool,
    color: u8,
}

struct WebStrand {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    vibration: f32,
}

struct Vine {
    x: f32,
    _y: f32,
    length: u16,
    growth_rate: f32,
    max_length: u16,
}

struct MossCell {
    x: u16,
    y: u16,
    age: u8,
    spreading: bool,
}

struct SignalWave {
    x: u16,
    y: u16,
    radius: f32,
    max_radius: f32,
    amplitude: u8,
}

struct WifiWave {
    _center_x: f32,
    _center_y: f32,
    radius: f32,
    intensity: u8,
}

struct PaintSplatter {
    x: u16,
    y: u16,
    size: u8,
    color: (u8, u8, u8),
    age: u8,
}

struct InkDrop {
    x: f32,
    y: f32,
    radius: f32,
    max_radius: f32,
    color: (u8, u8, u8),
}

struct MosaicTile {
    x: u16,
    y: u16,
    color: (u8, u8, u8),
    changing: bool,
    change_timer: u8,
}

struct GlassPanel {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    hue: u8,
    light_intensity: u8,
}

struct FilmScratch {
    x: u16,
    y: u16,
    length: u8,
    visible: bool,
}

struct GameOfLifeCell {
    x: usize,
    y: usize,
    alive: bool,
    next_state: bool,
    age: u8,
}

struct Firework {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    particles: Vec<FireworkParticle>,
    exploded: bool,
    life: u8,
    color: (u8, u8, u8),
}

struct FireworkParticle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: u8,
    max_life: u8,
}

struct CubeRotation {
    angle_x: f32,
    angle_y: f32,
    angle_z: f32,
}

impl App {
    fn previous_horizontal(&mut self) {
        if !self.actions.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.actions.len() - 1;
            }
        }
    }

    fn next_horizontal(&mut self) {
        if !self.actions.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.actions.len();
        }
    }

    fn previous_grid(&mut self, cols: usize) {
        if !self.actions.is_empty() {
            let current_row = self.selected_index / cols;
            let current_col = self.selected_index % cols;

            if current_row > 0 {
                // Move up in same column
                self.selected_index -= cols;
            } else {
                // Wrap to bottom of same column
                let total_rows = self.actions.len().div_ceil(cols);
                let new_row = (total_rows - 1).min(current_row);
                self.selected_index = (new_row * cols + current_col).min(self.actions.len() - 1);
            }
        }
    }

    fn next_grid(&mut self, cols: usize) {
        if !self.actions.is_empty() {
            let new_index = self.selected_index + cols;
            if new_index < self.actions.len() {
                self.selected_index = new_index;
            } else {
                // Wrap to top of same column
                let current_col = self.selected_index % cols;
                self.selected_index = current_col.min(self.actions.len() - 1);
            }
        }
    }

    fn new(config: Config) -> Self {
        // Determine if we should use emoji icons
        let use_emoji = config.use_emoji_icons.unwrap_or_else(|| !has_nerd_fonts());

        let mut actions: Vec<Action> = config
            .actions
            .iter()
            .filter(|(_, action_config)| action_config.enabled)
            .map(|(_id, action_config)| {
                let icon = if use_emoji {
                    action_config
                        .icon_fallback
                        .clone()
                        .unwrap_or_else(|| get_icon(action_config).to_string())
                } else {
                    action_config.icon.clone()
                };
                Action {
                    icon,
                    label: action_config.label.clone(),
                    command: action_config.command.clone(),
                    args: action_config.args.clone(),
                    confirm: action_config.confirm,
                    favorite: action_config.favorite,
                    shortcut: action_config.shortcut.clone(),
                }
            })
            .collect();

        // Sort: favorites first, then by label
        actions.sort_by(|a, b| match (b.favorite, a.favorite) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a.label.cmp(&b.label),
        });

        // Load last executed action and find its index
        let last_executed = load_last_executed();
        let selected_index = last_executed
            .as_ref()
            .and_then(|label| actions.iter().position(|a| &a.label == label))
            .unwrap_or(0);

        // Detect WM if set to auto
        let mut config = config;
        if config.wm_type == "auto" {
            config.wm_type = detect_wm();
        }

        // Update logout command based on detected WM
        for action in &mut actions {
            if action.label.to_lowercase().contains("logout")
                || action.label.to_lowercase().contains("exit")
            {
                let (cmd, args) = get_logout_command(&config.wm_type);
                if !cmd.is_empty() {
                    action.command = cmd;
                    action.args = args;
                }
            }
        }

        // Check for lock command availability and fallback if needed
        for action in &mut actions {
            if action.label.to_lowercase().contains("lock") {
                if !check_command_exists(&action.command) {
                    let (cmd, args) = find_lock_command();
                    action.command = cmd;
                    action.args = args;
                }
            }
        }

        let mut app = Self {
            actions,
            selected_index,
            should_quit: false,
            config,
            animation_state: AnimationState::new(),
            state: AppState::Selecting,
            last_executed,
            easter_egg: EasterEggState::new(),
            animation_menu_index: 0,
            grace_period_cancelled: false,
            performance_monitor: PerformanceMonitor::new(),
        };

        // Initialize animation based on terminal size
        let terminal_size = ratatui::layout::Rect::new(0, 0, 80, 24);
        app.animation_state.init(&app.config, terminal_size);

        app
    }

    fn open_animation_menu(&mut self) {
        // Find current animation index
        self.animation_menu_index = ANIMATION_TYPES
            .iter()
            .position(|&a| a == self.config.animation.animation_type)
            .unwrap_or(0);
        self.state = AppState::AnimationMenu;
    }

    fn close_animation_menu(&mut self) {
        self.state = AppState::Selecting;
    }

    fn next_animation(&mut self) {
        self.animation_menu_index = (self.animation_menu_index + 1) % ANIMATION_TYPES.len();
    }

    fn previous_animation(&mut self) {
        if self.animation_menu_index > 0 {
            self.animation_menu_index -= 1;
        } else {
            self.animation_menu_index = ANIMATION_TYPES.len() - 1;
        }
    }

    fn select_animation(&mut self, size: Rect) {
        let selected = ANIMATION_TYPES[self.animation_menu_index];
        self.config.animation.animation_type = selected.to_string();
        self.animation_state.init(&self.config, size);
        self.state = AppState::Selecting;
    }

    fn next(&mut self) {
        if !self.actions.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.actions.len();
        }
    }

    fn previous(&mut self) {
        if !self.actions.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.actions.len() - 1;
            }
        }
    }

    fn select(&mut self) -> Result<()> {
        if let Some(action) = self.actions.get(self.selected_index) {
            // Check if confirmation is needed (explicitly set OR auto-detected critical action)
            let needs_confirm = action.confirm || action.is_critical();

            if needs_confirm && !matches!(self.state, AppState::Confirming { .. }) {
                // Enter confirmation mode
                self.state = AppState::Confirming {
                    action_index: self.selected_index,
                };
                return Ok(());
            }

            // Check if grace period is enabled for critical actions
            let needs_grace = self.config.grace_period.enabled
                && action.is_critical()
                && self.config.grace_period.duration_secs > 0;

            if needs_grace && !matches!(self.state, AppState::GracePeriod { .. }) {
                // Enter grace period mode
                self.state = AppState::GracePeriod {
                    action_index: self.selected_index,
                    remaining_secs: self.config.grace_period.duration_secs,
                    last_tick: std::time::Instant::now(),
                };
                self.grace_period_cancelled = false;
                return Ok(());
            }

            action.execute()?;
            self.last_executed = Some(action.label.clone());
            save_last_executed(&action.label);
        }
        self.should_quit = true;
        Ok(())
    }

    fn select_at_index(&mut self, index: usize) -> Result<()> {
        if index < self.actions.len() {
            self.selected_index = index;
            self.select()
        } else {
            Ok(())
        }
    }

    fn confirm_yes(&mut self) -> Result<()> {
        if let AppState::Confirming { action_index } = self.state {
            // Check if grace period is enabled for critical actions
            if let Some(action) = self.actions.get(action_index) {
                let needs_grace = self.config.grace_period.enabled
                    && action.is_critical()
                    && self.config.grace_period.duration_secs > 0;

                if needs_grace {
                    // Enter grace period mode instead of executing immediately
                    self.state = AppState::GracePeriod {
                        action_index,
                        remaining_secs: self.config.grace_period.duration_secs,
                        last_tick: std::time::Instant::now(),
                    };
                    self.grace_period_cancelled = false;
                    return Ok(());
                }

                action.execute()?;
                self.last_executed = Some(action.label.clone());
                save_last_executed(&action.label);
            }
            self.should_quit = true;
        }
        Ok(())
    }

    fn confirm_no(&mut self) {
        self.state = AppState::Selecting;
    }

    fn cancel_grace_period(&mut self) {
        self.grace_period_cancelled = true;
        self.state = AppState::Selecting;
    }

    fn update_grace_period(&mut self) -> Result<bool> {
        if let AppState::GracePeriod {
            action_index,
            remaining_secs,
            last_tick,
        } = self.state
        {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_tick).as_secs();

            if elapsed >= 1 {
                let new_remaining = remaining_secs.saturating_sub(elapsed);
                if new_remaining == 0 {
                    // Grace period expired, execute the action
                    if let Some(action) = self.actions.get(action_index) {
                        action.execute()?;
                        self.last_executed = Some(action.label.clone());
                        save_last_executed(&action.label);
                    }
                    self.should_quit = true;
                    return Ok(true);
                } else {
                    // Update remaining time
                    self.state = AppState::GracePeriod {
                        action_index,
                        remaining_secs: new_remaining,
                        last_tick: now,
                    };
                }
            }
        }
        Ok(false)
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn check_key(&self, key_str: &str, event: &crossterm::event::KeyEvent) -> bool {
        if let Some(key_binding) = parse_key(key_str) {
            matches_key(&key_binding, event)
        } else {
            false
        }
    }

    fn update_animation(&mut self, area: Rect) {
        if !self.config.animation.enabled || self.config.animation.animation_type == "none" {
            return;
        }

        // Update performance monitor
        self.performance_monitor.update();

        // Check if we should skip this frame due to degraded performance
        if self.config.performance.auto_degrade
            && self
                .performance_monitor
                .should_skip_frame(self.animation_state.tick)
        {
            return;
        }

        let now = std::time::Instant::now();
        let elapsed = now
            .duration_since(self.animation_state.last_update)
            .as_millis() as u64;

        // Calculate target frame time from FPS setting
        let target_frame_time = 1000 / self.config.performance.target_fps as u64;

        // Use min_speed_ms in degraded mode or if adaptive quality is enabled and CPU is high
        let speed_ms =
            if self.config.animation.adaptive_quality && self.performance_monitor.is_degraded() {
                self.config.animation.min_speed_ms.max(target_frame_time)
            } else {
                self.config.animation.speed_ms.max(target_frame_time)
            };

        if elapsed < speed_ms {
            return;
        }

        // Record frame time for performance monitoring
        self.performance_monitor.record_frame(elapsed);

        self.animation_state.last_update = now;
        self.animation_state.tick += 1;

        // Reinitialize if terminal size changed significantly
        if area.width > 0 && area.height > 0 {
            let needs_init = match self.config.animation.animation_type.as_str() {
                "matrix" => {
                    self.animation_state.matrix_columns.is_empty()
                        && self.config.animation.density > 0
                }
                "rain" => {
                    self.animation_state.rain_drops.is_empty() && self.config.animation.density > 0
                }
                "snow" => {
                    self.animation_state.snow_flakes.is_empty() && self.config.animation.density > 0
                }
                "stars" => {
                    self.animation_state.stars.is_empty() && self.config.animation.density > 0
                }
                "fireflies" => {
                    self.animation_state.fireflies.is_empty() && self.config.animation.density > 0
                }
                "bubbles" => {
                    self.animation_state.bubbles.is_empty() && self.config.animation.density > 0
                }
                "confetti" => {
                    self.animation_state.confetti.is_empty() && self.config.animation.density > 0
                }
                "wave" => false,
                "particles" => {
                    self.animation_state.particles.is_empty() && self.config.animation.density > 0
                }
                "digital_rain" => {
                    self.animation_state.matrix_columns.is_empty()
                        && self.config.animation.density > 0
                }
                "heartbeat" => false,
                "plasma" => self.animation_state.plasma.is_empty(),
                "scanlines" => false,
                "aurora" => false,
                "autumn" => {
                    self.animation_state.leaves.is_empty() && self.config.animation.density > 0
                }
                "dna" => self.animation_state.dna.is_empty(),
                "synthwave" => false,
                "smoke" => {
                    self.animation_state.smoke.is_empty() && self.config.animation.density > 0
                }
                "gradient_flow" => false,
                "constellation" => {
                    self.animation_state.nodes.is_empty() && self.config.animation.density > 0
                }
                "fish_tank" => {
                    self.animation_state.fish.is_empty() && self.config.animation.density > 0
                }
                "typing_code" => self.animation_state.code_lines.is_empty(),
                "vortex" => false,
                "circuit" => {
                    self.animation_state.traces.is_empty() && self.config.animation.density > 0
                }
                "flow_field" => {
                    self.animation_state.flow_particles.is_empty()
                        && self.config.animation.density > 0
                }
                "morse" => self.animation_state.morse_message.is_empty(),
                "lissajous" => self.animation_state.lissajous.is_empty(),
                "game_of_life" => self.animation_state.gol_grid.is_empty(),
                "matrix_cjk" => {
                    self.animation_state.matrix_columns.is_empty()
                        && self.config.animation.density > 0
                }
                "fireworks" => self.animation_state.fireworks.is_empty(),
                "neon_grid" => false,
                "perlin_flow" => false,
                "cube_3d" => false,
                "fractals" => false,
                // New animations v1.1.5
                "ocean" => false,
                "ripple" => false,
                "fog" => false,
                "flames" => {
                    self.animation_state.flames.is_empty() && self.config.animation.density > 0
                }
                "sparks" => {
                    self.animation_state.sparks.is_empty() && self.config.animation.density > 0
                }
                "lava_lamp" => {
                    self.animation_state.lava_blobs.is_empty() && self.config.animation.density > 0
                }
                "sun" => false,
                "galaxy" => false,
                "meteor_shower" => {
                    self.animation_state.meteors.is_empty() && self.config.animation.density > 0
                }
                "satellite" => false,
                "pulsar" => false,
                "pong" => false,
                "snake" => self.animation_state.snake.segments.is_empty(),
                "tetris" => false,
                "invaders" => {
                    self.animation_state.invaders.is_empty() && self.config.animation.density > 0
                }
                "fibonacci" => false,
                "mandelbrot" => false,
                "hex_grid" => false,
                "rose" => false,
                "butterflies" => {
                    self.animation_state.butterflies.is_empty() && self.config.animation.density > 0
                }
                "spider_web" => {
                    self.animation_state.web_strands.is_empty() && self.config.animation.density > 0
                }
                "vine_growth" => {
                    self.animation_state.vines.is_empty() && self.config.animation.density > 0
                }
                "moss" => self.animation_state.moss.is_empty() && self.config.animation.density > 0,
                "radar" => false,
                "binary_clock" => false,
                "signal" => self.animation_state.signals.is_empty(),
                "wifi" => false,
                "paint_splatter" => false,
                "ink_bleed" => false,
                "mosaic" => self.animation_state.mosaic_tiles.is_empty(),
                "stained_glass" => self.animation_state.glass_panels.is_empty(),
                "hologram" => false,
                "glitch" => false,
                "old_film" => false,
                "thermal" => false,
                _ => false,
            };

            if needs_init {
                self.animation_state.init(&self.config, area);
            }
        }

        // Update based on animation type
        match self.config.animation.animation_type.as_str() {
            "matrix" => self.animation_state.update_matrix(area, &self.config),
            "rain" => self.animation_state.update_rain(area, &self.config),
            "thunder" => self.animation_state.update_thunder(),
            "snow" => self.animation_state.update_snow(area, &self.config),
            "stars" => self.animation_state.update_stars(&self.config),
            "fireflies" => self.animation_state.update_fireflies(area, &self.config),
            "bubbles" => self.animation_state.update_bubbles(area, &self.config),
            "confetti" => self.animation_state.update_confetti(area, &self.config),
            "wave" => self.animation_state.update_wave(),
            "particles" => self.animation_state.update_particles(area, &self.config),
            "digital_rain" => self.animation_state.update_digital_rain(area, &self.config),
            "heartbeat" => self.animation_state.update_heartbeat(),
            "plasma" => self.animation_state.update_plasma(),
            "scanlines" => self.animation_state.update_scanlines(area),
            "aurora" => self.animation_state.update_aurora(),
            "autumn" => self.animation_state.update_autumn(area, &self.config),
            "dna" => self.animation_state.update_dna(area, &self.config),
            "synthwave" => self.animation_state.update_synthwave(),
            "smoke" => self.animation_state.update_smoke(area, &self.config),
            "gradient_flow" => self.animation_state.update_gradient_flow(),
            "constellation" => self
                .animation_state
                .update_constellation(area, &self.config),
            "fish_tank" => self.animation_state.update_fish_tank(area, &self.config),
            "typing_code" => self.animation_state.update_typing_code(),
            "vortex" => self.animation_state.update_vortex(),
            "circuit" => self.animation_state.update_circuit(area, &self.config),
            "flow_field" => self.animation_state.update_flow_field(area, &self.config),
            "morse" => self.animation_state.update_morse(),
            "lissajous" => self.animation_state.update_lissajous(),
            "game_of_life" => self.animation_state.update_game_of_life(),
            "matrix_cjk" => self.animation_state.update_matrix(area, &self.config),
            "fireworks" => self.animation_state.update_fireworks(area),
            "neon_grid" => self.animation_state.update_neon_grid(),
            "perlin_flow" => self.animation_state.update_perlin_flow(),
            "cube_3d" => self.animation_state.update_cube_3d(),
            "fractals" => self.animation_state.update_fractals(),
            // New animations v1.1.5
            "ocean" => self.animation_state.update_ocean(),
            "ripple" => self.animation_state.update_ripple(area, &self.config),
            "fog" => self.animation_state.update_fog(),
            "flames" => self.animation_state.update_flames(area, &self.config),
            "sparks" => self.animation_state.update_sparks(area, &self.config),
            "lava_lamp" => self.animation_state.update_lava_lamp(area, &self.config),
            "sun" => self.animation_state.update_sun(),
            "galaxy" => self.animation_state.update_galaxy(),
            "meteor_shower" => self
                .animation_state
                .update_meteor_shower(area, &self.config),
            "satellite" => self.animation_state.update_satellite(area, &self.config),
            "pulsar" => self.animation_state.update_pulsar(),
            "pong" => self.animation_state.update_pong(area, &self.config),
            "snake" => self.animation_state.update_snake(area, &self.config),
            "tetris" => self.animation_state.update_tetris(area, &self.config),
            "invaders" => self.animation_state.update_invaders(area, &self.config),
            "fibonacci" => self.animation_state.update_fibonacci(),
            "mandelbrot" => self.animation_state.update_mandelbrot(),
            "hex_grid" => self.animation_state.update_hex_grid(),
            "rose" => self.animation_state.update_rose(),
            "butterflies" => self.animation_state.update_butterflies(area, &self.config),
            "spider_web" => self.animation_state.update_spider_web(),
            "vine_growth" => self.animation_state.update_vine_growth(area, &self.config),
            "moss" => self.animation_state.update_moss(area, &self.config),
            "radar" => self.animation_state.update_radar(),
            "binary_clock" => self.animation_state.update_binary_clock(),
            "signal" => self.animation_state.update_signal(area, &self.config),
            "wifi" => self.animation_state.update_wifi(),
            "paint_splatter" => self
                .animation_state
                .update_paint_splatter(area, &self.config),
            "ink_bleed" => self.animation_state.update_ink_bleed(area, &self.config),
            "mosaic" => self.animation_state.update_mosaic(),
            "stained_glass" => self.animation_state.update_stained_glass(),
            "hologram" => self.animation_state.update_hologram(area),
            "glitch" => self.animation_state.update_glitch(),
            "old_film" => self.animation_state.update_old_film(area, &self.config),
            "thermal" => self.animation_state.update_thermal(area),
            _ => {}
        }
    }
}

impl EasterEggState {
    fn new() -> Self {
        Self {
            sequence: Vec::new(),
            konami_code: vec![
                KeyCode::Up,
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Down,
                KeyCode::Left,
                KeyCode::Right,
                KeyCode::Left,
                KeyCode::Right,
                KeyCode::Char('b'),
                KeyCode::Char('a'),
            ],
            rainbow_mode: false,
        }
    }

    fn check_konami(&mut self, key: KeyCode) -> bool {
        self.sequence.push(key);
        // Keep only the last N keys where N is the length of the konami code
        while self.sequence.len() > self.konami_code.len() {
            self.sequence.remove(0);
        }

        if self.sequence == self.konami_code {
            self.rainbow_mode = !self.rainbow_mode;
            self.sequence.clear();
            return true;
        }
        false
    }
}

/// Detect the current window manager
fn detect_wm() -> String {
    // Check environment variables
    if let Ok(wayland_display) = std::env::var("WAYLAND_DISPLAY") {
        if !wayland_display.is_empty() {
            // It's Wayland - check for specific compositors
            if let Ok(hyprland) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
                if !hyprland.is_empty() {
                    return "hyprland".to_string();
                }
            }
            if let Ok(sway_sock) = std::env::var("SWAYSOCK") {
                if !sway_sock.is_empty() {
                    return "sway".to_string();
                }
            }
        }
    }

    // Check XDG_SESSION_DESKTOP
    if let Ok(desktop) = std::env::var("XDG_SESSION_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();
        if desktop_lower.contains("hyprland") {
            return "hyprland".to_string();
        } else if desktop_lower.contains("sway") {
            return "sway".to_string();
        } else if desktop_lower.contains("i3") {
            return "i3".to_string();
        } else if desktop_lower.contains("bspwm") {
            return "bspwm".to_string();
        } else if desktop_lower.contains("awesome") {
            return "awesome".to_string();
        }
    }

    // Check XDG_CURRENT_DESKTOP
    if let Ok(current) = std::env::var("XDG_CURRENT_DESKTOP") {
        let current_lower = current.to_lowercase();
        if current_lower.contains("hyprland") {
            return "hyprland".to_string();
        } else if current_lower.contains("sway") {
            return "sway".to_string();
        }
    }

    // Default to hyprland if we can't detect
    "hyprland".to_string()
}

/// Get the logout command for a specific window manager
fn get_logout_command(wm: &str) -> (String, Vec<String>) {
    match wm {
        "sway" => ("swaymsg".to_string(), vec!["exit".to_string()]),
        "i3" => ("i3-msg".to_string(), vec!["exit".to_string()]),
        "bspwm" => ("bspc".to_string(), vec!["quit".to_string()]),
        "awesome" => (
            "awesome-client".to_string(),
            vec!["awesome.quit()".to_string()],
        ),
        _ => (
            "hyprctl".to_string(),
            vec!["dispatch".to_string(), "exit".to_string()],
        ),
    }
}

impl AnimationState {
    fn new() -> Self {
        Self {
            tick: 0,
            matrix_columns: Vec::new(),
            rain_drops: Vec::new(),
            snow_flakes: Vec::new(),
            stars: Vec::new(),
            fireflies: Vec::new(),
            bubbles: Vec::new(),
            confetti: Vec::new(),
            wave_offset: 0.0,
            particles: Vec::new(),
            plasma: Vec::new(),
            scanline_pos: 0,
            aurora_phase: 0.0,
            leaves: Vec::new(),
            dna: Vec::new(),
            synthwave_offset: 0.0,
            smoke: Vec::new(),
            gradient_phase: 0.0,
            nodes: Vec::new(),
            fish: Vec::new(),
            code_lines: Vec::new(),
            code_line_idx: 0,
            code_char_idx: 0,
            vortex_angle: 0.0,
            traces: Vec::new(),
            flow_particles: Vec::new(),
            morse_message: String::new(),
            morse_idx: 0,
            morse_timer: 0,
            morse_display: String::new(),
            lissajous: Vec::new(),
            gol_grid: Vec::new(),
            gol_width: 0,
            gol_height: 0,
            thunder_flash: 0,
            heartbeat_phase: 0.0,
            fireworks: Vec::new(),
            neon_offset: 0.0,
            perlin_offset: 0.0,
            cube_rotation: CubeRotation {
                angle_x: 0.0,
                angle_y: 0.0,
                angle_z: 0.0,
            },
            fractal_offset: (0.0, 0.0),
            // New animations v1.1.5
            ocean_phase: 0.0,
            ripple_radius: 0.0,
            fog_density: 0.5,
            flames: Vec::new(),
            sparks: Vec::new(),
            lava_blobs: Vec::new(),
            sun_phase: 0.0,
            galaxy_angle: 0.0,
            meteors: Vec::new(),
            satellite: Satellite {
                x: 0.0,
                y: 0.0,
                angle: 0.0,
                orbit_radius: 10.0,
                signal_timer: 0,
            },
            pulsar_angle: 0.0,
            pong: PongGame {
                ball_x: 40.0,
                ball_y: 12.0,
                ball_vx: 0.5,
                ball_vy: 0.3,
                paddle1_y: 10.0,
                paddle2_y: 10.0,
                score1: 0,
                score2: 0,
            },
            snake: SnakeGame {
                segments: Vec::new(),
                direction: 1,
                food: (20, 10),
                tick_count: 0,
            },
            tetris: TetrisGame {
                pieces: Vec::new(),
                falling_piece: None,
                tick_count: 0,
            },
            invaders: Vec::new(),
            fibonacci_angle: 0.0,
            mandelbrot_offset: (-0.5, 0.0),
            hex_phase: 0.0,
            rose_angle: 0.0,
            butterflies: Vec::new(),
            web_strands: Vec::new(),
            vines: Vec::new(),
            moss: Vec::new(),
            radar_angle: 0.0,
            binary_time: 0,
            signals: Vec::new(),
            wifi_waves: Vec::new(),
            splatters: Vec::new(),
            ink_drops: Vec::new(),
            mosaic_tiles: Vec::new(),
            glass_panels: Vec::new(),
            hologram_line: 0,
            glitch_timer: 0,
            scratches: Vec::new(),
            thermal_noise: Vec::new(),
            last_update: std::time::Instant::now(),
        }
    }

    fn init(&mut self, config: &Config, area: Rect) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        match config.animation.animation_type.as_str() {
            "matrix" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 100).max(1);
                self.matrix_columns.clear();
                for _ in 0..count {
                    self.matrix_columns.push(MatrixColumn {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0.0..area.height as f32),
                        speed: rng.gen_range(0.2..1.5),
                        char_idx: rng.gen_range(0..MATRIX_CHARS.len()),
                    });
                }
            }
            "rain" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 10).max(5);
                self.rain_drops.clear();
                for _ in 0..count {
                    self.rain_drops.push(RainDrop {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0.0..area.height as f32),
                        speed: rng.gen_range(0.5..2.5),
                        length: rng.gen_range(2..6),
                    });
                }
            }
            "snow" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 500).max(10);
                self.snow_flakes.clear();
                for _ in 0..count {
                    self.snow_flakes.push(SnowFlake {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(0.0..area.height as f32),
                        speed: rng.gen_range(0.1..0.5),
                        size: rng.gen_range(1..3),
                    });
                }
            }
            "stars" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 300).max(5);
                self.stars.clear();
                for _ in 0..count {
                    self.stars.push(Star {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0..area.height),
                        brightness: rng.gen_range(50..255),
                        twinkle_speed: rng.gen_range(0.05..0.2),
                        twinkle_offset: rng.gen_range(0.0..std::f32::consts::TAU),
                    });
                }
            }
            "fireflies" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 800).max(3);
                self.fireflies.clear();
                for _ in 0..count {
                    self.fireflies.push(Firefly {
                        x: rng.gen_range(2.0..(area.width.saturating_sub(2)) as f32),
                        y: rng.gen_range(2.0..(area.height.saturating_sub(2)) as f32),
                        dx: rng.gen_range(-0.3..0.3),
                        dy: rng.gen_range(-0.3..0.3),
                        brightness: rng.gen_range(100..255),
                    });
                }
            }
            "bubbles" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 20).max(3);
                self.bubbles.clear();
                for _ in 0..count {
                    self.bubbles.push(Bubble {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(area.height as f32..(area.height as f32 * 2.0)),
                        speed: rng.gen_range(0.1..0.5),
                        size: rng.gen_range(1..4),
                        wobble: rng.gen_range(0.0..std::f32::consts::TAU),
                    });
                }
            }
            "confetti" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 10).max(5);
                self.confetti.clear();
                for _ in 0..count {
                    self.confetti.push(Confetti {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(-10.0..0.0),
                        dx: rng.gen_range(-0.5..0.5),
                        dy: rng.gen_range(0.5..2.0),
                        color: rng.gen_range(0..255),
                        rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                        rotation_speed: rng.gen_range(-0.2..0.2),
                        character: ['■', '▲', '●', '◆', '★'][rng.gen_range(0..5)],
                    });
                }
            }
            "wave" => {
                self.wave_offset = 0.0;
            }
            "particles" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 400).max(10);
                self.particles.clear();
                for _ in 0..count {
                    self.particles.push(Particle {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(0.0..area.height as f32),
                        dx: rng.gen_range(-0.5..0.5),
                        dy: rng.gen_range(-0.5..0.5),
                        life: rng.gen_range(50..150),
                        max_life: 150,
                        color: Color::Rgb(
                            rng.gen_range(100..255),
                            rng.gen_range(100..255),
                            rng.gen_range(100..255),
                        ),
                    });
                }
            }
            "digital_rain" => {
                // Similar to matrix but with binary/hex
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 100).max(1);
                self.matrix_columns.clear();
                for _ in 0..count {
                    self.matrix_columns.push(MatrixColumn {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0.0..area.height as f32),
                        speed: rng.gen_range(0.2..1.5),
                        char_idx: rng.gen_range(0..16), // 0-F for hex
                    });
                }
            }
            "heartbeat" => {
                self.heartbeat_phase = 0.0;
            }
            "plasma" => {
                self.plasma.clear();
                for y in 0..area.height {
                    for x in 0..area.width {
                        self.plasma.push(PlasmaCell {
                            x,
                            y,
                            value: rng.gen_range(0.0..1.0),
                        });
                    }
                }
            }
            "scanlines" => {
                self.scanline_pos = 0;
            }
            "aurora" => {
                self.aurora_phase = 0.0;
            }
            "autumn" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 15).max(5);
                self.leaves.clear();
                for _ in 0..count {
                    self.leaves.push(Leaf {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(0.0..area.height as f32),
                        rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                        rotation_speed: rng.gen_range(-0.1..0.1),
                        speed: rng.gen_range(0.1..0.4),
                        color: rng.gen_range(0..4), // Different autumn colors
                    });
                }
            }
            "dna" => {
                self.dna.clear();
                for y in (0..area.height.saturating_sub(4)).step_by(2) {
                    let bases = [('A', 'T'), ('T', 'A'), ('C', 'G'), ('G', 'C')];
                    let (left, right) = bases[rng.gen_range(0..4)];
                    self.dna.push(DnaBase {
                        y: y as f32,
                        left_char: left,
                        right_char: right,
                        connection: rng.gen_bool(0.7),
                    });
                }
            }
            "synthwave" => {
                self.synthwave_offset = 0.0;
            }
            "smoke" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 20).max(3);
                self.smoke.clear();
                for _ in 0..count {
                    self.smoke.push(Particle {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: area.height as f32 + rng.gen_range(0.0..10.0),
                        dx: rng.gen_range(-0.2..0.2),
                        dy: rng.gen_range(-0.3..-0.1),
                        life: rng.gen_range(100..200),
                        max_life: 200,
                        color: Color::Rgb(100, 100, 100),
                    });
                }
            }
            "gradient_flow" => {
                self.gradient_phase = 0.0;
            }
            "constellation" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 1000).max(5);
                self.nodes.clear();
                for _ in 0..count {
                    self.nodes.push(ConstellationNode {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(0.0..area.height as f32),
                        dx: rng.gen_range(-0.3..0.3),
                        dy: rng.gen_range(-0.3..0.3),
                    });
                }
            }
            "fish_tank" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 30).max(2);
                self.fish.clear();
                for _ in 0..count {
                    self.fish.push(Fish {
                        x: rng.gen_range(5.0..(area.width.saturating_sub(5)) as f32),
                        y: rng.gen_range(2.0..(area.height.saturating_sub(2)) as f32),
                        dx: rng.gen_range(0.1..0.4),
                        direction: rng.gen_bool(0.5),
                        color: rng.gen_range(0..6),
                    });
                }
            }
            "typing_code" => {
                self.code_lines = vec![
                    "fn main() {".to_string(),
                    "    let mut app = App::new();".to_string(),
                    "    app.run();".to_string(),
                    "}".to_string(),
                    "use std::io;".to_string(),
                    "impl Drop for Resource {".to_string(),
                    "    fn drop(&mut self) {".to_string(),
                    "        self.cleanup();".to_string(),
                    "    }".to_string(),
                    "}".to_string(),
                    "#[derive(Debug)]".to_string(),
                    "struct Config {".to_string(),
                    "    value: String,".to_string(),
                    "}".to_string(),
                    "mod utils;".to_string(),
                    "pub async fn fetch() -> Result<()> {".to_string(),
                    "    Ok(())".to_string(),
                    "}".to_string(),
                    "const MAX_SIZE: usize = 1024;".to_string(),
                ];
                self.code_line_idx = 0;
                self.code_char_idx = 0;
            }
            "vortex" => {
                self.vortex_angle = 0.0;
            }
            "circuit" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 25).max(3);
                self.traces.clear();
                for _ in 0..count {
                    self.traces.push(CircuitTrace {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0..area.height),
                        direction: rng.gen_range(0..4),
                        life: rng.gen_range(50..150),
                    });
                }
            }
            "flow_field" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 500).max(10);
                self.flow_particles.clear();
                for _ in 0..count {
                    self.flow_particles.push(FlowParticle {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(0.0..area.height as f32),
                        vx: 0.0,
                        vy: 0.0,
                        color: rng.gen_range(0..255),
                    });
                }
            }
            "morse" => {
                self.morse_message = "I'd just like to interject for a moment. What you're refering to as Linux, is in fact, GNU/Linux, or as I've recently taken to calling it, GNU plus Linux. Linux is not an operating system unto itself, but rather another free component of a fully functioning GNU system made useful by the GNU corelibs, shell utilities and vital system components comprising a full OS as defined by POSIX. Many computer users run a modified version of the GNU system every day, without realizing it. Through a peculiar turn of events, the version of GNU which is widely used today is often called Linux, and many of its users are not aware that it is basically the GNU system, developed by the GNU Project. There really is a Linux, and these people are using it, but it is just a part of the system they use. Linux is the kernel: the program in the system that allocates the machine's resources to the other programs that you run. The kernel is an essential part of an operating system, but useless by itself; it can only function in the context of a complete operating system. Linux is normally used in combination with the GNU operating system: the whole system is basically GNU with Linux added, or GNU/Linux. All the so-called Linux distributions are really distributions of GNU/Linux!".to_string();
                self.morse_idx = 0;
                self.morse_timer = 0;
                self.morse_display = String::new();
            }
            "lissajous" => {
                self.lissajous.clear();
                for i in 0..5 {
                    self.lissajous.push(LissajousCurve {
                        a: 3.0 + i as f32,
                        b: 2.0 + i as f32 * 0.5,
                        delta: i as f32 * 0.5,
                        t: 0.0,
                        color: (i * 50) as u8,
                    });
                }
            }
            "game_of_life" => {
                self.gol_width = area.width as usize;
                self.gol_height = area.height as usize;
                self.gol_grid.clear();
                for y in 0..self.gol_height {
                    for x in 0..self.gol_width {
                        self.gol_grid.push(GameOfLifeCell {
                            x,
                            y,
                            alive: rng.gen_bool(0.3),
                            next_state: false,
                            age: 0,
                        });
                    }
                }
            }
            "matrix_cjk" => {
                let density = config.animation.density as usize;
                let count = ((area.width as usize * density) / 100).max(1);
                self.matrix_columns.clear();
                for _ in 0..count {
                    self.matrix_columns.push(MatrixColumn {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0.0..area.height as f32),
                        speed: rng.gen_range(0.2..1.5),
                        char_idx: rng.gen_range(0..256),
                    });
                }
            }
            "fireworks" => {
                self.fireworks.clear();
                self.fireworks.push(Firework {
                    x: area.width as f32 / 2.0,
                    y: area.height as f32,
                    vx: rng.gen_range(-1.0..1.0),
                    vy: rng.gen_range(-3.0..-2.0),
                    particles: Vec::new(),
                    exploded: false,
                    life: 100,
                    color: (255, 100, 50),
                });
            }
            "neon_grid" => {
                self.neon_offset = 0.0;
            }
            "perlin_flow" => {
                self.perlin_offset = 0.0;
            }
            "cube_3d" => {
                self.cube_rotation = CubeRotation {
                    angle_x: 0.0,
                    angle_y: 0.0,
                    angle_z: 0.0,
                };
            }
            "fractals" => {
                self.fractal_offset = (0.0, 0.0);
            }
            // New animations v1.1.5
            "ocean" => {
                self.ocean_phase = 0.0;
            }
            "ripple" => {
                self.ripple_radius = 0.0;
            }
            "fog" => {
                self.fog_density = 0.5;
            }
            "flames" => {
                let density = config.animation.density as usize;
                let count = (density / 2).max(5);
                self.flames.clear();
                for _ in 0..count {
                    self.flames.push(FlameParticle {
                        x: rng.gen_range(0.0..area.width as f32),
                        _y: area.height as f32,
                        height: rng.gen_range(3.0..10.0),
                        intensity: rng.gen_range(150..255),
                    });
                }
            }
            "sparks" => {
                let density = config.animation.density as usize;
                let count = (density / 3).max(3);
                self.sparks.clear();
                for _ in 0..count {
                    self.sparks.push(Spark {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(area.height as f32 / 2.0..area.height as f32),
                        vx: rng.gen_range(-0.5..0.5),
                        vy: rng.gen_range(-2.0..-0.5),
                        life: rng.gen_range(20..60),
                        brightness: rng.gen_range(200..255),
                    });
                }
            }
            "lava_lamp" => {
                let density = config.animation.density as usize;
                let count = (density / 10).max(2);
                self.lava_blobs.clear();
                for _ in 0..count {
                    self.lava_blobs.push(LavaBlob {
                        x: rng.gen_range(5.0..(area.width.saturating_sub(5)) as f32),
                        y: rng.gen_range(5.0..(area.height.saturating_sub(5)) as f32),
                        size: rng.gen_range(2.0..5.0),
                        dy: rng.gen_range(-0.1..0.1),
                        color_phase: rng.gen_range(0.0..std::f32::consts::TAU),
                    });
                }
            }
            "sun" => {
                self.sun_phase = 0.0;
            }
            "galaxy" => {
                self.galaxy_angle = 0.0;
            }
            "meteor_shower" => {
                let density = config.animation.density as usize;
                let count = (density / 5).max(2);
                self.meteors.clear();
                for _ in 0..count {
                    self.meteors.push(Meteor {
                        x: rng.gen_range(0.0..area.width as f32),
                        y: rng.gen_range(0.0..(area.height / 2) as f32),
                        vx: rng.gen_range(-1.0..1.0),
                        vy: rng.gen_range(0.5..2.0),
                        tail_length: rng.gen_range(3..8),
                        brightness: rng.gen_range(200..255),
                    });
                }
            }
            "satellite" => {
                self.satellite = Satellite {
                    x: area.width as f32 / 2.0,
                    y: area.height as f32 / 2.0,
                    angle: 0.0,
                    orbit_radius: (area.width.min(area.height) as f32 / 3.0).min(15.0),
                    signal_timer: 0,
                };
            }
            "pulsar" => {
                self.pulsar_angle = 0.0;
            }
            "pong" => {
                self.pong = PongGame {
                    ball_x: area.width as f32 / 2.0,
                    ball_y: area.height as f32 / 2.0,
                    ball_vx: if rng.gen_bool(0.5) { 0.8 } else { -0.8 },
                    ball_vy: if rng.gen_bool(0.5) { 0.5 } else { -0.5 },
                    paddle1_y: area.height as f32 / 2.0,
                    paddle2_y: area.height as f32 / 2.0,
                    score1: 0,
                    score2: 0,
                };
            }
            "snake" => {
                let start_x = area.width / 2;
                let start_y = area.height / 2;
                self.snake = SnakeGame {
                    segments: vec![
                        (start_x, start_y),
                        (start_x - 1, start_y),
                        (start_x - 2, start_y),
                    ],
                    direction: 1,
                    food: (
                        rng.gen_range(5..area.width - 5),
                        rng.gen_range(3..area.height - 3),
                    ),
                    tick_count: 0,
                };
            }
            "tetris" => {
                self.tetris = TetrisGame {
                    pieces: Vec::new(),
                    falling_piece: Some((area.width / 2, 0, rng.gen_range(0..7))),
                    tick_count: 0,
                };
            }
            "invaders" => {
                let density = config.animation.density as usize;
                let rows = 3;
                let cols = (density / 10).max(3).min(8);
                self.invaders.clear();
                for row in 0..rows {
                    for col in 0..cols {
                        self.invaders.push(Invader {
                            x: (5 + col * 6) as f32,
                            y: (2 + row * 3) as f32,
                            invader_type: (row as u8) % 3,
                            direction: 1,
                            anim_frame: false,
                        });
                    }
                }
            }
            "fibonacci" => {
                self.fibonacci_angle = 0.0;
            }
            "mandelbrot" => {
                self.mandelbrot_offset = (-0.5, 0.0);
            }
            "hex_grid" => {
                self.hex_phase = 0.0;
            }
            "rose" => {
                self.rose_angle = 0.0;
            }
            "butterflies" => {
                let density = config.animation.density as usize;
                let count = (density / 5).max(3);
                self.butterflies.clear();
                for _ in 0..count {
                    self.butterflies.push(Butterfly {
                        x: rng.gen_range(5.0..(area.width.saturating_sub(5)) as f32),
                        y: rng.gen_range(3.0..(area.height.saturating_sub(3)) as f32),
                        target_x: rng.gen_range(5.0..(area.width.saturating_sub(5)) as f32),
                        target_y: rng.gen_range(3.0..(area.height.saturating_sub(3)) as f32),
                        wing_open: true,
                        color: rng.gen_range(0..255),
                    });
                }
            }
            "spider_web" => {
                self.web_strands.clear();
                let center_x = area.width as f32 / 2.0;
                let center_y = area.height as f32 / 2.0;
                let radius = (area.width.min(area.height) as f32 / 3.0).min(12.0);
                // Radial strands
                for i in 0..8 {
                    let angle = (i as f32) * std::f32::consts::PI / 4.0;
                    self.web_strands.push(WebStrand {
                        x1: center_x,
                        y1: center_y,
                        x2: center_x + angle.cos() * radius,
                        y2: center_y + angle.sin() * radius,
                        vibration: 0.0,
                    });
                }
                // Spiral strands
                for r in (2..=radius as i32).step_by(3) {
                    let r = r as f32;
                    for i in 0..8 {
                        let angle1 = (i as f32) * std::f32::consts::PI / 4.0;
                        let angle2 = ((i + 1) as f32) * std::f32::consts::PI / 4.0;
                        self.web_strands.push(WebStrand {
                            x1: center_x + angle1.cos() * r,
                            y1: center_y + angle1.sin() * r,
                            x2: center_x + angle2.cos() * r,
                            y2: center_y + angle2.sin() * r,
                            vibration: 0.0,
                        });
                    }
                }
            }
            "vine_growth" => {
                self.vines.clear();
                let density = config.animation.density as usize;
                let count = (density / 15).max(2);
                for _ in 0..count {
                    self.vines.push(Vine {
                        x: rng.gen_range(0.0..area.width as f32),
                        _y: area.height as f32,
                        length: 0,
                        growth_rate: rng.gen_range(0.1..0.3),
                        max_length: rng.gen_range(10..area.height.min(30)),
                    });
                }
            }
            "moss" => {
                self.moss.clear();
                let density = config.animation.density as usize;
                let count = ((area.width as usize * area.height as usize * density) / 2000).max(10);
                for _ in 0..count {
                    self.moss.push(MossCell {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range((area.height / 2)..area.height),
                        age: rng.gen_range(0..50),
                        spreading: rng.gen_bool(0.3),
                    });
                }
            }
            "radar" => {
                self.radar_angle = 0.0;
            }
            "binary_clock" => {
                self.binary_time = 0;
            }
            "signal" => {
                self.signals.clear();
                let density = config.animation.density as usize;
                let count = (density / 10).max(2);
                for _ in 0..count {
                    self.signals.push(SignalWave {
                        x: rng.gen_range(5..area.width - 5),
                        y: area.height / 2,
                        radius: 0.0,
                        max_radius: rng.gen_range(10.0..30.0),
                        amplitude: rng.gen_range(100..200),
                    });
                }
            }
            "wifi" => {
                self.wifi_waves.clear();
                for i in 0..3 {
                    self.wifi_waves.push(WifiWave {
                        _center_x: area.width as f32 / 2.0,
                        _center_y: area.height as f32 / 2.0,
                        radius: i as f32 * 3.0,
                        intensity: 200 - (i * 50) as u8,
                    });
                }
            }
            "paint_splatter" => {
                self.splatters.clear();
            }
            "ink_bleed" => {
                self.ink_drops.clear();
            }
            "mosaic" => {
                self.mosaic_tiles.clear();
                let tile_size = 4u16;
                for y in (0..area.height).step_by(tile_size as usize) {
                    for x in (0..area.width).step_by(tile_size as usize) {
                        self.mosaic_tiles.push(MosaicTile {
                            x,
                            y,
                            color: (
                                rng.gen_range(50..200),
                                rng.gen_range(50..200),
                                rng.gen_range(50..200),
                            ),
                            changing: false,
                            change_timer: 0,
                        });
                    }
                }
            }
            "stained_glass" => {
                self.glass_panels.clear();
                let panel_width = area.width / 4;
                let panel_height = area.height / 3;
                for row in 0..3 {
                    for col in 0..4 {
                        self.glass_panels.push(GlassPanel {
                            x: col * panel_width,
                            y: row * panel_height,
                            width: panel_width,
                            height: panel_height,
                            hue: rng.gen_range(0..255),
                            light_intensity: rng.gen_range(100..200),
                        });
                    }
                }
            }
            "hologram" => {
                self.hologram_line = 0;
            }
            "glitch" => {
                self.glitch_timer = 0;
            }
            "old_film" => {
                self.scratches.clear();
                for _ in 0..5 {
                    self.scratches.push(FilmScratch {
                        x: rng.gen_range(0..area.width),
                        y: rng.gen_range(0..area.height),
                        length: rng.gen_range(3..10),
                        visible: rng.gen_bool(0.5),
                    });
                }
            }
            "thermal" => {
                self.thermal_noise.clear();
                let count = (area.width * area.height) as usize;
                self.thermal_noise = (0..count).map(|_| rng.gen_range(0..255)).collect();
            }
            _ => {}
        }
    }

    fn update_matrix(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for col in &mut self.matrix_columns {
            col.y += col.speed;
            if col.y >= area.height as f32 {
                col.y = 0.0;
                col.x = rng.gen_range(0..area.width);
                col.speed = rng.gen_range(0.2..1.5);
            }
            if self.tick.is_multiple_of(3) {
                col.char_idx = rng.gen_range(0..MATRIX_CHARS.len());
            }
        }

        // Randomly respawn columns to maintain density
        let target_count = ((area.width as usize * config.animation.density as usize) / 100).max(1);
        while self.matrix_columns.len() < target_count {
            self.matrix_columns.push(MatrixColumn {
                x: rng.gen_range(0..area.width),
                y: 0.0,
                speed: rng.gen_range(0.2..1.5),
                char_idx: rng.gen_range(0..MATRIX_CHARS.len()),
            });
        }
    }

    fn update_rain(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for drop in &mut self.rain_drops {
            drop.y += drop.speed;
            if drop.y >= area.height as f32 + drop.length as f32 {
                drop.y = -(drop.length as f32);
                drop.x = rng.gen_range(0..area.width);
            }
        }

        let target_count = ((area.width as usize * config.animation.density as usize) / 10).max(5);
        while self.rain_drops.len() < target_count {
            self.rain_drops.push(RainDrop {
                x: rng.gen_range(0..area.width),
                y: rng.gen_range(-10.0..0.0),
                speed: rng.gen_range(0.5..2.5),
                length: rng.gen_range(2..6),
            });
        }
    }

    fn update_thunder(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Random thunder flashes
        if self.thunder_flash > 0 {
            self.thunder_flash -= 1;
        } else if rng.gen_bool(0.02) {
            self.thunder_flash = rng.gen_range(2..5);
        }
    }

    fn update_snow(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for flake in &mut self.snow_flakes {
            flake.y += flake.speed;
            flake.x += rng.gen_range(-0.3..0.3); // Slight horizontal drift

            if flake.y >= area.height as f32 {
                flake.y = 0.0;
                flake.x = rng.gen_range(0.0..area.width as f32);
            }
            if flake.x < 0.0 {
                flake.x = area.width as f32 - 1.0;
            } else if flake.x >= area.width as f32 {
                flake.x = 0.0;
            }
        }

        let target_count =
            ((area.width as usize * area.height as usize * config.animation.density as usize)
                / 500)
                .max(10);
        while self.snow_flakes.len() < target_count {
            self.snow_flakes.push(SnowFlake {
                x: rng.gen_range(0.0..area.width as f32),
                y: rng.gen_range(0.0..area.height as f32),
                speed: rng.gen_range(0.1..0.5),
                size: rng.gen_range(1..3),
            });
        }
    }

    fn update_stars(&mut self, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for star in &mut self.stars {
            let twinkle = (self.tick as f32 * star.twinkle_speed + star.twinkle_offset).sin();
            star.brightness = ((twinkle + 1.0) * 100.0 + 50.0) as u8;
        }

        // Occasionally add/remove stars
        if self.tick.is_multiple_of(60) && rng.gen_bool(0.1) {
            let target_count = ((200 * config.animation.density as usize) / 100).max(5);
            if self.stars.len() < target_count && !self.stars.is_empty() {
                self.stars.push(Star {
                    x: rng.gen_range(0..200),
                    y: rng.gen_range(0..60),
                    brightness: rng.gen_range(50..255),
                    twinkle_speed: rng.gen_range(0.05..0.2),
                    twinkle_offset: rng.gen_range(0.0..std::f32::consts::TAU),
                });
            }
        }
    }

    fn update_fireflies(&mut self, area: Rect, _config: &Config) {
        for firefly in &mut self.fireflies {
            firefly.x += firefly.dx;
            firefly.y += firefly.dy;

            // Bounce off edges
            if firefly.x <= 1.0 || firefly.x >= area.width.saturating_sub(2) as f32 {
                firefly.dx = -firefly.dx;
                firefly.x = firefly.x.clamp(1.0, area.width.saturating_sub(2) as f32);
            }
            if firefly.y <= 1.0 || firefly.y >= area.height.saturating_sub(2) as f32 {
                firefly.dy = -firefly.dy;
                firefly.y = firefly.y.clamp(1.0, area.height.saturating_sub(2) as f32);
            }

            // Pulse brightness
            let pulse = (self.tick as f32 * 0.1).sin();
            firefly.brightness = ((pulse + 1.0) * 75.0 + 50.0) as u8;
        }
    }

    fn update_digital_rain(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for col in &mut self.matrix_columns {
            col.y += col.speed;
            if col.y >= area.height as f32 {
                col.y = 0.0;
                col.x = rng.gen_range(0..area.width);
                col.speed = rng.gen_range(0.2..1.5);
            }
            if self.tick.is_multiple_of(3) {
                col.char_idx = rng.gen_range(0..16);
            }
        }

        let target_count = ((area.width as usize * config.animation.density as usize) / 100).max(1);
        while self.matrix_columns.len() < target_count {
            self.matrix_columns.push(MatrixColumn {
                x: rng.gen_range(0..area.width),
                y: 0.0,
                speed: rng.gen_range(0.2..1.5),
                char_idx: rng.gen_range(0..16),
            });
        }
    }

    fn update_heartbeat(&mut self) {
        self.heartbeat_phase += 0.1;
    }

    fn update_plasma(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for cell in &mut self.plasma {
            // Update plasma value based on position and time
            let t = self.tick as f32 * 0.05;
            let x = cell.x as f32;
            let y = cell.y as f32;
            cell.value =
                ((x * 0.1 + t).sin() + (y * 0.1 + t).cos() + ((x + y) * 0.05 + t * 0.5).sin())
                    / 3.0;

            // Add some randomness
            if rng.gen_bool(0.01) {
                cell.value = cell.value * 0.9 + rng.gen_range(0.0..1.0) * 0.1;
            }
        }
    }

    fn update_scanlines(&mut self, area: Rect) {
        self.scanline_pos = (self.scanline_pos + 1) % area.height;
    }

    fn update_aurora(&mut self) {
        self.aurora_phase += 0.02;
    }

    fn update_autumn(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for leaf in &mut self.leaves {
            leaf.y += leaf.speed;
            leaf.x += (self.tick as f32 * 0.05 + leaf.y * 0.1).sin() * 0.3;
            leaf.rotation += leaf.rotation_speed;

            // Reset if fell below screen
            if leaf.y > area.height as f32 {
                leaf.y = -2.0;
                leaf.x = rng.gen_range(0.0..area.width as f32);
                leaf.rotation = rng.gen_range(0.0..std::f32::consts::TAU);
            }
        }

        let target_count = ((area.width as usize * config.animation.density as usize) / 15).max(5);
        while self.leaves.len() < target_count {
            self.leaves.push(Leaf {
                x: rng.gen_range(0.0..area.width as f32),
                y: rng.gen_range(-10.0..0.0),
                rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                rotation_speed: rng.gen_range(-0.1..0.1),
                speed: rng.gen_range(0.1..0.4),
                color: rng.gen_range(0..4),
            });
        }
    }

    fn update_dna(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Move DNA up
        for base in &mut self.dna {
            base.y -= 0.2;
        }

        // Remove bases that went off screen and add new ones at bottom
        self.dna.retain(|b| b.y > -2.0);

        while self.dna.len() < (area.height as usize / 2) {
            let last_y = self.dna.last().map(|b| b.y).unwrap_or(area.height as f32);
            if last_y < area.height as f32 - 2.0 {
                let bases = [('A', 'T'), ('T', 'A'), ('C', 'G'), ('G', 'C')];
                let (left, right) = bases[rng.gen_range(0..4)];
                self.dna.push(DnaBase {
                    y: area.height as f32,
                    left_char: left,
                    right_char: right,
                    connection: rng.gen_bool(0.7),
                });
            } else {
                break;
            }
        }
    }

    fn update_synthwave(&mut self) {
        self.synthwave_offset += 0.5;
    }

    fn update_smoke(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for particle in &mut self.smoke {
            particle.x += particle.dx;
            particle.y += particle.dy;
            particle.life = particle.life.saturating_sub(1);

            // Expand as it rises
            particle.dx += rng.gen_range(-0.01..0.01);

            // Reset if dead
            if particle.life == 0 || particle.y < 0.0 {
                particle.y = area.height as f32 + rng.gen_range(0.0..5.0);
                particle.x = rng.gen_range(0.0..area.width as f32);
                particle.life = rng.gen_range(100..200);
            }
        }

        let target_count = ((area.width as usize * config.animation.density as usize) / 20).max(3);
        while self.smoke.len() < target_count {
            self.smoke.push(Particle {
                x: rng.gen_range(0.0..area.width as f32),
                y: area.height as f32 + rng.gen_range(0.0..10.0),
                dx: rng.gen_range(-0.2..0.2),
                dy: rng.gen_range(-0.3..-0.1),
                life: rng.gen_range(100..200),
                max_life: 200,
                color: Color::Rgb(100, 100, 100),
            });
        }
    }

    fn update_gradient_flow(&mut self) {
        self.gradient_phase += 0.02;
    }

    fn update_constellation(&mut self, area: Rect, _config: &Config) {
        for node in &mut self.nodes {
            node.x += node.dx;
            node.y += node.dy;

            // Bounce off edges
            if node.x <= 0.0 || node.x >= area.width as f32 {
                node.dx = -node.dx;
            }
            if node.y <= 0.0 || node.y >= area.height as f32 {
                node.dy = -node.dy;
            }
        }
    }

    fn update_fish_tank(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for fish in &mut self.fish {
            // Move fish
            if fish.direction {
                fish.x += fish.dx;
            } else {
                fish.x -= fish.dx;
            }

            // Change direction at edges
            if fish.x <= 2.0 {
                fish.direction = true;
                fish.x = 2.0;
            } else if fish.x >= area.width.saturating_sub(2) as f32 {
                fish.direction = false;
                fish.x = area.width.saturating_sub(2) as f32;
            }

            // Slight vertical movement
            fish.y += (self.tick as f32 * 0.05 + fish.x * 0.1).sin() * 0.1;
            fish.y = fish.y.clamp(1.0, area.height.saturating_sub(2) as f32);

            // Random direction change
            if rng.gen_bool(0.01) {
                fish.direction = !fish.direction;
            }
        }

        // Occasionally add bubbles
        if rng.gen_bool(0.05) {
            self.bubbles.push(Bubble {
                x: rng.gen_range(0.0..area.width as f32),
                y: area.height as f32,
                speed: rng.gen_range(0.2..0.5),
                size: 1,
                wobble: rng.gen_range(0.0..std::f32::consts::TAU),
            });
        }
    }

    fn update_typing_code(&mut self) {
        // Type one character every tick for faster animation
        if let Some(line) = self.code_lines.get(self.code_line_idx) {
            if self.code_char_idx < line.len() {
                self.code_char_idx += 1;
            } else {
                // Move to next line
                self.code_line_idx = (self.code_line_idx + 1) % self.code_lines.len();
                self.code_char_idx = 0;
            }
        }
    }

    fn update_vortex(&mut self) {
        self.vortex_angle += 0.1;
    }

    fn update_circuit(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for trace in &mut self.traces {
            // Move in current direction
            match trace.direction {
                0 => trace.y = trace.y.saturating_sub(1),
                1 => trace.x = (trace.x + 1).min(area.width - 1),
                2 => trace.y = (trace.y + 1).min(area.height - 1),
                _ => trace.x = trace.x.saturating_sub(1),
            }

            trace.life = trace.life.saturating_sub(1);

            // Random direction change
            if rng.gen_bool(0.2) {
                trace.direction = rng.gen_range(0..4);
            }
        }

        // Remove dead traces
        self.traces.retain(|t| t.life > 0);

        // Spawn new traces
        let target_count = ((area.width as usize * config.animation.density as usize) / 25).max(3);
        while self.traces.len() < target_count {
            self.traces.push(CircuitTrace {
                x: rng.gen_range(0..area.width),
                y: rng.gen_range(0..area.height),
                direction: rng.gen_range(0..4),
                life: rng.gen_range(50..150),
            });
        }
    }

    fn update_flow_field(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for particle in &mut self.flow_particles {
            // Calculate flow field vector at position
            let scale = 0.05;
            let angle =
                (particle.x * scale).sin() + (particle.y * scale).cos() + self.tick as f32 * 0.02;

            // Update velocity
            let target_vx = angle.cos() * 0.5;
            let target_vy = angle.sin() * 0.5;

            particle.vx += (target_vx - particle.vx) * 0.1;
            particle.vy += (target_vy - particle.vy) * 0.1;

            // Update position
            particle.x += particle.vx;
            particle.y += particle.vy;

            // Wrap around
            if particle.x < 0.0 {
                particle.x = area.width as f32;
            }
            if particle.x > area.width as f32 {
                particle.x = 0.0;
            }
            if particle.y < 0.0 {
                particle.y = area.height as f32;
            }
            if particle.y > area.height as f32 {
                particle.y = 0.0;
            }
        }

        let target_count =
            ((area.width as usize * area.height as usize * config.animation.density as usize)
                / 500)
                .max(10);
        while self.flow_particles.len() < target_count {
            self.flow_particles.push(FlowParticle {
                x: rng.gen_range(0.0..area.width as f32),
                y: rng.gen_range(0.0..area.height as f32),
                vx: 0.0,
                vy: 0.0,
                color: rng.gen_range(0..255),
            });
        }
    }

    fn update_morse(&mut self) {
        // Morse timing: dot=1, dash=3, space=7 (in animation ticks)
        if self.morse_timer > 0 {
            self.morse_timer -= 1;
            return;
        }

        if self.morse_idx < self.morse_message.len() {
            let ch = self.morse_message.chars().nth(self.morse_idx).unwrap();

            // Get morse code for character
            let morse = match ch.to_ascii_uppercase() {
                'A' => ".-",
                'B' => "-...",
                'C' => "-.-.",
                'D' => "-..",
                'E' => ".",
                'F' => "..-.",
                'G' => "--.",
                'H' => "....",
                'I' => "..",
                'J' => ".---",
                'K' => "-.-",
                'L' => ".-..",
                'M' => "--",
                'N' => "-.",
                'O' => "---",
                'P' => ".--.",
                'Q' => "--.-",
                'R' => ".-.",
                'S' => "...",
                'T' => "-",
                'U' => "..-",
                'V' => "...-",
                'W' => ".--",
                'X' => "-..-",
                'Y' => "-.--",
                'Z' => "--..",
                '0' => "-----",
                '1' => ".----",
                '2' => "..---",
                '3' => "...--",
                '4' => "....-",
                '5' => ".....",
                '6' => "-....",
                '7' => "--...",
                '8' => "---..",
                '9' => "----.",
                ' ' => " ",
                _ => "",
            };

            // Build display string
            if ch == ' ' {
                self.morse_display.push_str("  ");
                self.morse_timer = 7; // Space between words
            } else {
                self.morse_display.push_str(morse);
                self.morse_display.push(' ');
                self.morse_timer = 3; // Space between letters
            }

            // Keep display manageable
            if self.morse_display.len() > 200 {
                self.morse_display = self.morse_display.split_off(self.morse_display.len() - 150);
            }

            self.morse_idx += 1;
        } else {
            // Loop back to start
            self.morse_idx = 0;
            self.morse_display.clear();
            self.morse_timer = 14; // Pause before restarting
        }
    }

    fn update_lissajous(&mut self) {
        for curve in &mut self.lissajous {
            curve.t += 0.05;
        }
    }

    fn update_game_of_life(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let width = self.gol_width;
        let height = self.gol_height;

        if width == 0 || height == 0 {
            return;
        }

        // Calculate next state for all cells
        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                let cell = &self.gol_grid[idx];

                // Count neighbors
                let mut neighbors = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        let nx = (x as isize + dx).rem_euclid(width as isize) as usize;
                        let ny = (y as isize + dy).rem_euclid(height as isize) as usize;
                        let nidx = ny * width + nx;

                        if self.gol_grid[nidx].alive {
                            neighbors += 1;
                        }
                    }
                }

                // Apply rules
                let next_alive = if cell.alive {
                    neighbors == 2 || neighbors == 3
                } else {
                    neighbors == 3
                };

                self.gol_grid[idx].next_state = next_alive;
            }
        }

        // Apply next state and update age
        for cell in &mut self.gol_grid {
            if cell.alive && cell.next_state {
                cell.age = cell.age + 1;
            } else if cell.next_state {
                cell.age = 0;
            }
            cell.alive = cell.next_state;
        }

        // Randomly seed new cells to prevent stagnation
        if self.tick.is_multiple_of(100) && rng.gen_bool(0.3) {
            for _ in 0..10 {
                let x = rng.gen_range(0..width);
                let y = rng.gen_range(0..height);
                let idx = y * width + x;
                self.gol_grid[idx].alive = true;
                self.gol_grid[idx].age = 0;
            }
        }
    }

    fn update_bubbles(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for bubble in &mut self.bubbles {
            bubble.y -= bubble.speed;
            bubble.wobble += 0.05;
            bubble.x += bubble.wobble.sin() * 0.3;

            // Reset if reached top
            if bubble.y < 0.0 {
                bubble.y = area.height as f32 + bubble.size as f32;
                bubble.x = rng.gen_range(0.0..area.width as f32);
                bubble.speed = rng.gen_range(0.1..0.5);
            }
        }

        let target_count = ((area.width as usize * config.animation.density as usize) / 20).max(3);
        while self.bubbles.len() < target_count {
            self.bubbles.push(Bubble {
                x: rng.gen_range(0.0..area.width as f32),
                y: area.height as f32 + rng.gen_range(0.0..10.0),
                speed: rng.gen_range(0.1..0.5),
                size: rng.gen_range(1..4),
                wobble: rng.gen_range(0.0..std::f32::consts::TAU),
            });
        }
    }

    fn update_confetti(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for conf in &mut self.confetti {
            conf.x += conf.dx;
            conf.y += conf.dy;
            conf.rotation += conf.rotation_speed;

            // Slight drift
            conf.dx += rng.gen_range(-0.05..0.05);
            conf.dx = conf.dx.clamp(-1.0, 1.0);

            // Reset if fell below screen
            if conf.y > area.height as f32 {
                conf.y = -2.0;
                conf.x = rng.gen_range(0.0..area.width as f32);
                conf.dy = rng.gen_range(0.5..2.0);
                conf.color = rng.gen_range(0..255);
            }
        }

        let target_count = ((area.width as usize * config.animation.density as usize) / 10).max(5);
        while self.confetti.len() < target_count {
            self.confetti.push(Confetti {
                x: rng.gen_range(0.0..area.width as f32),
                y: rng.gen_range(-10.0..0.0),
                dx: rng.gen_range(-0.5..0.5),
                dy: rng.gen_range(0.5..2.0),
                color: rng.gen_range(0..255),
                rotation: rng.gen_range(0.0..std::f32::consts::TAU),
                rotation_speed: rng.gen_range(-0.2..0.2),
                character: ['■', '▲', '●', '◆', '★'][rng.gen_range(0..5)],
            });
        }
    }

    fn update_wave(&mut self) {
        self.wave_offset += 0.1;
    }

    fn update_particles(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for particle in &mut self.particles {
            particle.x += particle.dx;
            particle.y += particle.dy;
            particle.life = particle.life.saturating_sub(1);

            // Bounce off edges
            if particle.x <= 0.0 || particle.x >= area.width as f32 {
                particle.dx = -particle.dx;
            }
            if particle.y <= 0.0 || particle.y >= area.height as f32 {
                particle.dy = -particle.dy;
            }

            // Respawn if dead
            if particle.life == 0 {
                particle.x = rng.gen_range(0.0..area.width as f32);
                particle.y = rng.gen_range(0.0..area.height as f32);
                particle.dx = rng.gen_range(-0.5..0.5);
                particle.dy = rng.gen_range(-0.5..0.5);
                particle.life = rng.gen_range(50..particle.max_life);
                particle.color = Color::Rgb(
                    rng.gen_range(100..255),
                    rng.gen_range(100..255),
                    rng.gen_range(100..255),
                );
            }
        }

        let target_count =
            ((area.width as usize * area.height as usize * config.animation.density as usize)
                / 400)
                .max(10);
        while self.particles.len() < target_count {
            self.particles.push(Particle {
                x: rng.gen_range(0.0..area.width as f32),
                y: rng.gen_range(0.0..area.height as f32),
                dx: rng.gen_range(-0.5..0.5),
                dy: rng.gen_range(-0.5..0.5),
                life: rng.gen_range(50..150),
                max_life: 150,
                color: Color::Rgb(
                    rng.gen_range(100..255),
                    rng.gen_range(100..255),
                    rng.gen_range(100..255),
                ),
            });
        }
    }

    fn update_fireworks(&mut self, area: Rect) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for firework in &mut self.fireworks {
            if !firework.exploded {
                // Rocket phase
                firework.x += firework.vx;
                firework.y += firework.vy;
                firework.vy += 0.05; // Gravity

                // Explode when velocity slows down
                if firework.vy >= -0.5 {
                    firework.exploded = true;
                    let particle_count = rng.gen_range(15..30);
                    for _ in 0..particle_count {
                        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                        let speed = rng.gen_range(0.5..2.5);
                        firework.particles.push(FireworkParticle {
                            x: firework.x,
                            y: firework.y,
                            vx: angle.cos() * speed,
                            vy: angle.sin() * speed,
                            life: rng.gen_range(30..60),
                            max_life: 60,
                        });
                    }
                }
            } else {
                // Particle phase
                for particle in &mut firework.particles {
                    particle.x += particle.vx;
                    particle.y += particle.vy;
                    particle.vy += 0.03; // Gravity on particles
                    particle.life = particle.life.saturating_sub(1);
                }
                firework.particles.retain(|p| p.life > 0);
            }
            firework.life = firework.life.saturating_sub(1);
        }

        // Remove dead fireworks
        self.fireworks
            .retain(|f| f.life > 0 && (f.life > 50 || !f.particles.is_empty()));

        // Spawn new firework occasionally
        if rng.gen_bool(0.02) && self.fireworks.len() < 5 {
            let colors = [
                (255, 100, 50),  // Orange
                (255, 50, 50),   // Red
                (50, 255, 100),  // Green
                (50, 100, 255),  // Blue
                (255, 50, 255),  // Purple
                (255, 255, 50),  // Yellow
                (50, 255, 255),  // Cyan
                (255, 255, 255), // White
            ];
            self.fireworks.push(Firework {
                x: rng.gen_range(5.0..(area.width.saturating_sub(5)) as f32),
                y: area.height as f32,
                vx: rng.gen_range(-0.5..0.5),
                vy: rng.gen_range(-3.5..-2.5),
                particles: Vec::new(),
                exploded: false,
                life: 120,
                color: colors[rng.gen_range(0..colors.len())],
            });
        }
    }

    fn update_neon_grid(&mut self) {
        self.neon_offset += 0.08;
    }

    fn update_perlin_flow(&mut self) {
        self.perlin_offset += 0.015;
    }

    fn update_cube_3d(&mut self) {
        self.cube_rotation.angle_x += 0.03;
        self.cube_rotation.angle_y += 0.05;
        self.cube_rotation.angle_z += 0.02;
    }

    fn update_fractals(&mut self) {
        // Slowly pan the fractal view
        self.fractal_offset.0 += 0.002;
        self.fractal_offset.1 += 0.001;
    }

    // New update methods for v1.1.5 animations
    fn update_ocean(&mut self) {
        self.ocean_phase += 0.05;
    }

    fn update_ripple(&mut self, area: Rect, _config: &Config) {
        self.ripple_radius += 0.5;
        if self.ripple_radius > (area.width.max(area.height) as f32) {
            self.ripple_radius = 0.0;
        }
    }

    fn update_fog(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.fog_density += rng.gen_range(-0.02..0.02);
        self.fog_density = self.fog_density.clamp(0.3, 0.8);
    }

    fn update_flames(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for flame in &mut self.flames {
            flame.height += rng.gen_range(-0.5..0.5);
            flame.height = flame.height.clamp(3.0, 15.0);
            flame.intensity =
                (flame.intensity as i16 + rng.gen_range(-10..10)).clamp(100, 255) as u8;
        }

        let target_count = (config.animation.density as usize / 2).max(5);
        while self.flames.len() < target_count {
            self.flames.push(FlameParticle {
                x: rng.gen_range(0.0..area.width as f32),
                _y: area.height as f32,
                height: rng.gen_range(3.0..10.0),
                intensity: rng.gen_range(150..255),
            });
        }
    }

    fn update_sparks(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for spark in &mut self.sparks {
            spark.x += spark.vx;
            spark.y += spark.vy;
            spark.vy += 0.05; // gravity
            spark.life = spark.life.saturating_sub(1);
            spark.brightness = spark.brightness.saturating_sub(2);
        }

        self.sparks.retain(|s| s.life > 0 && s.y > 0.0);

        let target_count = (config.animation.density as usize / 3).max(3);
        while self.sparks.len() < target_count {
            self.sparks.push(Spark {
                x: rng.gen_range(0.0..area.width as f32),
                y: rng.gen_range(area.height as f32 / 2.0..area.height as f32),
                vx: rng.gen_range(-0.5..0.5),
                vy: rng.gen_range(-2.0..-0.5),
                life: rng.gen_range(20..60),
                brightness: rng.gen_range(200..255),
            });
        }
    }

    fn update_lava_lamp(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for blob in &mut self.lava_blobs {
            blob.y += blob.dy;
            blob.color_phase += 0.02;

            // Bounce off top and bottom
            if blob.y <= blob.size || blob.y >= area.height as f32 - blob.size {
                blob.dy = -blob.dy;
            }

            // Random direction change
            if rng.gen_bool(0.02) {
                blob.dy += rng.gen_range(-0.05..0.05);
                blob.dy = blob.dy.clamp(-0.3, 0.3);
            }
        }
    }

    fn update_sun(&mut self) {
        self.sun_phase += 0.03;
    }

    fn update_galaxy(&mut self) {
        self.galaxy_angle += 0.01;
    }

    fn update_meteor_shower(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for meteor in &mut self.meteors {
            meteor.x += meteor.vx;
            meteor.y += meteor.vy;
        }

        // Remove meteors that went off screen
        self.meteors.retain(|m| {
            m.y < area.height as f32 + 5.0 && m.x > -5.0 && m.x < area.width as f32 + 5.0
        });

        // Spawn new meteors
        let target_count = (config.animation.density as usize / 5).max(2);
        if self.meteors.len() < target_count && rng.gen_bool(0.1) {
            self.meteors.push(Meteor {
                x: rng.gen_range(-5.0..area.width as f32),
                y: rng.gen_range(-5.0..(area.height / 2) as f32),
                vx: rng.gen_range(-1.0..1.0),
                vy: rng.gen_range(0.5..2.0),
                tail_length: rng.gen_range(3..8),
                brightness: rng.gen_range(200..255),
            });
        }
    }

    fn update_satellite(&mut self, area: Rect, _config: &Config) {
        self.satellite.angle += 0.02;
        let cx = area.width as f32 / 2.0;
        let cy = area.height as f32 / 2.0;
        self.satellite.x = cx + self.satellite.angle.cos() * self.satellite.orbit_radius;
        self.satellite.y = cy + self.satellite.angle.sin() * self.satellite.orbit_radius * 0.5;
        self.satellite.signal_timer = self.satellite.signal_timer.saturating_sub(1);
    }

    fn update_pulsar(&mut self) {
        self.pulsar_angle += 0.05;
    }

    fn update_pong(&mut self, area: Rect, _config: &Config) {
        // fog uses randomness in render

        // Move ball
        self.pong.ball_x += self.pong.ball_vx;
        self.pong.ball_y += self.pong.ball_vy;

        // Bounce off top/bottom
        if self.pong.ball_y <= 1.0 || self.pong.ball_y >= area.height as f32 - 1.0 {
            self.pong.ball_vy = -self.pong.ball_vy;
        }

        // Bounce off paddles
        if self.pong.ball_x <= 2.0 {
            if (self.pong.ball_y - self.pong.paddle1_y).abs() < 3.0 {
                self.pong.ball_vx = -self.pong.ball_vx;
            } else {
                // Reset ball
                self.pong.ball_x = area.width as f32 / 2.0;
                self.pong.ball_y = area.height as f32 / 2.0;
                self.pong.ball_vx = 0.8;
                self.pong.score2 += 1;
            }
        } else if self.pong.ball_x >= area.width as f32 - 2.0 {
            if (self.pong.ball_y - self.pong.paddle2_y).abs() < 3.0 {
                self.pong.ball_vx = -self.pong.ball_vx;
            } else {
                // Reset ball
                self.pong.ball_x = area.width as f32 / 2.0;
                self.pong.ball_y = area.height as f32 / 2.0;
                self.pong.ball_vx = -0.8;
                self.pong.score1 += 1;
            }
        }

        // Move AI paddles towards ball
        if self.pong.paddle1_y < self.pong.ball_y - 1.0 {
            self.pong.paddle1_y += 0.4;
        } else if self.pong.paddle1_y > self.pong.ball_y + 1.0 {
            self.pong.paddle1_y -= 0.4;
        }

        if self.pong.paddle2_y < self.pong.ball_y - 1.0 {
            self.pong.paddle2_y += 0.4;
        } else if self.pong.paddle2_y > self.pong.ball_y + 1.0 {
            self.pong.paddle2_y -= 0.4;
        }

        // Clamp paddles
        self.pong.paddle1_y = self.pong.paddle1_y.clamp(2.0, area.height as f32 - 3.0);
        self.pong.paddle2_y = self.pong.paddle2_y.clamp(2.0, area.height as f32 - 3.0);
    }

    fn update_snake(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        self.snake.tick_count += 1;
        if self.snake.tick_count < 3 {
            return;
        }
        self.snake.tick_count = 0;

        // Move snake
        let head = self.snake.segments[0];
        let new_head = match self.snake.direction {
            0 => (head.0, head.1.saturating_sub(1)),
            1 => ((head.0 + 1).min(area.width - 1), head.1),
            2 => (head.0, (head.1 + 1).min(area.height - 1)),
            _ => (head.0.saturating_sub(1), head.1),
        };

        // Check food collision
        if new_head == self.snake.food {
            self.snake.segments.insert(0, new_head);
            self.snake.food = (
                rng.gen_range(5..area.width - 5),
                rng.gen_range(3..area.height - 3),
            );
        } else {
            self.snake.segments.pop();
            self.snake.segments.insert(0, new_head);
        }

        // Random direction change occasionally
        if rng.gen_bool(0.1) {
            self.snake.direction = rng.gen_range(0..4);
        }
    }

    fn update_tetris(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        self.tetris.tick_count += 1;
        if self.tetris.tick_count < 5 {
            return;
        }
        self.tetris.tick_count = 0;

        if let Some((x, y, piece_type)) = self.tetris.falling_piece {
            let new_y = y + 1;
            if new_y >= area.height - 1 {
                self.tetris.pieces.push((x, y, piece_type));
                self.tetris.falling_piece = Some((area.width / 2, 0, rng.gen_range(0..7)));
            } else {
                self.tetris.falling_piece = Some((x, new_y, piece_type));
            }
        }
    }

    fn update_invaders(&mut self, area: Rect, _config: &Config) {
        // paint_splatter uses randomness in render

        let move_down = self.invaders.iter().any(|i| {
            (i.x <= 2.0 && i.direction < 0) || (i.x >= area.width as f32 - 3.0 && i.direction > 0)
        });

        for invader in &mut self.invaders {
            if move_down {
                invader.y += 1.0;
                invader.direction = -invader.direction;
            } else {
                invader.x += invader.direction as f32 * 0.5;
            }
            if self.tick % 10 == 0 {
                invader.anim_frame = !invader.anim_frame;
            }
        }

        // Reset if all went off bottom
        if self.invaders.iter().all(|i| i.y > area.height as f32) {
            self.invaders.clear();
            for row in 0..3 {
                for col in 0..5 {
                    self.invaders.push(Invader {
                        x: (5 + col * 6) as f32,
                        y: (2 + row * 3) as f32,
                        invader_type: (row as u8) % 3,
                        direction: 1,
                        anim_frame: false,
                    });
                }
            }
        }
    }

    fn update_fibonacci(&mut self) {
        self.fibonacci_angle += 0.02;
    }

    fn update_mandelbrot(&mut self) {
        self.mandelbrot_offset.0 += 0.001;
    }

    fn update_hex_grid(&mut self) {
        self.hex_phase += 0.03;
    }

    fn update_rose(&mut self) {
        self.rose_angle += 0.02;
    }

    fn update_butterflies(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for butterfly in &mut self.butterflies {
            // Move towards target
            let dx = butterfly.target_x - butterfly.x;
            let dy = butterfly.target_y - butterfly.y;
            butterfly.x += dx * 0.02;
            butterfly.y += dy * 0.02;

            // Flap wings
            if self.tick % 5 == 0 {
                butterfly.wing_open = !butterfly.wing_open;
            }

            // New target if close
            if dx.abs() < 1.0 && dy.abs() < 1.0 {
                butterfly.target_x = rng.gen_range(5.0..(area.width.saturating_sub(5)) as f32);
                butterfly.target_y = rng.gen_range(3.0..(area.height.saturating_sub(3)) as f32);
            }
        }
    }

    fn update_spider_web(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for strand in &mut self.web_strands {
            strand.vibration = rng.gen_range(-0.1..0.1);
        }
    }

    fn update_vine_growth(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for vine in &mut self.vines {
            vine.length = (vine.length as f32 + vine.growth_rate) as u16;
            if vine.length >= vine.max_length {
                vine.length = 0;
                vine.x = rng.gen_range(0.0..area.width as f32);
                vine._y = area.height as f32;
            }
        }
    }

    fn update_moss(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for cell in &mut self.moss {
            cell.age = cell.age.saturating_add(1);
        }

        let target_count =
            ((area.width as usize * area.height as usize * config.animation.density as usize)
                / 2000)
                .max(10);
        if self.moss.len() < target_count && rng.gen_bool(0.1) {
            self.moss.push(MossCell {
                x: rng.gen_range(0..area.width),
                y: rng.gen_range((area.height / 2)..area.height),
                age: 0,
                spreading: rng.gen_bool(0.3),
            });
        }
    }

    fn update_radar(&mut self) {
        self.radar_angle += 0.05;
    }

    fn update_binary_clock(&mut self) {
        self.binary_time += 1;
    }

    fn update_signal(&mut self, _area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for signal in &mut self.signals {
            signal.radius += 0.3;
        }

        self.signals.retain(|s| s.radius < s.max_radius);

        if self.signals.len() < 2 && rng.gen_bool(0.05) {
            self.signals.push(SignalWave {
                x: rng.gen_range(5.._area.width - 5),
                y: _area.height / 2,
                radius: 0.0,
                max_radius: rng.gen_range(10.0..30.0),
                amplitude: rng.gen_range(100..200),
            });
        }
    }

    fn update_wifi(&mut self) {
        for wave in &mut self.wifi_waves {
            wave.radius += 0.2;
            wave.intensity = wave.intensity.saturating_sub(1);
        }

        // Reset waves that got too big
        for wave in &mut self.wifi_waves {
            if wave.radius > 20.0 {
                wave.radius = 0.0;
                wave.intensity = 200;
            }
        }
    }

    fn update_paint_splatter(&mut self, area: Rect, config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for splatter in &mut self.splatters {
            splatter.age = splatter.age.saturating_add(1);
        }

        self.splatters.retain(|s| s.age < 200);

        let target_count = (config.animation.density as usize / 10).max(1);
        if self.splatters.len() < target_count && rng.gen_bool(0.05) {
            self.splatters.push(PaintSplatter {
                x: rng.gen_range(0..area.width),
                y: rng.gen_range(0..area.height),
                size: rng.gen_range(1..4),
                color: (
                    rng.gen_range(100..255),
                    rng.gen_range(100..255),
                    rng.gen_range(100..255),
                ),
                age: 0,
            });
        }
    }

    fn update_ink_bleed(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for drop in &mut self.ink_drops {
            drop.radius += 0.1;
        }

        self.ink_drops.retain(|d| d.radius < d.max_radius);

        if self.ink_drops.len() < 3 && rng.gen_bool(0.02) {
            self.ink_drops.push(InkDrop {
                x: rng.gen_range(5.0..(area.width - 5) as f32),
                y: rng.gen_range(5.0..(area.height - 5) as f32),
                radius: 0.5,
                max_radius: rng.gen_range(3.0..8.0),
                color: (
                    rng.gen_range(0..100),
                    rng.gen_range(0..100),
                    rng.gen_range(100..200),
                ),
            });
        }
    }

    fn update_mosaic(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for tile in &mut self.mosaic_tiles {
            if tile.changing {
                tile.change_timer = tile.change_timer.saturating_sub(1);
                if tile.change_timer == 0 {
                    tile.changing = false;
                    tile.color = (
                        rng.gen_range(50..200),
                        rng.gen_range(50..200),
                        rng.gen_range(50..200),
                    );
                }
            } else if rng.gen_bool(0.01) {
                tile.changing = true;
                tile.change_timer = rng.gen_range(10..30);
            }
        }
    }

    fn update_stained_glass(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for panel in &mut self.glass_panels {
            panel.light_intensity =
                (panel.light_intensity as i16 + rng.gen_range(-5..5)).clamp(50, 255) as u8;
        }
    }

    fn update_hologram(&mut self, area: Rect) {
        self.hologram_line = (self.hologram_line + 1) % area.height;
    }

    fn update_glitch(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.glitch_timer = rng.gen_range(0..10);
    }

    fn update_old_film(&mut self, area: Rect, _config: &Config) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for scratch in &mut self.scratches {
            scratch.visible = rng.gen_bool(0.3);
            scratch.y = (scratch.y + 1) % area.height;
        }
    }

    fn update_thermal(&mut self, area: Rect) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let count = (area.width * area.height) as usize;
        if self.thermal_noise.len() != count {
            self.thermal_noise = (0..count).map(|_| rng.gen_range(0..255)).collect();
        }
        for noise in &mut self.thermal_noise {
            *noise = (*noise as i16 + rng.gen_range(-10..10)).clamp(0, 255) as u8;
        }
    }
}

// New render functions for v1.1.5 animations
fn render_ocean(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(0, 20, 40)));
    f.render_widget(bg_fill, size);

    let phase = state.ocean_phase;
    let wave_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    for y in (size.height / 2)..size.height {
        let wave_height =
            ((y as f32 - size.height as f32 / 2.0) / (size.height as f32 / 2.0) * 8.0) as usize;
        for x in 0..size.width {
            let wave = ((x as f32 * 0.2 + phase + y as f32 * 0.1).sin() * 4.0 + 4.0) as usize;
            let char_idx = (wave + wave_height).min(7);
            let intensity = (150 + char_idx * 10) as u8;
            let color = Color::Rgb(0, intensity / 2, intensity);
            let span = Span::styled(wave_chars[char_idx].to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_ripple(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 15, 25)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let radius = state.ripple_radius;

    for ring in 0..5 {
        let r = radius - ring as f32 * 4.0;
        if r < 0.0 {
            continue;
        }
        let intensity = (255 - ring * 40) as u8;
        let ring_color = match color {
            Color::Rgb(r, g, b) => Color::Rgb(
                (r as u16 * intensity as u16 / 255) as u8,
                (g as u16 * intensity as u16 / 255) as u8,
                (b as u16 * intensity as u16 / 255) as u8,
            ),
            _ => Color::Rgb(intensity, intensity, intensity),
        };

        for angle in (0..360).step_by(10) {
            let rad = angle as f32 * std::f32::consts::PI / 180.0;
            let x = center_x + rad.cos() * r;
            let y = center_y + rad.sin() * r * 0.5;

            let px = x as u16;
            let py = y as u16;
            if px < size.width && py < size.height {
                let span = Span::styled("◦", Style::default().fg(ring_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_fog(f: &mut Frame, state: &AnimationState, size: Rect) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    let density = state.fog_density;
    for y in 0..size.height {
        for x in 0..size.width {
            if rng.gen_bool(density as f64 * 0.3) {
                let alpha = rng.gen_range(50..150) as u8;
                let color = Color::Rgb(alpha, alpha, alpha + 10);
                let span = Span::styled("░", Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_flames(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 5, 5)));
    f.render_widget(bg_fill, size);

    let flame_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█', '▲', '◆'];
    let colors = [
        (255u8, 50u8, 0u8),
        (255, 100, 0),
        (255, 150, 0),
        (255, 200, 0),
        (255, 255, 100),
    ];

    for flame in &state.flames {
        let x = flame.x as u16;
        let height = flame.height as u16;
        let intensity = flame.intensity;

        for h in 0..height {
            let y = size.height.saturating_sub(h + 1);
            if y >= size.height || x >= size.width {
                continue;
            }

            let color_idx = (h as f32 / height as f32 * colors.len() as f32) as usize;
            let (rc, gc, bc) = colors[color_idx.min(colors.len() - 1)];
            let r = (rc as u16 * intensity as u16 / 255) as u8;
            let g = (gc as u16 * intensity as u16 / 255) as u8;
            let b = (bc as u16 * intensity as u16 / 255) as u8;
            let flame_color = Color::Rgb(r, g, b);

            let char_idx = (h as f32 / height as f32 * flame_chars.len() as f32) as usize;
            let ch = flame_chars[char_idx.min(flame_chars.len() - 1)];

            let span = Span::styled(ch.to_string(), Style::default().fg(flame_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_sparks(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 5)));
    f.render_widget(bg_fill, size);

    for spark in &state.sparks {
        let x = spark.x as u16;
        let y = spark.y as u16;
        if x < size.width && y < size.height {
            let intensity = spark.brightness;
            let color = Color::Rgb(255, 200 + intensity / 5, intensity);
            let span = Span::styled("✦", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_lava_lamp(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(20, 10, 10)));
    f.render_widget(bg_fill, size);

    for blob in &state.lava_blobs {
        let x = blob.x as u16;
        let y = blob.y as u16;
        let size_blob = blob.size as u16;

        let hue = (blob.color_phase.sin() * 0.5 + 0.5) * 60.0;
        let r = 255u8;
        let g = (hue * 2.0) as u8;
        let b = 50u8;
        let color = Color::Rgb(r, g, b);

        for dy in 0..size_blob {
            for dx in 0..size_blob {
                let px = x + dx;
                let py = y + dy;
                if px < size.width && py < size.height {
                    let span = Span::styled("●", Style::default().fg(color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(px, py, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }
}

fn render_sun(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(0, 10, 30)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let pulse = state.sun_phase.sin() * 0.2 + 1.0;
    let radius = (size.width.min(size.height) as f32 / 4.0) * pulse;

    for y in 0..size.height {
        for x in 0..size.width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < radius {
                let intensity = (1.0 - dist / radius) * 255.0;
                let color = Color::Rgb(
                    255,
                    (200.0 + intensity * 0.2) as u8,
                    (intensity * 0.5) as u8,
                );
                let ch = if dist < radius * 0.3 { "█" } else { "▓" };
                let span = Span::styled(ch, Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Sun rays
    for i in 0..12 {
        let angle = (i as f32 * 30.0 + state.sun_phase * 10.0) * std::f32::consts::PI / 180.0;
        for r in (radius as u16 + 2)..(radius as u16 + 8) {
            let x = center_x + angle.cos() * r as f32;
            let y = center_y + angle.sin() * r as f32 * 0.5;
            let px = x as u16;
            let py = y as u16;
            if px < size.width && py < size.height {
                let span = Span::styled("│", Style::default().fg(Color::Rgb(255, 200, 100)));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_galaxy(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 15)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;

    // Spiral arms
    for arm in 0..4 {
        let arm_offset = arm as f32 * std::f32::consts::PI / 2.0;
        for r in 1..30 {
            let angle = r as f32 * 0.2 + state.galaxy_angle + arm_offset;
            let x = center_x + angle.cos() * r as f32;
            let y = center_y + angle.sin() * r as f32 * 0.5;
            let px = x as u16;
            let py = y as u16;
            if px < size.width && py < size.height {
                let intensity = (255 - r * 6) as u8;
                let color = Color::Rgb(intensity, intensity / 2, intensity);
                let span = Span::styled("•", Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Center
    let span = Span::styled("◉", Style::default().fg(Color::Rgb(255, 255, 200)));
    let text = Line::from(vec![span]);
    let paragraph = Paragraph::new(text);
    let area = Rect::new(center_x as u16, center_y as u16, 1, 1);
    f.render_widget(paragraph, area);
}

fn render_meteor_shower(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
    f.render_widget(bg_fill, size);

    for meteor in &state.meteors {
        let x = meteor.x as u16;
        let y = meteor.y as u16;
        if x < size.width && y < size.height {
            let intensity = meteor.brightness;
            let color = Color::Rgb(255, 255, intensity);
            let span = Span::styled("☄", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);

            // Tail
            for t in 1..meteor.tail_length {
                let tx = (meteor.x - meteor.vx * t as f32) as u16;
                let ty = (meteor.y - meteor.vy * t as f32) as u16;
                if tx < size.width && ty < size.height {
                    let tail_intensity = intensity.saturating_sub(t * 20);
                    let tail_color = Color::Rgb(tail_intensity, tail_intensity, tail_intensity / 2);
                    let span = Span::styled("·", Style::default().fg(tail_color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(tx, ty, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }
}

fn render_satellite(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
    f.render_widget(bg_fill, size);

    // Orbit path
    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let radius = state.satellite.orbit_radius;

    for angle in (0..360).step_by(15) {
        let rad = angle as f32 * std::f32::consts::PI / 180.0;
        let x = center_x + rad.cos() * radius;
        let y = center_y + rad.sin() * radius * 0.5;
        let px = x as u16;
        let py = y as u16;
        if px < size.width && py < size.height {
            let span = Span::styled("·", Style::default().fg(Color::Rgb(50, 50, 80)));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(px, py, 1, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Satellite
    let x = state.satellite.x as u16;
    let y = state.satellite.y as u16;
    if x < size.width && y < size.height {
        let span = Span::styled("🛰", Style::default().fg(Color::Rgb(200, 200, 220)));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(x, y, 1, 1);
        f.render_widget(paragraph, area);

        // Signal waves
        if state.satellite.signal_timer % 20 < 10 {
            for r in 1..=3 {
                let sx = (state.satellite.x + r as f32) as u16;
                if sx < size.width && y < size.height {
                    let intensity = (200 - r * 50) as u8;
                    let span = Span::styled(
                        ")",
                        Style::default().fg(Color::Rgb(intensity, intensity, intensity + 20)),
                    );
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(sx, y, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }
}

fn render_pulsar(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let pulse = state.pulsar_angle.sin() * 0.5 + 0.5;

    // Spinning beams
    for i in 0..2 {
        let beam_angle = state.pulsar_angle + i as f32 * std::f32::consts::PI;
        for r in 0..20 {
            let x = center_x + beam_angle.cos() * r as f32;
            let y = center_y + beam_angle.sin() * r as f32 * 0.5;
            let px = x as u16;
            let py = y as u16;
            if px < size.width && py < size.height {
                let intensity = (pulse * 255.0) as u8;
                let c = match color {
                    Color::Rgb(r, g, b) => Color::Rgb(
                        (r as u16 * intensity as u16 / 255) as u8,
                        (g as u16 * intensity as u16 / 255) as u8,
                        (b as u16 * intensity as u16 / 255) as u8,
                    ),
                    _ => Color::Rgb(intensity, intensity, intensity),
                };
                let span = Span::styled("█", Style::default().fg(c));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Center pulsar
    let center_intensity = (pulse * 255.0) as u8;
    let span = Span::styled(
        "◉",
        Style::default().fg(Color::Rgb(255, 255, center_intensity)),
    );
    let text = Line::from(vec![span]);
    let paragraph = Paragraph::new(text);
    let area = Rect::new(center_x as u16, center_y as u16, 1, 1);
    f.render_widget(paragraph, area);
}

fn render_pong(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 15, 10)));
    f.render_widget(bg_fill, size);

    // Paddles
    for dy in -2..=2 {
        let y1 = (state.pong.paddle1_y + dy as f32) as u16;
        let y2 = (state.pong.paddle2_y + dy as f32) as u16;
        if y1 < size.height {
            let span = Span::styled("█", Style::default().fg(Color::Rgb(200, 200, 200)));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(1, y1, 1, 1);
            f.render_widget(paragraph, area);
        }
        if y2 < size.height {
            let span = Span::styled("█", Style::default().fg(Color::Rgb(200, 200, 200)));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(size.width - 2, y2, 1, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Ball
    let bx = state.pong.ball_x as u16;
    let by = state.pong.ball_y as u16;
    if bx < size.width && by < size.height {
        let span = Span::styled("◆", Style::default().fg(Color::Rgb(255, 255, 100)));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(bx, by, 1, 1);
        f.render_widget(paragraph, area);
    }

    // Score
    let score_text = format!("{} : {}", state.pong.score1, state.pong.score2);
    let span = Span::styled(score_text, Style::default().fg(Color::Rgb(150, 150, 150)));
    let text = Line::from(vec![span]);
    let paragraph = Paragraph::new(text);
    let area = Rect::new(size.width / 2 - 3, 1, 7, 1);
    f.render_widget(paragraph, area);
}

fn render_snake(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 20, 10)));
    f.render_widget(bg_fill, size);

    // Food
    let (fx, fy) = state.snake.food;
    if fx < size.width && fy < size.height {
        let span = Span::styled("●", Style::default().fg(Color::Rgb(255, 50, 50)));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(fx, fy, 1, 1);
        f.render_widget(paragraph, area);
    }

    // Snake body
    for (i, (x, y)) in state.snake.segments.iter().enumerate() {
        if *x < size.width && *y < size.height {
            let color = if i == 0 {
                Color::Rgb(100, 255, 100)
            } else {
                Color::Rgb(50, 200, 50)
            };
            let span = Span::styled("█", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(*x, *y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_tetris(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(20, 20, 20)));
    f.render_widget(bg_fill, size);

    let piece_chars = ['█', '▓', '▒', '░', '◆', '●', '■'];
    let piece_colors = [
        Color::Rgb(255, 50, 50),
        Color::Rgb(50, 255, 50),
        Color::Rgb(50, 50, 255),
        Color::Rgb(255, 255, 50),
        Color::Rgb(255, 50, 255),
        Color::Rgb(50, 255, 255),
        Color::Rgb(255, 150, 50),
    ];

    // Placed pieces
    for (x, y, piece_type) in &state.tetris.pieces {
        if *x < size.width && *y < size.height {
            let color = piece_colors[*piece_type as usize % piece_colors.len()];
            let ch = piece_chars[*piece_type as usize % piece_chars.len()];
            let span = Span::styled(ch.to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(*x, *y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Falling piece
    if let Some((x, y, piece_type)) = state.tetris.falling_piece {
        if x < size.width && y < size.height {
            let color = piece_colors[piece_type as usize % piece_colors.len()];
            let ch = piece_chars[piece_type as usize % piece_chars.len()];
            let span = Span::styled(ch.to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_invaders(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 5)));
    f.render_widget(bg_fill, size);

    let invader_chars = ['👾', '👽', '👻'];
    let colors = [
        Color::Rgb(255, 100, 100),
        Color::Rgb(100, 255, 100),
        Color::Rgb(100, 100, 255),
    ];

    for invader in &state.invaders {
        let x = invader.x as u16;
        let y = invader.y as u16;
        if x < size.width && y < size.height {
            let color = colors[invader.invader_type as usize % colors.len()];
            let ch = invader_chars[invader.invader_type as usize % invader_chars.len()];
            let span = Span::styled(ch.to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_fibonacci(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 10)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let golden_angle = 137.5_f32.to_radians();

    for i in 0..200 {
        let r = (i as f32).sqrt() * 0.8;
        let theta = i as f32 * golden_angle + state.fibonacci_angle;
        let x = center_x + r * theta.cos();
        let y = center_y + r * theta.sin() * 0.5;

        let px = x as u16;
        let py = y as u16;
        if px < size.width && py < size.height {
            let intensity = (255 - i / 2) as u8;
            let c = match color {
                Color::Rgb(r, g, b) => Color::Rgb(
                    (r as u16 * intensity as u16 / 255) as u8,
                    (g as u16 * intensity as u16 / 255) as u8,
                    (b as u16 * intensity as u16 / 255) as u8,
                ),
                _ => Color::Rgb(intensity, intensity, intensity),
            };
            let span = Span::styled("●", Style::default().fg(c));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(px, py, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_mandelbrot(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
    f.render_widget(bg_fill, size);

    let offset_x = state.mandelbrot_offset.0;
    let offset_y = state.mandelbrot_offset.1;

    for py in 0..size.height {
        for px in 0..size.width {
            let x0 = (px as f32 / size.width as f32 - 0.5) * 3.0 + offset_x;
            let y0 = (py as f32 / size.height as f32 - 0.5) * 2.0 + offset_y;

            let mut x = 0.0;
            let mut y = 0.0;
            let mut iter = 0;

            while x * x + y * y <= 4.0 && iter < 30 {
                let xtemp = x * x - y * y + x0;
                y = 2.0 * x * y + y0;
                x = xtemp;
                iter += 1;
            }

            if iter < 30 {
                let intensity = (iter as f32 / 30.0 * 255.0) as u8;
                let c = match color {
                    Color::Rgb(r, g, b) => Color::Rgb(
                        (r as u16 * intensity as u16 / 255) as u8,
                        (g as u16 * intensity as u16 / 255) as u8,
                        (b as u16 * intensity as u16 / 255) as u8,
                    ),
                    _ => Color::Rgb(intensity, intensity, intensity),
                };
                let chars = ['·', ':', '-', '=', '+', '*', '#', '%', '@'];
                let ch = chars[iter as usize % chars.len()];
                let span = Span::styled(ch.to_string(), Style::default().fg(c));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_hex_grid(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 15, 20)));
    f.render_widget(bg_fill, size);

    let hex_chars = ['⬡', '⬢', '⬣'];
    for y in 0..size.height {
        for x in 0..size.width {
            let wave = (x as f32 * 0.3 + y as f32 * 0.2 + state.hex_phase).sin() * 0.5 + 0.5;
            if wave > 0.5 {
                let char_idx = (wave * hex_chars.len() as f32) as usize % hex_chars.len();
                let intensity = (wave * 200.0) as u8 + 50;
                let color = Color::Rgb(intensity / 3, intensity / 2, intensity);
                let span =
                    Span::styled(hex_chars[char_idx].to_string(), Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_rose(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 15)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let k = 5.0; // petals
    let a = 10.0;

    for theta in (0..720).step_by(2) {
        let rad = theta as f32 * std::f32::consts::PI / 180.0 + state.rose_angle;
        let r = a * (k * rad).cos();
        let x = center_x + r * rad.cos();
        let y = center_y + r * rad.sin() * 0.5;

        let px = x as u16;
        let py = y as u16;
        if px < size.width && py < size.height {
            let c = match color {
                Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
                _ => Color::Rgb(255, 100, 150),
            };
            let span = Span::styled("●", Style::default().fg(c));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(px, py, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_butterflies(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(20, 25, 20)));
    f.render_widget(bg_fill, size);

    for butterfly in &state.butterflies {
        let x = butterfly.x as u16;
        let y = butterfly.y as u16;
        if x < size.width && y < size.height {
            let hue = butterfly.color as f32 / 255.0;
            let r = ((hue * 6.0).sin() * 0.5 + 0.5) * 255.0;
            let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * 255.0;
            let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * 255.0;
            let color = Color::Rgb(r as u8, g as u8, b as u8);

            let ch = if butterfly.wing_open { '⌘' } else { '⍟' };
            let span = Span::styled(ch.to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_spider_web(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(15, 15, 20)));
    f.render_widget(bg_fill, size);

    for strand in &state.web_strands {
        let x1 = strand.x1 as u16;
        let y1 = strand.y1 as u16;
        let x2 = strand.x2 as u16;
        let y2 = strand.y2 as u16;

        // Simple line drawing
        let dx = if x2 > x1 { x2 - x1 } else { x1 - x2 };
        let dy = if y2 > y1 { y2 - y1 } else { y1 - y2 };
        let steps = dx.max(dy);

        for step in 0..=steps {
            let t = if steps == 0 {
                0.0
            } else {
                step as f32 / steps as f32
            };
            let x = (strand.x1 + (strand.x2 - strand.x1) * t) as u16;
            let y = (strand.y1 + (strand.y2 - strand.y1) * t) as u16;
            if x < size.width && y < size.height {
                let intensity = (200.0 + strand.vibration * 500.0) as u8;
                let color = Color::Rgb(intensity, intensity, intensity + 20);
                let span = Span::styled("·", Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_vine_growth(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 20, 10)));
    f.render_widget(bg_fill, size);

    let vine_chars = ['│', '├', '┤', '╱', '╲'];
    let colors = [
        Color::Rgb(50, 150, 50),
        Color::Rgb(80, 180, 80),
        Color::Rgb(100, 200, 100),
    ];

    for vine in &state.vines {
        let x = vine.x as u16;
        let _start_y = size.height.saturating_sub(vine.length);
        for dy in 0..vine.length {
            let y = size.height.saturating_sub(dy + 1);
            if y < size.height {
                let color_idx = (dy as usize / 5) % colors.len();
                let char_idx = (vine.x as usize + dy as usize) % vine_chars.len();
                let span = Span::styled(
                    vine_chars[char_idx].to_string(),
                    Style::default().fg(colors[color_idx]),
                );
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_moss(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(15, 20, 15)));
    f.render_widget(bg_fill, size);

    for cell in &state.moss {
        if cell.x < size.width && cell.y < size.height {
            let intensity = (100 + cell.age / 2).min(255) as u8;
            let color = if cell.spreading {
                Color::Rgb(intensity / 2, intensity, intensity / 3)
            } else {
                Color::Rgb(intensity / 3, intensity / 2, intensity / 4)
            };
            let ch = if cell.age < 20 {
                '·'
            } else if cell.age < 50 {
                ':'
            } else {
                '▓'
            };
            let span = Span::styled(ch.to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(cell.x, cell.y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_radar(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 15, 5)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let radius = (size.width.min(size.height) as f32 / 2.5).min(15.0);

    // Grid circles
    for r in (2..radius as i32).step_by(4) {
        for angle in (0..360).step_by(10) {
            let rad = angle as f32 * std::f32::consts::PI / 180.0;
            let x = center_x + rad.cos() * r as f32;
            let y = center_y + rad.sin() * r as f32 * 0.6;
            let px = x as u16;
            let py = y as u16;
            if px < size.width && py < size.height {
                let span = Span::styled("·", Style::default().fg(Color::Rgb(50, 100, 50)));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Sweep line
    let sweep_angle = state.radar_angle;
    for r in 0..radius as i32 {
        let rad = sweep_angle;
        let x = center_x + rad.cos() * r as f32;
        let y = center_y + rad.sin() * r as f32 * 0.6;
        let px = x as u16;
        let py = y as u16;
        if px < size.width && py < size.height {
            let span = Span::styled("█", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(px, py, 1, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Blips
    use rand::Rng;
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.05) {
        let r = rng.gen_range(5.0..radius);
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let x = center_x + angle.cos() * r;
        let y = center_y + angle.sin() * r * 0.6;
        let px = x as u16;
        let py = y as u16;
        if px < size.width && py < size.height {
            let span = Span::styled("●", Style::default().fg(Color::Rgb(255, 50, 50)));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(px, py, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_binary_clock(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 10)));
    f.render_widget(bg_fill, size);

    let time = state.binary_time;
    let bits = [
        (time >> 5) & 1,
        (time >> 4) & 1,
        (time >> 3) & 1,
        (time >> 2) & 1,
        (time >> 1) & 1,
        time & 1,
    ];

    for (i, bit) in bits.iter().enumerate() {
        let y = size.height / 2 + i as u16 * 2;
        if y < size.height {
            let color = if *bit == 1 {
                Color::Rgb(0, 255, 0)
            } else {
                Color::Rgb(50, 50, 50)
            };
            let ch = if *bit == 1 { '●' } else { '○' };
            let span = Span::styled(ch.to_string(), Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(size.width / 2, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_signal(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 15)));
    f.render_widget(bg_fill, size);

    for signal in &state.signals {
        let x = signal.x;
        let y = signal.y;
        let r = signal.radius as i32;

        // Draw wave at this radius
        for angle in (0..360).step_by(30) {
            let rad = angle as f32 * std::f32::consts::PI / 180.0;
            let px = (x as f32 + rad.cos() * r as f32) as u16;
            let py = (y as f32 + rad.sin() * r as f32 * 0.3) as u16;

            if px < size.width && py < size.height {
                let intensity =
                    (signal.amplitude as f32 * (1.0 - signal.radius / signal.max_radius)) as u8;
                let color = Color::Rgb(intensity, intensity, intensity + 50);
                let span = Span::styled("~", Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Center point
    let cx = size.width / 2;
    let cy = size.height / 2;
    let span = Span::styled("●", Style::default().fg(Color::Rgb(255, 255, 255)));
    let text = Line::from(vec![span]);
    let paragraph = Paragraph::new(text);
    let area = Rect::new(cx, cy, 1, 1);
    f.render_widget(paragraph, area);
}

fn render_wifi(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(15, 15, 20)));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;

    for wave in &state.wifi_waves {
        let r = wave.radius as i32;
        let intensity = wave.intensity;
        let color = Color::Rgb(intensity, intensity, intensity + 20);

        // Draw arc
        for angle in 200..340 {
            let rad = angle as f32 * std::f32::consts::PI / 180.0;
            let x = center_x + rad.cos() * r as f32;
            let y = center_y + rad.sin() * r as f32 * 0.5;
            let px = x as u16;
            let py = y as u16;
            if px < size.width && py < size.height {
                let span = Span::styled(")", Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Source
    let span = Span::styled("●", Style::default().fg(Color::Rgb(100, 200, 255)));
    let text = Line::from(vec![span]);
    let paragraph = Paragraph::new(text);
    let area = Rect::new(center_x as u16, center_y as u16, 1, 1);
    f.render_widget(paragraph, area);
}

fn render_paint_splatter(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(240, 240, 240)));
    f.render_widget(bg_fill, size);

    for splatter in &state.splatters {
        let x = splatter.x;
        let y = splatter.y;
        let color = Color::Rgb(splatter.color.0, splatter.color.1, splatter.color.2);
        let chars = ['·', ':', '∙', '•', '◦'];

        for dy in 0..splatter.size {
            for dx in 0..splatter.size {
                let px = x + dx as u16;
                let py = y + dy as u16;
                if px < size.width && py < size.height {
                    let ch =
                        chars[(splatter.age as usize + dx as usize + dy as usize) % chars.len()];
                    let span = Span::styled(ch.to_string(), Style::default().fg(color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(px, py, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }
}

fn render_ink_bleed(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(245, 245, 250)));
    f.render_widget(bg_fill, size);

    for drop in &state.ink_drops {
        let cx = drop.x as u16;
        let cy = drop.y as u16;
        let r = drop.radius as i32;

        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let px = (cx as i32 + dx) as u16;
                    let py = (cy as i32 + dy) as u16;
                    if px < size.width && py < size.height {
                        let intensity = (1.0 - (dx * dx + dy * dy) as f32 / (r * r) as f32) * 255.0;
                        let c = Color::Rgb(
                            (drop.color.0 as f32 * intensity / 255.0) as u8,
                            (drop.color.1 as f32 * intensity / 255.0) as u8,
                            (drop.color.2 as f32 * intensity / 255.0) as u8,
                        );
                        let span = Span::styled("▒", Style::default().fg(c));
                        let text = Line::from(vec![span]);
                        let paragraph = Paragraph::new(text);
                        let area = Rect::new(px, py, 1, 1);
                        f.render_widget(paragraph, area);
                    }
                }
            }
        }
    }
}

fn render_mosaic(f: &mut Frame, state: &AnimationState, size: Rect) {
    for tile in &state.mosaic_tiles {
        let x = tile.x;
        let y = tile.y;
        let color = if tile.changing {
            Color::Rgb(255, 255, 255)
        } else {
            Color::Rgb(tile.color.0, tile.color.1, tile.color.2)
        };

        for dy in 0..4 {
            for dx in 0..4 {
                let px = x + dx;
                let py = y + dy;
                if px < size.width && py < size.height {
                    let span = Span::styled("█", Style::default().fg(color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(px, py, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }
}

fn render_stained_glass(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(20, 20, 25)));
    f.render_widget(bg_fill, size);

    for panel in &state.glass_panels {
        let x = panel.x;
        let y = panel.y;
        let w = panel.width;
        let h = panel.height;

        let hue = panel.hue as f32 / 255.0;
        let r = ((hue * 6.0).sin() * 0.5 + 0.5) * panel.light_intensity as f32;
        let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * panel.light_intensity as f32;
        let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * panel.light_intensity as f32;
        let color = Color::Rgb(r as u8, g as u8, b as u8);

        // Draw panel with border
        for py in y..(y + h).min(size.height) {
            for px in x..(x + w).min(size.width) {
                let ch = if px == x || px == x + w - 1 || py == y || py == y + h - 1 {
                    '│'
                } else {
                    '█'
                };
                let span = Span::styled(ch.to_string(), Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_hologram(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 10, 10)));
    f.render_widget(bg_fill, size);

    // Scanline
    let scan_y = state.hologram_line;
    for x in 0..size.width {
        let span = Span::styled("─", Style::default().fg(Color::Rgb(0, 255, 200)));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(x, scan_y, 1, 1);
        f.render_widget(paragraph, area);
    }

    // Holographic content (flickering grid)
    use rand::Rng;
    let mut rng = rand::thread_rng();
    for y in (0..size.height).step_by(3) {
        for x in (0..size.width).step_by(4) {
            if rng.gen_bool(0.3) {
                let intensity = rng.gen_range(50..200) as u8;
                let c = match color {
                    Color::Rgb(r, g, b) => Color::Rgb(
                        (r as u16 * intensity as u16 / 255) as u8,
                        (g as u16 * intensity as u16 / 255) as u8,
                        (b as u16 * intensity as u16 / 255) as u8,
                    ),
                    _ => Color::Rgb(0, intensity, intensity),
                };
                let span = Span::styled("╋", Style::default().fg(c));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_glitch(f: &mut Frame, state: &AnimationState, size: Rect) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Base background
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 10)));
    f.render_widget(bg_fill, size);

    if state.glitch_timer > 5 {
        // Glitch effect - random colored blocks
        for _ in 0..10 {
            let x = rng.gen_range(0..size.width);
            let y = rng.gen_range(0..size.height);
            let w = rng.gen_range(2..8);
            let h = rng.gen_range(1..3);
            let color = Color::Rgb(
                rng.gen_range(0..255),
                rng.gen_range(0..255),
                rng.gen_range(0..255),
            );

            for dy in 0..h {
                for dx in 0..w {
                    let px = x + dx;
                    let py = y + dy;
                    if px < size.width && py < size.height {
                        let span = Span::styled("█", Style::default().fg(color));
                        let text = Line::from(vec![span]);
                        let paragraph = Paragraph::new(text);
                        let area = Rect::new(px, py, 1, 1);
                        f.render_widget(paragraph, area);
                    }
                }
            }
        }
    }
}

fn render_old_film(f: &mut Frame, state: &AnimationState, size: Rect) {
    // Sepia background
    for y in 0..size.height {
        let sepia = Color::Rgb(120, 100, 70);
        let spans: Vec<Span> = (0..size.width)
            .map(|_| Span::styled("█", Style::default().fg(sepia)))
            .collect();
        let text = Line::from(spans);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(0, y, size.width, 1);
        f.render_widget(paragraph, area);
    }

    // Scratches
    for scratch in &state.scratches {
        if scratch.visible {
            for i in 0..scratch.length {
                let y = (scratch.y + i as u16) % size.height;
                let span = Span::styled("│", Style::default().fg(Color::Rgb(200, 190, 170)));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(scratch.x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Film grain
    use rand::Rng;
    let mut rng = rand::thread_rng();
    for _ in 0..50 {
        let x = rng.gen_range(0..size.width);
        let y = rng.gen_range(0..size.height);
        let intensity = rng.gen_range(150..200) as u8;
        let span = Span::styled(
            "·",
            Style::default().fg(Color::Rgb(intensity, intensity - 20, intensity - 50)),
        );
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(x, y, 1, 1);
        f.render_widget(paragraph, area);
    }
}

fn render_thermal(f: &mut Frame, state: &AnimationState, size: Rect) {
    for y in 0..size.height {
        for x in 0..size.width {
            let idx = (y * size.width + x) as usize;
            if let Some(noise) = state.thermal_noise.get(idx) {
                let temp = *noise as f32 / 255.0;
                // Thermal color mapping: black -> blue -> purple -> red -> yellow -> white
                let color = if temp < 0.2 {
                    Color::Rgb(0, 0, (temp * 5.0 * 255.0) as u8)
                } else if temp < 0.4 {
                    Color::Rgb(((temp - 0.2) * 5.0 * 255.0) as u8, 0, 255)
                } else if temp < 0.6 {
                    Color::Rgb(255, 0, (255.0 - (temp - 0.4) * 5.0 * 255.0) as u8)
                } else if temp < 0.8 {
                    Color::Rgb(255, ((temp - 0.6) * 5.0 * 255.0) as u8, 0)
                } else {
                    let c = ((temp - 0.8) * 5.0 * 255.0) as u8;
                    Color::Rgb(255, 255, c)
                };

                let chars = [' ', '░', '▒', '▓', '█'];
                let ch = chars[(temp * (chars.len() - 1) as f32) as usize];
                let span = Span::styled(ch.to_string(), Style::default().fg(color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

// Matrix characters for the animation
const MATRIX_CHARS: &[char; 49] = &[
    'ｱ', 'ｲ', 'ｳ', 'ｴ', 'ｵ', 'ｶ', 'ｷ', 'ｸ', 'ｹ', 'ｺ', 'ｻ', 'ｼ', 'ｽ', 'ｾ', 'ｿ', 'ﾀ', 'ﾁ', 'ﾂ', 'ﾃ',
    'ﾄ', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'H', 'E', 'M', 'A', 'T', 'R', 'I',
    'X', 'ﾊ', 'ﾋ', 'ﾌ', 'ﾍ', 'ﾎ', 'ﾏ', 'ﾐ', 'ﾑ', 'ﾒ', 'ﾓ',
];

// ============================================================================
// UI RENDERING
// ============================================================================

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Update and render background animation first (needs mutable borrow)
    app.update_animation(size);

    // Get config reference after mutable borrow is done
    let config = &app.config;

    // Clone config values we need to avoid borrow issues
    let auto_scale = config.layout.auto_scale;
    let render_help = config.help_text.enabled;
    let layout_mode = config.layout_mode.clone();

    render_background_animation(f, app, size);

    // Check if we're in confirmation mode
    match &app.state {
        AppState::Confirming { action_index } => {
            render_confirmation_dialog(f, app, *action_index, size);
        }
        AppState::GracePeriod {
            action_index,
            remaining_secs,
            ..
        } => {
            render_grace_period(f, app, *action_index, *remaining_secs, size);
        }
        AppState::AnimationMenu => {
            render_animation_menu(f, app, size);
        }
        AppState::Selecting => {
            // Render based on layout mode
            match layout_mode.as_str() {
                "horizontal" => render_horizontal_layout(f, app, size),
                "grid" => render_grid_layout(f, app, size),
                "compact" => render_compact_layout(f, app, size),
                _ => render_vertical_layout(f, app, size, auto_scale),
            }

            // Render help text
            if render_help {
                render_help_text(f, app, size);
            }
        }
    }
}

fn render_vertical_layout(f: &mut Frame, app: &App, size: Rect, auto_scale: bool) {
    let config = &app.config;
    let center_area = if auto_scale {
        calculate_auto_layout(f, app, size)
    } else {
        calculate_fixed_layout(f, app, size)
    };

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let selected_fg = parse_color(&config.colors.selected_fg);
    let selected_bg = parse_color(&config.colors.selected_bg);
    let selected_modifier = parse_modifier(&config.colors.selected_modifier);
    let border_color = parse_color(&config.colors.border);

    // Create list items with shortcut display
    let items: Vec<ListItem> = app
        .actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let content = action.display_text(true);
            let style = if i == app.selected_index {
                Style::default()
                    .fg(selected_fg)
                    .bg(selected_bg)
                    .add_modifier(selected_modifier)
            } else {
                Style::default().fg(fg_color)
            };
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    // Create border style
    let border_type = match config.border.style.as_str() {
        "plain" => Borders::ALL,
        "rounded" => Borders::ALL,
        "double" => Borders::ALL,
        "thick" => Borders::ALL,
        _ => Borders::ALL,
    };

    let title_alignment = match config.title_alignment.as_str() {
        "left" => Alignment::Left,
        "center" => Alignment::Center,
        "right" => Alignment::Right,
        _ => Alignment::Center,
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(if config.border.enabled {
                    border_type
                } else {
                    Borders::NONE
                })
                .title(config.title.clone())
                .title_alignment(title_alignment)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(fg_color));

    f.render_widget(list, center_area);
}

fn render_horizontal_layout(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let selected_fg = parse_color(&config.colors.selected_fg);
    let selected_bg = parse_color(&config.colors.selected_bg);
    let selected_modifier = parse_modifier(&config.colors.selected_modifier);
    let border_color = parse_color(&config.colors.border);

    // Calculate layout
    let action_count = app.actions.len() as u16;
    let item_width = 15u16; // Fixed width for each item
    let total_width = item_width * action_count + 4; // +4 for borders
    let height = 5u16;

    let x = (size.width.saturating_sub(total_width)) / 2;
    let y = (size.height.saturating_sub(height)) / 2;

    let menu_area = Rect {
        x,
        y,
        width: total_width,
        height,
    };

    // Create border
    let border_type = match config.border.style.as_str() {
        "plain" => Borders::ALL,
        "rounded" => Borders::ALL,
        "double" => Borders::ALL,
        "thick" => Borders::ALL,
        _ => Borders::ALL,
    };

    let title_alignment = match config.title_alignment.as_str() {
        "left" => Alignment::Left,
        "center" => Alignment::Center,
        "right" => Alignment::Right,
        _ => Alignment::Center,
    };

    let block = Block::default()
        .borders(if config.border.enabled {
            border_type
        } else {
            Borders::NONE
        })
        .title(config.title.clone())
        .title_alignment(title_alignment)
        .border_style(Style::default().fg(border_color));

    f.render_widget(block.clone(), menu_area);

    let inner = block.inner(menu_area);

    // Render each action horizontally
    for (i, action) in app.actions.iter().enumerate() {
        let item_x = inner.x + (i as u16 * item_width);
        if item_x + item_width > inner.x + inner.width {
            break;
        }

        let item_area = Rect {
            x: item_x,
            y: inner.y,
            width: item_width,
            height: inner.height,
        };

        let is_selected = i == app.selected_index;
        let style = if is_selected {
            Style::default()
                .fg(selected_fg)
                .bg(selected_bg)
                .add_modifier(selected_modifier)
        } else {
            Style::default().fg(fg_color)
        };

        let content = format!("\n  {}\n  {}\n", action.icon, action.label);
        let paragraph = Paragraph::new(content)
            .alignment(Alignment::Center)
            .style(style);

        f.render_widget(paragraph, item_area);
    }
}

fn render_grid_layout(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let selected_fg = parse_color(&config.colors.selected_fg);
    let selected_bg = parse_color(&config.colors.selected_bg);
    let selected_modifier = parse_modifier(&config.colors.selected_modifier);
    let border_color = parse_color(&config.colors.border);

    let cols = 2u16;
    let rows = (app.actions.len() as u16).div_ceil(cols).max(1);

    let cell_width = 20u16;
    let cell_height = 4u16;
    let total_width = cell_width * cols + 4;
    let total_height = cell_height * rows + 4;

    let x = (size.width.saturating_sub(total_width)) / 2;
    let y = (size.height.saturating_sub(total_height)) / 2;

    let menu_area = Rect {
        x,
        y,
        width: total_width,
        height: total_height,
    };

    // Create border
    let border_type = match config.border.style.as_str() {
        "plain" => Borders::ALL,
        "rounded" => Borders::ALL,
        "double" => Borders::ALL,
        "thick" => Borders::ALL,
        _ => Borders::ALL,
    };

    let title_alignment = match config.title_alignment.as_str() {
        "left" => Alignment::Left,
        "center" => Alignment::Center,
        "right" => Alignment::Right,
        _ => Alignment::Center,
    };

    let block = Block::default()
        .borders(if config.border.enabled {
            border_type
        } else {
            Borders::NONE
        })
        .title(config.title.clone())
        .title_alignment(title_alignment)
        .border_style(Style::default().fg(border_color));

    f.render_widget(block.clone(), menu_area);

    let inner = block.inner(menu_area);

    // Render actions in grid
    for (i, action) in app.actions.iter().enumerate() {
        let col = i as u16 % cols;
        let row = i as u16 / cols;

        let item_x = inner.x + (col * cell_width);
        let item_y = inner.y + (row * cell_height);

        let item_area = Rect {
            x: item_x,
            y: item_y,
            width: cell_width,
            height: cell_height,
        };

        let is_selected = i == app.selected_index;
        let style = if is_selected {
            Style::default()
                .fg(selected_fg)
                .bg(selected_bg)
                .add_modifier(selected_modifier)
        } else {
            Style::default().fg(fg_color)
        };

        let content = format!(" {} {}\n [{}] ", action.icon, action.label, action.shortcut);
        let paragraph = Paragraph::new(content)
            .alignment(Alignment::Center)
            .style(style);

        f.render_widget(paragraph, item_area);
    }
}

fn render_compact_layout(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let selected_fg = parse_color(&config.colors.selected_fg);
    let selected_bg = parse_color(&config.colors.selected_bg);
    let selected_modifier = parse_modifier(&config.colors.selected_modifier);
    let border_color = parse_color(&config.colors.border);

    // Compact horizontal layout with just icons
    let action_count = app.actions.len() as u16;
    let item_width = 5u16;
    let total_width = item_width * action_count + 4;
    let height = 4u16;

    let x = (size.width.saturating_sub(total_width)) / 2;
    let y = (size.height.saturating_sub(height)) / 2;

    let menu_area = Rect {
        x,
        y,
        width: total_width,
        height,
    };

    // Create border
    let border_type = match config.border.style.as_str() {
        "plain" => Borders::ALL,
        "rounded" => Borders::ALL,
        "double" => Borders::ALL,
        "thick" => Borders::ALL,
        _ => Borders::ALL,
    };

    let title_alignment = match config.title_alignment.as_str() {
        "left" => Alignment::Left,
        "center" => Alignment::Center,
        "right" => Alignment::Right,
        _ => Alignment::Center,
    };

    let block = Block::default()
        .borders(if config.border.enabled {
            border_type
        } else {
            Borders::NONE
        })
        .title(config.title.clone())
        .title_alignment(title_alignment)
        .border_style(Style::default().fg(border_color));

    f.render_widget(block.clone(), menu_area);

    let inner = block.inner(menu_area);

    // Render just icons
    for (i, action) in app.actions.iter().enumerate() {
        let item_x = inner.x + 1 + (i as u16 * item_width);
        if item_x >= inner.x + inner.width {
            break;
        }

        let item_area = Rect {
            x: item_x,
            y: inner.y,
            width: item_width,
            height: inner.height,
        };

        let is_selected = i == app.selected_index;
        let style = if is_selected {
            Style::default()
                .fg(selected_fg)
                .bg(selected_bg)
                .add_modifier(selected_modifier)
        } else {
            Style::default().fg(fg_color)
        };

        let content = format!("\n{}\n", action.icon);
        let paragraph = Paragraph::new(content)
            .alignment(Alignment::Center)
            .style(style);

        f.render_widget(paragraph, item_area);
    }
}

fn render_confirmation_dialog(f: &mut Frame, app: &App, action_index: usize, size: Rect) {
    let config = &app.config;
    let action = app.actions.get(action_index).unwrap();

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let selected_fg = parse_color(&config.colors.selected_fg);
    let selected_bg = parse_color(&config.colors.selected_bg);
    let selected_modifier = parse_modifier(&config.colors.selected_modifier);
    let border_color = parse_color(&config.colors.border);

    // Calculate dialog size
    let message = format!("Confirm {}?", action.label);
    let width = (message.len() as u16 + 10).max(30).min(size.width - 4);
    let height = 7u16;

    let x = (size.width.saturating_sub(width)) / 2;
    let y = (size.height.saturating_sub(height)) / 2;

    let dialog_area = Rect {
        x,
        y,
        width,
        height,
    };

    // Clear background under dialog
    let clear = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(clear, dialog_area);

    // Create dialog border
    let border_type = match config.border.style.as_str() {
        "rounded" => Borders::ALL,
        _ => Borders::ALL,
    };

    let block = Block::default()
        .borders(border_type)
        .title(" Confirm ")
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    // Render message
    let message_paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::default().fg(fg_color));
    let message_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: 1,
    };
    f.render_widget(message_paragraph, message_area);

    // Render Yes/No options - No is default (highlighted)
    let yes_style = Style::default().fg(fg_color);
    let no_style = Style::default()
        .fg(selected_fg)
        .bg(selected_bg)
        .add_modifier(selected_modifier);

    let options_text = Line::from(vec![
        Span::styled("[Y] Yes", yes_style),
        Span::raw("   "),
        Span::styled("[N] No", no_style),
    ]);

    let options_paragraph = Paragraph::new(options_text).alignment(Alignment::Center);
    let options_area = Rect {
        x: inner.x,
        y: inner.y + 3,
        width: inner.width,
        height: 1,
    };
    f.render_widget(options_paragraph, options_area);

    // Render help text
    let help_text = "Y to confirm, N/Enter to cancel, Esc to cancel";
    let help_paragraph = Paragraph::new(help_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(parse_color("gray")));
    let help_area = Rect {
        x: inner.x,
        y: inner.y + 5,
        width: inner.width,
        height: 1,
    };
    f.render_widget(help_paragraph, help_area);
}

fn render_grace_period(
    f: &mut Frame,
    app: &App,
    action_index: usize,
    remaining_secs: u64,
    size: Rect,
) {
    let config = &app.config;
    let action = app.actions.get(action_index).unwrap();

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let border_color = parse_color(&config.colors.border);
    let icon_color = parse_color(&config.colors.icon_color);

    // Build message from template
    let message = config
        .grace_period
        .message_template
        .replace("{action}", &action.label)
        .replace("{seconds}", &remaining_secs.to_string());

    // Calculate dialog size
    let width = (message.len() as u16 + 10).max(40).min(size.width - 4);
    let height = 9u16;

    let x = (size.width.saturating_sub(width)) / 2;
    let y = (size.height.saturating_sub(height)) / 2;

    let dialog_area = Rect {
        x,
        y,
        width,
        height,
    };

    // Clear background under dialog
    let clear = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(clear, dialog_area);

    // Create dialog border
    let border_type = match config.border.style.as_str() {
        "rounded" => Borders::ALL,
        _ => Borders::ALL,
    };

    let block = Block::default()
        .borders(border_type)
        .title(" Grace Period ")
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);

    // Render icon
    let icon_text = format!("{} ", action.icon);
    let icon_paragraph = Paragraph::new(icon_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(icon_color));
    let icon_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: 1,
    };
    f.render_widget(icon_paragraph, icon_area);

    // Render message
    let message_paragraph = Paragraph::new(message.clone())
        .alignment(Alignment::Center)
        .style(Style::default().fg(fg_color).add_modifier(Modifier::BOLD));
    let message_area = Rect {
        x: inner.x,
        y: inner.y + 3,
        width: inner.width,
        height: 1,
    };
    f.render_widget(message_paragraph, message_area);

    // Render countdown bar
    let total_secs = config.grace_period.duration_secs as f64;
    let progress = remaining_secs as f64 / total_secs;
    let bar_width = inner.width.saturating_sub(4) as usize;
    let filled = (bar_width as f64 * progress) as usize;
    let empty = bar_width.saturating_sub(filled);

    let filled_char = "█";
    let empty_char = "░";

    let bar = format!("{}{}", filled_char.repeat(filled), empty_char.repeat(empty));

    let bar_color = if progress > 0.6 {
        Color::Green
    } else if progress > 0.3 {
        Color::Yellow
    } else {
        Color::Red
    };

    let bar_paragraph = Paragraph::new(bar)
        .alignment(Alignment::Center)
        .style(Style::default().fg(bar_color));
    let bar_area = Rect {
        x: inner.x + 2,
        y: inner.y + 5,
        width: inner.width.saturating_sub(4),
        height: 1,
    };
    f.render_widget(bar_paragraph, bar_area);

    // Render help text
    let help_text = "Press any key to cancel";
    let help_paragraph = Paragraph::new(help_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(parse_color("gray")));
    let help_area = Rect {
        x: inner.x,
        y: inner.y + 7,
        width: inner.width,
        height: 1,
    };
    f.render_widget(help_paragraph, help_area);
}

fn render_animation_menu(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;

    // Parse colors
    let fg_color = parse_color(&config.colors.foreground);
    let selected_fg = parse_color(&config.colors.selected_fg);
    let selected_bg = parse_color(&config.colors.selected_bg);
    let selected_modifier = parse_modifier(&config.colors.selected_modifier);
    let border_color = parse_color(&config.colors.border);

    // Calculate menu size
    let max_item_len = ANIMATION_TYPES.iter().map(|s| s.len()).max().unwrap_or(10);
    let width = (max_item_len as u16 + 10).max(25).min(size.width - 4);
    let height = (ANIMATION_TYPES.len() as u16 + 4).min(size.height - 4);

    let x = (size.width.saturating_sub(width)) / 2;
    let y = (size.height.saturating_sub(height)) / 2;

    let menu_area = Rect {
        x,
        y,
        width,
        height,
    };

    // Clear background under menu
    let clear = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(clear, menu_area);

    // Create border
    let border_type = match config.border.style.as_str() {
        "rounded" => Borders::ALL,
        _ => Borders::ALL,
    };

    let block = Block::default()
        .borders(border_type)
        .title(" Select Animation ")
        .title_alignment(Alignment::Center)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(menu_area);
    f.render_widget(block, menu_area);

    // Render animation list
    let visible_items = (inner.height.saturating_sub(2)) as usize;
    let start_idx = if app.animation_menu_index >= visible_items {
        app.animation_menu_index.saturating_sub(visible_items - 1)
    } else {
        0
    };

    for (i, &animation) in ANIMATION_TYPES
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(visible_items)
    {
        let is_selected = i == app.animation_menu_index;
        let is_current = animation == config.animation.animation_type;

        let prefix = if is_current { "● " } else { "  " };
        let text = format!("{}{}", prefix, animation.replace('_', " "));

        let style = if is_selected {
            Style::default()
                .fg(selected_fg)
                .bg(selected_bg)
                .add_modifier(selected_modifier)
        } else {
            Style::default().fg(fg_color)
        };

        let item_area = Rect {
            x: inner.x + 1,
            y: inner.y + 1 + (i - start_idx) as u16,
            width: inner.width.saturating_sub(2),
            height: 1,
        };

        let paragraph = Paragraph::new(text).style(style);
        f.render_widget(paragraph, item_area);
    }

    // Render help text at bottom
    let help_text = "↑↓ navigate | Enter select | Esc/q cancel";
    let help_paragraph = Paragraph::new(help_text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(parse_color("gray")));
    let help_area = Rect {
        x: inner.x,
        y: inner.y + inner.height - 1,
        width: inner.width,
        height: 1,
    };
    f.render_widget(help_paragraph, help_area);
}

fn render_background_animation(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;

    if !config.animation.enabled || config.animation.animation_type == "none" {
        return;
    }

    // Use rainbow colors if easter egg is activated
    let animation_color = if app.easter_egg.rainbow_mode {
        Color::White
    } else {
        parse_color(&config.animation.color)
    };
    let bg_color = parse_color(&config.colors.background);

    match config.animation.animation_type.as_str() {
        "matrix" => render_matrix(
            f,
            &app.animation_state,
            size,
            animation_color,
            bg_color,
            app.easter_egg.rainbow_mode,
        ),
        "rain" => render_rain(f, &app.animation_state, size, animation_color, bg_color),
        "thunder" => render_thunder(f, &app.animation_state, size, animation_color, bg_color),
        "snow" => render_snow(f, &app.animation_state, size, animation_color, bg_color),
        "stars" => render_stars(f, &app.animation_state, size, animation_color, bg_color),
        "fireflies" => render_fireflies(
            f,
            &app.animation_state,
            size,
            animation_color,
            bg_color,
            app.easter_egg.rainbow_mode,
        ),
        "bubbles" => render_bubbles(f, &app.animation_state, size, animation_color, bg_color),
        "confetti" => render_confetti(f, &app.animation_state, size, bg_color),
        "wave" => render_wave(f, &app.animation_state, size, animation_color, bg_color),
        "particles" => render_particles(f, &app.animation_state, size, bg_color),
        "digital_rain" => render_digital_rain(
            f,
            &app.animation_state,
            size,
            animation_color,
            bg_color,
            app.easter_egg.rainbow_mode,
        ),
        "heartbeat" => render_heartbeat(f, app, size, bg_color),
        "plasma" => render_plasma(f, &app.animation_state, size),
        "scanlines" => render_scanlines(f, &app.animation_state, size, animation_color),
        "aurora" => render_aurora(f, &app.animation_state, size),
        "autumn" => render_autumn(f, &app.animation_state, size),
        "dna" => render_dna(f, &app.animation_state, size, animation_color),
        "synthwave" => render_synthwave(f, &app.animation_state, size, animation_color),
        "smoke" => render_smoke(f, &app.animation_state, size),
        "gradient_flow" => render_gradient_flow(f, &app.animation_state, size),
        "constellation" => render_constellation(f, &app.animation_state, size, animation_color),
        "fish_tank" => render_fish_tank(f, &app.animation_state, size),
        "typing_code" => render_typing_code(f, &app.animation_state, size, animation_color),
        "vortex" => render_vortex(f, &app.animation_state, size, animation_color),
        "circuit" => render_circuit(f, &app.animation_state, size, animation_color),
        "flow_field" => render_flow_field(f, &app.animation_state, size),
        "morse" => render_morse(f, &app.animation_state, size, animation_color),
        "lissajous" => render_lissajous(f, &app.animation_state, size),
        "game_of_life" => render_game_of_life(f, &app.animation_state, size),
        "matrix_cjk" => render_matrix_cjk(
            f,
            &app.animation_state,
            size,
            animation_color,
            bg_color,
            app.easter_egg.rainbow_mode,
        ),
        "fireworks" => render_fireworks(f, &app.animation_state, size, bg_color),
        "neon_grid" => render_neon_grid(f, &app.animation_state, size, animation_color),
        "perlin_flow" => render_perlin_flow(f, &app.animation_state, size, animation_color),
        "cube_3d" => render_cube_3d(f, &app.animation_state, size, animation_color),
        "fractals" => render_fractals(f, &app.animation_state, size, animation_color),
        // New animations v1.1.5
        "ocean" => render_ocean(f, &app.animation_state, size),
        "ripple" => render_ripple(f, &app.animation_state, size, animation_color),
        "fog" => render_fog(f, &app.animation_state, size),
        "flames" => render_flames(f, &app.animation_state, size),
        "sparks" => render_sparks(f, &app.animation_state, size),
        "lava_lamp" => render_lava_lamp(f, &app.animation_state, size),
        "sun" => render_sun(f, &app.animation_state, size),
        "galaxy" => render_galaxy(f, &app.animation_state, size),
        "meteor_shower" => render_meteor_shower(f, &app.animation_state, size),
        "satellite" => render_satellite(f, &app.animation_state, size),
        "pulsar" => render_pulsar(f, &app.animation_state, size, animation_color),
        "pong" => render_pong(f, &app.animation_state, size),
        "snake" => render_snake(f, &app.animation_state, size),
        "tetris" => render_tetris(f, &app.animation_state, size),
        "invaders" => render_invaders(f, &app.animation_state, size),
        "fibonacci" => render_fibonacci(f, &app.animation_state, size, animation_color),
        "mandelbrot" => render_mandelbrot(f, &app.animation_state, size, animation_color),
        "hex_grid" => render_hex_grid(f, &app.animation_state, size),
        "rose" => render_rose(f, &app.animation_state, size, animation_color),
        "butterflies" => render_butterflies(f, &app.animation_state, size),
        "spider_web" => render_spider_web(f, &app.animation_state, size),
        "vine_growth" => render_vine_growth(f, &app.animation_state, size),
        "moss" => render_moss(f, &app.animation_state, size),
        "radar" => render_radar(f, &app.animation_state, size, animation_color),
        "binary_clock" => render_binary_clock(f, &app.animation_state, size),
        "signal" => render_signal(f, &app.animation_state, size),
        "wifi" => render_wifi(f, &app.animation_state, size),
        "paint_splatter" => render_paint_splatter(f, &app.animation_state, size),
        "ink_bleed" => render_ink_bleed(f, &app.animation_state, size),
        "mosaic" => render_mosaic(f, &app.animation_state, size),
        "stained_glass" => render_stained_glass(f, &app.animation_state, size),
        "hologram" => render_hologram(f, &app.animation_state, size, animation_color),
        "glitch" => render_glitch(f, &app.animation_state, size),
        "old_film" => render_old_film(f, &app.animation_state, size),
        "thermal" => render_thermal(f, &app.animation_state, size),
        _ => {}
    }
}

fn render_matrix(
    f: &mut Frame,
    state: &AnimationState,
    size: Rect,
    color: Color,
    _bg: Color,
    rainbow: bool,
) {
    // Fill background with black first to avoid gray stripes
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    // Build each line of the matrix
    for y in 0..size.height {
        let mut line_chars: Vec<(char, Color)> = vec![];

        for col in &state.matrix_columns {
            let head_y = col.y as u16;
            let trail_length = 8u16;

            // Check if this column has content at this y position
            if col.x >= size.width {
                continue;
            }

            // Calculate trail
            for i in 0..=trail_length {
                let trail_y = head_y.saturating_sub(i);
                if trail_y == y {
                    let fade_factor = if i == 0 {
                        1.0 // Head is brightest
                    } else {
                        (trail_length - i) as f32 / trail_length as f32
                    };

                    let intensity = (fade_factor * 255.0) as u8;

                    let char_color = if rainbow {
                        // Rainbow effect based on position and time
                        let hue = ((col.x as f32 + state.tick as f32) % 360.0) / 360.0;
                        let r = ((hue * 6.0).sin() * 0.5 + 0.5) * intensity as f32;
                        let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * intensity as f32;
                        let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * intensity as f32;
                        Color::Rgb(r as u8, g as u8, b as u8)
                    } else {
                        match color {
                            Color::Green => Color::Rgb(0, intensity, 0),
                            Color::Blue => Color::Rgb(0, 0, intensity),
                            Color::Cyan => Color::Rgb(0, intensity, intensity),
                            _ => Color::Rgb(intensity, intensity, intensity),
                        }
                    };

                    let ch = if i == 0 {
                        MATRIX_CHARS[col.char_idx]
                    } else {
                        // Use different char for trail
                        MATRIX_CHARS[(col.char_idx + i as usize) % MATRIX_CHARS.len()]
                    };

                    // Store at correct x position
                    while line_chars.len() <= col.x as usize {
                        line_chars.push((' ', parse_color("black")));
                    }
                    line_chars[col.x as usize] = (ch, char_color);
                }
            }
        }

        // Build spans for this line
        let spans: Vec<Span> = line_chars
            .into_iter()
            .map(|(ch, col)| Span::styled(ch.to_string(), Style::default().fg(col)))
            .collect();

        if !spans.is_empty() {
            let text = Line::from(spans);
            let paragraph = Paragraph::new(text).style(Style::default().bg(parse_color("black")));
            let area = Rect::new(0, y, size.width, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_rain(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    // Fill background with black first to avoid gray stripes
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for drop in &state.rain_drops {
        let y = drop.y as u16;
        if y < size.height {
            let rain_char = if drop.speed > 1.5 { "│" } else { "┆" };
            let intensity = 100 + (drop.speed * 50.0) as u8;

            let rain_color = match color {
                Color::Blue => Color::Rgb(100, 100, intensity),
                Color::Cyan => Color::Rgb(100, intensity, intensity),
                Color::White => Color::Rgb(intensity, intensity, intensity + 50),
                _ => color,
            };

            let span = Span::styled(rain_char, Style::default().fg(rain_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(drop.x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_thunder(f: &mut Frame, state: &AnimationState, size: Rect, _color: Color, bg: Color) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Flash effect
    if state.thunder_flash > 0 {
        let flash_color = Color::Rgb(240, 240, 255);
        // Fill with very dark blue/black background during flash
        let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
        f.render_widget(bg_fill, size);

        // Random lightning bolt
        if state.thunder_flash > 2 {
            let start_x = rng.gen_range(5..size.width.saturating_sub(5));
            let mut current_x = start_x;
            let mut current_y = 0u16;

            while current_y < size.height {
                let bolt_char = if rng.gen_bool(0.5) { "│" } else { "╱" };
                let span = Span::styled(bolt_char, Style::default().fg(flash_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(current_x, current_y, 1, 1);
                f.render_widget(paragraph, area);

                current_y += 1;
                if rng.gen_bool(0.3) {
                    current_x = current_x.saturating_add(1).min(size.width - 1);
                } else if rng.gen_bool(0.3) {
                    current_x = current_x.saturating_sub(1);
                }
            }
        }
    } else {
        // Dark, moody background with occasional distant flashes
        let _dark_bg = match bg {
            Color::Black => Color::Rgb(10, 10, 15),
            _ => bg,
        };

        // Fill background with very dark color
        let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
        f.render_widget(bg_fill, size);

        // Occasional distant lightning glow
        if rng.gen_bool(0.05) {
            let glow_x = rng.gen_range(0..size.width);
            let glow_y = rng.gen_range(0..size.height.saturating_sub(5));
            let glow_span = Span::styled("░", Style::default().fg(Color::Rgb(30, 30, 40)));
            let text = Line::from(vec![glow_span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(glow_x, glow_y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_snow(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    // Fill background with black first to avoid gray stripes
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for flake in &state.snow_flakes {
        let y = flake.y as u16;
        let x = flake.x as u16;
        if y < size.height && x < size.width {
            let snow_char = match flake.size {
                1 => "·",
                2 => "•",
                _ => "*",
            };

            let intensity = 150 + flake.size * 30;
            let snow_color = match color {
                Color::White => Color::Rgb(intensity, intensity, intensity),
                _ => color,
            };

            let span = Span::styled(snow_char, Style::default().fg(snow_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_stars(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    // Fill background with black first to avoid gray stripes
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for star in &state.stars {
        if star.x < size.width && star.y < size.height {
            let star_char = if star.brightness > 200 { "★" } else { "☆" };
            let intensity = star.brightness;

            let star_color = match color {
                Color::Yellow => Color::Rgb(intensity, intensity, intensity / 2),
                Color::White => Color::Rgb(intensity, intensity, intensity),
                _ => color,
            };

            let span = Span::styled(star_char, Style::default().fg(star_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(star.x, star.y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_fireflies(
    f: &mut Frame,
    state: &AnimationState,
    size: Rect,
    color: Color,
    _bg: Color,
    rainbow: bool,
) {
    // Fill background with black first to avoid gray stripes
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for (i, firefly) in state.fireflies.iter().enumerate() {
        let y = firefly.y as u16;
        let x = firefly.x as u16;
        if y < size.height && x < size.width {
            let intensity = firefly.brightness;
            let firefly_color = if rainbow {
                let hue = ((i as f32 * 30.0 + state.tick as f32) % 360.0) / 360.0;
                let r = ((hue * 6.0).sin() * 0.5 + 0.5) * intensity as f32;
                let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * intensity as f32;
                let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * intensity as f32;
                Color::Rgb(r as u8, g as u8, b as u8)
            } else {
                match color {
                    Color::Yellow => Color::Rgb(intensity, intensity, 0),
                    Color::Green => Color::Rgb(0, intensity, 0),
                    _ => Color::Rgb(intensity, intensity, intensity / 2),
                }
            };

            let span = Span::styled("●", Style::default().fg(firefly_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_bubbles(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for bubble in &state.bubbles {
        let y = bubble.y as u16;
        let x = bubble.x as u16;
        if y < size.height && x < size.width {
            let bubble_char = match bubble.size {
                1 => "○",
                2 => "◎",
                _ => "◉",
            };

            let alpha = 150 + bubble.size * 30;
            let bubble_color = match color {
                Color::Blue => Color::Rgb(alpha / 2, alpha / 2, alpha),
                Color::Cyan => Color::Rgb(alpha / 2, alpha, alpha),
                Color::White => Color::Rgb(alpha, alpha, alpha),
                _ => color,
            };

            let span = Span::styled(bubble_char, Style::default().fg(bubble_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_confetti(f: &mut Frame, state: &AnimationState, size: Rect, _bg: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for conf in &state.confetti {
        let y = conf.y as u16;
        let x = conf.x as u16;
        if y < size.height && x < size.width {
            // HSL to RGB conversion for rainbow colors
            let hue = conf.color as f32 / 255.0;
            let r = ((hue * 6.0).sin() * 0.5 + 0.5) * 255.0;
            let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * 255.0;
            let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * 255.0;

            let conf_color = Color::Rgb(r as u8, g as u8, b as u8);

            let span = Span::styled(conf.character.to_string(), Style::default().fg(conf_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_wave(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for y in 0..size.height {
        let wave_y =
            ((y as f32 * 0.3 + state.wave_offset).sin() * 5.0) as i16 + (size.width / 2) as i16;
        let wave_y = wave_y.max(0) as u16;

        if wave_y < size.width {
            let intensity = 100 + ((y as f32 / size.height as f32) * 155.0) as u8;
            let wave_color = match color {
                Color::Blue => Color::Rgb(0, intensity / 2, intensity),
                Color::Cyan => Color::Rgb(0, intensity, intensity),
                Color::Green => Color::Rgb(0, intensity, intensity / 2),
                _ => Color::Rgb(intensity, intensity, intensity),
            };

            let wave_char = if y % 2 == 0 { "≈" } else { "~" };
            let span = Span::styled(wave_char, Style::default().fg(wave_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(wave_y, y, 1, 1);
            f.render_widget(paragraph, area);

            // Second wave offset
            let wave_y2 = ((y as f32 * 0.2 + state.wave_offset + 2.0).sin() * 5.0) as i16
                + (size.width / 2) as i16;
            let wave_y2 = (wave_y2 + 10).max(0) as u16;
            if wave_y2 < size.width && wave_y2 != wave_y {
                let span2 = Span::styled(wave_char, Style::default().fg(wave_color));
                let text2 = Line::from(vec![span2]);
                let paragraph2 = Paragraph::new(text2);
                let area2 = Rect::new(wave_y2, y, 1, 1);
                f.render_widget(paragraph2, area2);
            }
        }
    }
}

fn render_particles(f: &mut Frame, state: &AnimationState, size: Rect, _bg: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for particle in &state.particles {
        let y = particle.y as u16;
        let x = particle.x as u16;
        if y < size.height && x < size.width {
            let alpha = (particle.life as f32 / particle.max_life as f32 * 255.0) as u8;
            let color = match particle.color {
                Color::Rgb(r, g, b) => Color::Rgb(
                    (r as f32 * alpha as f32 / 255.0) as u8,
                    (g as f32 * alpha as f32 / 255.0) as u8,
                    (b as f32 * alpha as f32 / 255.0) as u8,
                ),
                _ => particle.color,
            };

            let particle_char = "•";
            let span = Span::styled(particle_char, Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_digital_rain(
    f: &mut Frame,
    state: &AnimationState,
    size: Rect,
    color: Color,
    _bg: Color,
    rainbow: bool,
) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let hex_chars = "0123456789ABCDEF";

    for col in &state.matrix_columns {
        let head_y = col.y as u16;
        let trail_length = 6u16;

        for i in 0..=trail_length {
            let trail_y = head_y.saturating_sub(i);
            if trail_y >= size.height {
                continue;
            }

            let fade_factor = if i == 0 {
                1.0
            } else {
                (trail_length - i) as f32 / trail_length as f32
            };

            let intensity = (fade_factor * 255.0) as u8;
            let ch = hex_chars.chars().nth(col.char_idx % 16).unwrap_or('0');

            let char_color = if rainbow {
                let hue = ((col.x as f32 + state.tick as f32 * 2.0) % 360.0) / 360.0;
                let r = ((hue * 6.0).sin() * 0.5 + 0.5) * intensity as f32;
                let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * intensity as f32;
                let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * intensity as f32;
                Color::Rgb(r as u8, g as u8, b as u8)
            } else {
                match color {
                    Color::Green => Color::Rgb(0, intensity, 0),
                    Color::Blue => Color::Rgb(0, 0, intensity),
                    Color::Cyan => Color::Rgb(0, intensity, intensity),
                    _ => Color::Rgb(intensity, intensity, intensity),
                }
            };

            let span = Span::styled(ch.to_string(), Style::default().fg(char_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(col.x, trail_y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_heartbeat(f: &mut Frame, app: &App, size: Rect, _bg: Color) {
    let phase = app.animation_state.heartbeat_phase;
    let beat = (phase.sin() * 0.5 + 0.5) * 0.3 + 0.1;
    let intensity = (beat * 255.0) as u8;

    let bg_color = Color::Rgb(intensity / 4, 0, intensity / 8);
    let bg_fill = Block::default().style(Style::default().bg(bg_color));
    f.render_widget(bg_fill, size);

    // Draw pulse line
    let center_y = size.height / 2;
    for x in 0..size.width {
        let local_phase = (x as f32 * 0.3 + phase * 3.0) % std::f32::consts::TAU;
        let pulse = local_phase.sin() * (beat * 3.0);
        let y = ((center_y as i16 + pulse as i16).max(0) as u16).min(size.height - 1);

        let line_color = Color::Rgb(intensity, intensity / 2, intensity / 2);
        let span = Span::styled("█", Style::default().fg(line_color));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(x, y, 1, 1);
        f.render_widget(paragraph, area);
    }
}

fn render_plasma(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for cell in &state.plasma {
        if cell.x >= size.width || cell.y >= size.height {
            continue;
        }

        let value = cell.value;
        let intensity = ((value + 1.0) * 127.5) as u8;

        // Plasma colors: blue -> purple -> red -> yellow
        let r = if value > 0.0 { intensity } else { 0 };
        let g = if value.abs() < 0.5 { intensity } else { 0 };
        let b = if value < 0.0 {
            intensity
        } else {
            intensity / 2
        };

        let plasma_color = Color::Rgb(r, g, b);
        let ch = if value > 0.5 {
            "█"
        } else if value > 0.0 {
            "▓"
        } else {
            "▒"
        };

        let span = Span::styled(ch, Style::default().fg(plasma_color));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(cell.x, cell.y, 1, 1);
        f.render_widget(paragraph, area);
    }
}

fn render_scanlines(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    // Dark background
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 5)));
    f.render_widget(bg_fill, size);

    // Render scanlines - fill entire lines
    for y in 0..size.height {
        let is_scanline = (y + state.scanline_pos) % 4 == 0;
        let line_color = if is_scanline {
            color
        } else {
            Color::Rgb(15, 15, 15)
        };

        // Create a full-width span with spaces for background color
        let line_spans: Vec<Span> = (0..size.width)
            .map(|_| Span::styled("█", Style::default().fg(line_color)))
            .collect();
        let text = Line::from(line_spans);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(0, y, size.width, 1);
        f.render_widget(paragraph, area);
    }

    // Occasional glitch effect
    use rand::Rng;
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.02) {
        let glitch_y = rng.gen_range(0..size.height);
        let glitch_color = Color::Rgb(
            rng.gen_range(100..255),
            rng.gen_range(100..255),
            rng.gen_range(100..255),
        );
        let glitch_spans: Vec<Span> = (0..size.width)
            .map(|_| Span::styled("░", Style::default().fg(glitch_color)))
            .collect();
        let glitch_text = Line::from(glitch_spans);
        let glitch_paragraph = Paragraph::new(glitch_text);
        let glitch_area = Rect::new(0, glitch_y, size.width, 1);
        f.render_widget(glitch_paragraph, glitch_area);
    }
}

fn render_aurora(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 15)));
    f.render_widget(bg_fill, size);

    let phase = state.aurora_phase;

    for y in 0..size.height {
        let wave1 = ((y as f32 * 0.1 + phase).sin() * 10.0) as i16;
        let wave2 = ((y as f32 * 0.05 - phase * 0.7).sin() * 8.0) as i16;

        for i in 0..3 {
            let x1 = (size.width as i16 / 2 + wave1 + i * 5).max(0) as u16;
            let x2 = (size.width as i16 / 3 + wave2 + i * 4).max(0) as u16;

            if x1 < size.width {
                let green = (150 + i * 30).min(255) as u8;
                let aurora_color = Color::Rgb(0, green, 100);
                let span = Span::styled("█", Style::default().fg(aurora_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x1, y, 1, 1);
                f.render_widget(paragraph, area);
            }

            if x2 < size.width && x2 != x1 {
                let purple = (100 + i * 20).min(255) as u8;
                let aurora_color = Color::Rgb(purple, 0, 150);
                let span = Span::styled("█", Style::default().fg(aurora_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x2, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_autumn(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(20, 15, 10)));
    f.render_widget(bg_fill, size);

    let autumn_colors = [
        Color::Rgb(200, 80, 0),  // Orange
        Color::Rgb(180, 50, 0),  // Red-orange
        Color::Rgb(160, 40, 20), // Red
        Color::Rgb(200, 160, 0), // Gold
    ];

    for leaf in &state.leaves {
        let y = leaf.y as u16;
        let x = leaf.x as u16;

        if y < size.height && x < size.width {
            let leaf_chars = ["🍂", "🍁", "•", "◦"];
            let leaf_char = leaf_chars[leaf.color as usize % leaf_chars.len()];
            let color = autumn_colors[leaf.color as usize % autumn_colors.len()];

            let span = Span::styled(leaf_char, Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_dna(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    // Define multiple helix center positions based on terminal width
    let num_helixes = ((size.width as usize) / 25).max(1).min(4);
    let spacing = size.width / (num_helixes as u16 + 1);

    for helix_idx in 0..num_helixes {
        let center_x = spacing * (helix_idx as u16 + 1);
        let phase_offset = helix_idx as f32 * 1.5; // Offset phase for visual variety

        for base in &state.dna {
            let y = base.y as u16;
            if y >= size.height {
                continue;
            }

            // Calculate helix offset - double helix with phase offset
            let phase1 = base.y * 0.3 + phase_offset;
            let phase2 = phase1 + std::f32::consts::PI; // 180 degree offset for second strand

            let offset1 = (phase1.sin() * 5.0) as i16;
            let offset2 = (phase2.sin() * 5.0) as i16;

            let strand1_x = (center_x as i16 + offset1).max(0) as u16;
            let strand2_x = (center_x as i16 + offset2).max(0) as u16;

            // Alternate colors for different helixes
            let hue_shift = (helix_idx * 60) as u8;
            let color1 = Color::Rgb(
                0u8,
                255u8.saturating_sub(hue_shift),
                100u8.saturating_add(hue_shift),
            );
            let color2 = Color::Rgb(
                255u8.saturating_sub(hue_shift),
                150u8.saturating_sub(hue_shift / 2),
                hue_shift,
            );

            // Draw first strand
            if strand1_x < size.width {
                let span = Span::styled(base.left_char.to_string(), Style::default().fg(color1));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(strand1_x, y, 1, 1);
                f.render_widget(paragraph, area);
            }

            // Draw second strand and connection
            if strand2_x < size.width {
                // Draw connection between strands if they connect
                if base.connection {
                    let min_x = strand1_x.min(strand2_x);
                    let max_x = strand1_x.max(strand2_x);

                    for cx in min_x..=max_x {
                        if cx < size.width {
                            let ch = if cx == strand1_x || cx == strand2_x {
                                "●" // Base marker
                            } else {
                                "=" // Hydrogen bond connection
                            };
                            let span = Span::styled(ch, Style::default().fg(color));
                            let text = Line::from(vec![span]);
                            let paragraph = Paragraph::new(text);
                            let area = Rect::new(cx, y, 1, 1);
                            f.render_widget(paragraph, area);
                        }
                    }
                }

                // Draw second strand base
                if strand2_x < size.width {
                    let span =
                        Span::styled(base.right_char.to_string(), Style::default().fg(color2));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(strand2_x, y, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }
}

fn render_synthwave(f: &mut Frame, state: &AnimationState, size: Rect, _color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 0, 20)));
    f.render_widget(bg_fill, size);

    // Draw sunset gradient background
    for y in 0..(size.height / 2) {
        let intensity = (y as f32 / (size.height / 2) as f32 * 100.0) as u8;
        let sunset_color = Color::Rgb(150 + intensity, 50, 100 + intensity / 2);
        let spans: Vec<Span> = (0..size.width)
            .map(|_| Span::styled("█", Style::default().fg(sunset_color)))
            .collect();
        let text = Line::from(spans);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(0, y, size.width, 1);
        f.render_widget(paragraph, area);
    }

    // Draw sun with horizontal stripes (synthwave style)
    let sun_y = size.height / 3;
    let sun_radius = (size.height / 6).min(12) as i16;
    for y_offset in -sun_radius..=0 {
        let sun_row_y = (sun_y as i16 + y_offset) as u16;
        if sun_row_y >= size.height / 2 {
            continue;
        }
        // Calculate width of sun at this height
        let width_at_height =
            ((sun_radius * sun_radius - y_offset * y_offset) as f32).sqrt() as i16;
        let start_x = (size.width / 2).saturating_sub(width_at_height as u16);
        let end_x = (size.width / 2 + width_at_height as u16).min(size.width);

        for px in start_x..end_x {
            if px < size.width {
                // Cut out horizontal lines for synthwave sun effect
                let stripe_pattern = (sun_row_y as i16 + state.synthwave_offset as i16) % 3 == 0;
                let sun_color = if stripe_pattern {
                    Color::Rgb(255, 50, 100) // Darker stripe
                } else {
                    Color::Rgb(255, 200, 100) // Bright sun
                };
                let span = Span::styled("█", Style::default().fg(sun_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, sun_row_y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }

    // Draw horizon line
    let horizon_y = size.height / 2;
    let horizon_spans: Vec<Span> = (0..size.width)
        .map(|_| Span::styled("▄", Style::default().fg(Color::Rgb(255, 100, 200))))
        .collect();
    let horizon_text = Line::from(horizon_spans);
    let horizon_paragraph = Paragraph::new(horizon_text);
    let horizon_area = Rect::new(0, horizon_y, size.width, 1);
    f.render_widget(horizon_paragraph, horizon_area);

    // Draw perspective grid
    let offset = (state.synthwave_offset as u16) % 4;
    let grid_start = horizon_y + 1;

    // Horizontal grid lines with perspective
    for y in (grid_start..size.height).step_by(2) {
        let distance = (y - grid_start) as f32;
        let perspective_gap = (1.0 + distance * 0.1) as u16;
        if (y + offset) % perspective_gap == 0 {
            let line_color = Color::Rgb(
                100 + (distance * 2.0) as u8,
                0,
                150 + (distance * 2.0) as u8,
            );
            let spans: Vec<Span> = (0..size.width)
                .map(|_| Span::styled("─", Style::default().fg(line_color)))
                .collect();
            let text = Line::from(spans);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(0, y, size.width, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Vertical perspective lines radiating from center
    let center_x = size.width / 2;
    let num_vertical_lines = 9;
    for i in 0..num_vertical_lines {
        let angle = (i as f32 - (num_vertical_lines as f32 / 2.0)) * 0.15;
        for y in grid_start..size.height {
            let progress = (y - grid_start) as f32 / (size.height - grid_start) as f32;
            let x_offset = (angle * progress * size.width as f32) as i16;
            let x = (center_x as i16 + x_offset).max(0) as u16;
            if x < size.width {
                let line_color = Color::Rgb(
                    (100.0 + progress * 100.0) as u8,
                    0,
                    (150.0 + progress * 100.0) as u8,
                );
                let span = Span::styled("│", Style::default().fg(line_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_smoke(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for particle in &state.smoke {
        let y = particle.y as u16;
        let x = particle.x as u16;

        if y < size.height && x < size.width {
            let alpha = (particle.life as f32 / particle.max_life as f32 * 100.0) as u8 + 50;
            let smoke_color = Color::Rgb(alpha, alpha, alpha);

            let smoke_chars = ["░", "▒", "▓"];
            let ch = smoke_chars[(particle.life % 3) as usize];

            let span = Span::styled(ch, Style::default().fg(smoke_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_gradient_flow(f: &mut Frame, state: &AnimationState, size: Rect) {
    let phase = state.gradient_phase;

    for y in 0..size.height {
        for x in 0..size.width {
            let hue = (x as f32 * 0.02 + y as f32 * 0.01 + phase) % 1.0;
            let r = ((hue * 6.0).sin() * 0.5 + 0.5) * 255.0;
            let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * 255.0;
            let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * 255.0;

            let color = Color::Rgb(r as u8, g as u8, b as u8);
            let span = Span::styled("█", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_constellation(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
    f.render_widget(bg_fill, size);

    // Draw connections
    for (i, node1) in state.nodes.iter().enumerate() {
        for node2 in state.nodes.iter().skip(i + 1) {
            let dx = node1.x - node2.x;
            let dy = node1.y - node2.y;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq < 400.0 {
                // Draw line between close nodes
                let mid_x = ((node1.x + node2.x) / 2.0) as u16;
                let mid_y = ((node1.y + node2.y) / 2.0) as u16;

                if mid_x < size.width && mid_y < size.height {
                    let alpha = (1.0 - dist_sq / 400.0) * 150.0;
                    let line_color =
                        Color::Rgb((alpha * 0.5) as u8, (alpha * 0.7) as u8, alpha as u8);
                    let span = Span::styled("·", Style::default().fg(line_color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(mid_x, mid_y, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }

    // Draw nodes
    for node in &state.nodes {
        let x = node.x as u16;
        let y = node.y as u16;

        if x < size.width && y < size.height {
            let span = Span::styled("●", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_fish_tank(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(0, 30, 60)));
    f.render_widget(bg_fill, size);

    // Draw bubbles
    for bubble in &state.bubbles {
        let y = bubble.y as u16;
        let x = bubble.x as u16;

        if y < size.height && x < size.width {
            let span = Span::styled("○", Style::default().fg(Color::Rgb(200, 200, 255)));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Draw fish
    let fish_colors = [
        Color::Rgb(255, 150, 0),
        Color::Rgb(255, 200, 0),
        Color::Rgb(200, 100, 255),
        Color::Rgb(100, 255, 150),
        Color::Rgb(255, 100, 100),
    ];

    for fish in &state.fish {
        let y = fish.y as u16;
        let x = fish.x as u16;

        // Check bounds properly - fish is 3 characters wide
        if y < size.height && x < size.width.saturating_sub(2) {
            let fish_char = if fish.direction { "><>" } else { "<><" };
            let color = fish_colors[fish.color as usize % fish_colors.len()];

            let span = Span::styled(fish_char, Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 3, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_typing_code(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 15)));
    f.render_widget(bg_fill, size);

    // Render typed code
    let mut y = 1u16;
    for (i, line) in state.code_lines.iter().enumerate() {
        if y >= size.height - 1 {
            break;
        }

        let display_line = if i < state.code_line_idx {
            line.clone()
        } else if i == state.code_line_idx {
            line.chars().take(state.code_char_idx).collect()
        } else {
            String::new()
        };

        if !display_line.is_empty() {
            let span = Span::styled(display_line, Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(1, y, size.width - 2, 1);
            f.render_widget(paragraph, area);
        }

        y += 1;
    }

    // Draw cursor
    let cursor_y = (state.code_line_idx + 1) as u16;
    if cursor_y < size.height - 1 {
        let span = Span::styled("█", Style::default().fg(Color::White));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(1, cursor_y, 1, 1);
        f.render_widget(paragraph, area);
    }
}

fn render_vortex(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let angle = state.vortex_angle;

    // Draw spiral arms
    for arm in 0..3 {
        let arm_offset = (arm as f32) * 2.094; // 120 degrees
        for r in (0..50).step_by(2) {
            let rad = r as f32 * 0.3;
            let x = center_x + (angle + arm_offset + rad * 0.2).cos() * rad;
            let y = center_y + (angle + arm_offset + rad * 0.2).sin() * rad;

            let px = x as u16;
            let py = y as u16;

            if px < size.width && py < size.height {
                let intensity = (255 - r * 4).max(50) as u8;
                let vortex_color = match color {
                    Color::Rgb(r, g, b) => Color::Rgb(
                        (r as u16 * intensity as u16 / 255) as u8,
                        (g as u16 * intensity as u16 / 255) as u8,
                        (b as u16 * intensity as u16 / 255) as u8,
                    ),
                    _ => Color::Rgb(intensity, intensity, intensity),
                };

                let span = Span::styled("◆", Style::default().fg(vortex_color));
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_circuit(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 10, 5)));
    f.render_widget(bg_fill, size);

    for trace in &state.traces {
        let x = trace.x;
        let y = trace.y;

        if x < size.width && y < size.height {
            let intensity = (trace.life as f32 / 150.0 * 255.0) as u8;
            let trace_color = Color::Rgb(0, intensity, intensity / 2);

            let ch = match trace.direction {
                0 | 2 => "│",
                _ => "─",
            };

            let span = Span::styled(ch, Style::default().fg(trace_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Draw circuit nodes
    for trace in &state.traces {
        if trace.life > 100 && trace.x < size.width && trace.y < size.height {
            let span = Span::styled("●", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(trace.x, trace.y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_flow_field(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for particle in &state.flow_particles {
        let x = particle.x as u16;
        let y = particle.y as u16;

        if x < size.width && y < size.height {
            let hue = particle.color as f32 / 255.0;
            let r = ((hue * 6.0).sin() * 0.5 + 0.5) * 255.0;
            let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * 255.0;
            let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * 255.0;

            let color = Color::Rgb(r as u8, g as u8, b as u8);
            let span = Span::styled("·", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_morse(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(5, 5, 10)));
    f.render_widget(bg_fill, size);

    // Render morse code at top of screen
    let morse_text = &state.morse_display;
    let lines: Vec<&str> = morse_text.lines().collect();

    for (i, line) in lines.iter().enumerate().take(size.height as usize) {
        let span = Span::styled(*line, Style::default().fg(color));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(1, i as u16, size.width - 2, 1);
        f.render_widget(paragraph, area);
    }

    // Show current character being transmitted
    if state.morse_idx < state.morse_message.len() {
        let current_ch = state.morse_message.chars().nth(state.morse_idx).unwrap();
        let status = format!("Transmitting: {}", current_ch);
        let span = Span::styled(status, Style::default().fg(Color::Rgb(100, 100, 100)));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(1, size.height - 2, size.width - 2, 1);
        f.render_widget(paragraph, area);
    }
}

fn render_lissajous(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;

    for curve in &state.lissajous {
        let x = center_x + (curve.a * curve.t + curve.delta).sin() * center_x * 0.8;
        let y = center_y + (curve.b * curve.t).sin() * center_y * 0.8;

        let px = x as u16;
        let py = y as u16;

        if px < size.width && py < size.height {
            let hue = curve.color as f32 / 255.0;
            let r = ((hue * 6.0).sin() * 0.5 + 0.5) * 255.0;
            let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * 255.0;
            let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * 255.0;

            let color = Color::Rgb(r as u8, g as u8, b as u8);
            let span = Span::styled("●", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(px, py, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_game_of_life(f: &mut Frame, state: &AnimationState, size: Rect) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let width = state.gol_width;
    if width == 0 {
        return;
    }

    for cell in &state.gol_grid {
        if cell.x >= size.width as usize || cell.y >= size.height as usize {
            continue;
        }

        if cell.alive {
            let age_factor = (cell.age as f32 / 50.0).min(1.0);
            let color = Color::Rgb(
                (50.0 + age_factor * 150.0) as u8,
                (100.0 + age_factor * 100.0) as u8,
                200,
            );

            let span = Span::styled("█", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(cell.x as u16, cell.y as u16, 1, 1);
            f.render_widget(paragraph, area);
        }
    }
}

fn render_matrix_cjk(
    f: &mut Frame,
    state: &AnimationState,
    size: Rect,
    color: Color,
    _bg: Color,
    rainbow: bool,
) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    // CJK characters for authentic Matrix feel
    const CJK_CHARS: &[char] = &[
        'ﾊ', 'ﾐ', 'ﾋ', 'ｰ', 'ｳ', 'ｼ', 'ﾅ', 'ﾓ', 'ﾆ', 'ｻ', 'ﾜ', 'ﾂ', 'ｵ', 'ﾘ', 'ｱ', 'ﾎ', 'ﾃ', 'ﾏ',
        'ｹ', 'ﾒ', 'ｴ', 'ｶ', 'ｷ', 'ﾑ', 'ﾕ', 'ﾗ', 'ｾ', 'ﾈ', 'ｽ', 'ﾀ', 'ﾇ', 'ﾍ', 'ｦ', 'ｲ', 'ｸ', 'ｺ',
        'ｿ', 'ﾁ', 'ﾄ', 'ﾉ', 'ﾌ', 'ﾔ', 'ﾖ', 'ﾙ', 'ﾚ', 'ﾛ', 'ﾝ', '零', '一', '二', '三', '四', '五',
        '六', '七', '八', '九', '十', '百', '千', '万', '円', '日', '本', '語', '中', '国', '人',
        '大', '小', '上', '下', '左', '右', '東', '西', '南', '北',
    ];

    for y in 0..size.height {
        let mut line_spans: Vec<Span> = vec![];

        for col in &state.matrix_columns {
            let head_y = col.y as u16;
            let trail_length = 10u16;

            if col.x >= size.width {
                continue;
            }

            for i in 0..=trail_length {
                let trail_y = head_y.saturating_sub(i);
                if trail_y == y {
                    let fade_factor = if i == 0 {
                        1.0
                    } else {
                        (trail_length - i) as f32 / trail_length as f32
                    };

                    let intensity = (fade_factor * 255.0) as u8;

                    // Use different characters in trail
                    let char_idx = (col.char_idx + i as usize) % CJK_CHARS.len();
                    let c = CJK_CHARS[char_idx];

                    let char_color = if rainbow {
                        let hue =
                            ((col.x as f32 + state.tick as f32 + i as f32 * 10.0) % 360.0) / 360.0;
                        let r = ((hue * 6.0).sin() * 0.5 + 0.5) * intensity as f32;
                        let g = ((hue * 6.0 + 2.0).sin() * 0.5 + 0.5) * intensity as f32;
                        let b = ((hue * 6.0 + 4.0).sin() * 0.5 + 0.5) * intensity as f32;
                        Color::Rgb(r as u8, g as u8, b as u8)
                    } else {
                        match i {
                            0 => Color::Rgb(255, 255, 255),             // White head
                            1 => Color::Rgb(intensity, 255, intensity), // Bright green
                            _ => match color {
                                Color::Rgb(r, g, b) => {
                                    let nr = ((r as f32 * intensity as f32) / 255.0) as u8;
                                    let ng = ((g as f32 * intensity as f32) / 255.0) as u8;
                                    let nb = ((b as f32 * intensity as f32) / 255.0) as u8;
                                    Color::Rgb(nr, ng, nb)
                                }
                                _ => color,
                            },
                        }
                    };

                    let span = Span::styled(c.to_string(), Style::default().fg(char_color));
                    // Store position as part of the span using spaces for alignment
                    let pos = col.x as usize;
                    while line_spans.len() <= pos {
                        line_spans.push(Span::styled(" ", Style::default()));
                    }
                    line_spans[pos] = span;
                }
            }
        }

        if !line_spans.is_empty() {
            let line = Line::from(line_spans);
            let text = Paragraph::new(line);
            let area = Rect::new(0, y, size.width, 1);
            f.render_widget(text, area);
        }
    }
}

fn render_fireworks(f: &mut Frame, state: &AnimationState, size: Rect, _bg: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    for firework in &state.fireworks {
        if !firework.exploded {
            // Draw rocket
            if firework.x >= 0.0
                && firework.x < size.width as f32
                && firework.y >= 0.0
                && firework.y < size.height as f32
            {
                let x = firework.x as u16;
                let y = firework.y as u16;
                if x < size.width && y < size.height {
                    let color = Color::Rgb(firework.color.0, firework.color.1, firework.color.2);
                    let span = Span::styled("▲", Style::default().fg(color));
                    let line = Line::from(vec![span]);
                    let text = Paragraph::new(line);
                    let area = Rect::new(x, y, 1, 1);
                    f.render_widget(text, area);
                }
            }
        } else {
            // Draw particles
            for particle in &firework.particles {
                if particle.x >= 0.0
                    && particle.x < size.width as f32
                    && particle.y >= 0.0
                    && particle.y < size.height as f32
                {
                    let x = particle.x as u16;
                    let y = particle.y as u16;
                    if x < size.width && y < size.height {
                        let fade = particle.life as f32 / particle.max_life as f32;
                        let r = (firework.color.0 as f32 * fade) as u8;
                        let g = (firework.color.1 as f32 * fade) as u8;
                        let b = (firework.color.2 as f32 * fade) as u8;
                        let color = Color::Rgb(r, g, b);

                        let chars = ['•', '∙', '·'];
                        let char_idx = ((1.0 - fade) * 2.0) as usize % chars.len();
                        let span =
                            Span::styled(chars[char_idx].to_string(), Style::default().fg(color));
                        let line = Line::from(vec![span]);
                        let text = Paragraph::new(line);
                        let area = Rect::new(x, y, 1, 1);
                        f.render_widget(text, area);
                    }
                }
            }
        }
    }
}

fn render_neon_grid(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let offset = state.neon_offset;

    // Draw perspective grid lines
    let _center_x = size.width as f32 / 2.0;
    let _center_y = size.height as f32 / 2.0 + 5.0;

    for i in 0..20 {
        let t = i as f32 * 0.05;
        let y_offset = (offset + t * size.height as f32) % size.height as f32;
        let y = size
            .height
            .saturating_sub(y_offset as u16)
            .saturating_sub(1);

        if y < size.height {
            let intensity = ((1.0 - t) * 255.0) as u8;
            let line_color = match color {
                Color::Rgb(r, g, b) => {
                    let nr = ((r as f32) * (intensity as f32 / 255.0)) as u8;
                    let ng = ((g as f32) * (intensity as f32 / 255.0)) as u8;
                    let nb = ((b as f32) * (intensity as f32 / 255.0)) as u8;
                    Color::Rgb(nr, ng, nb)
                }
                _ => color,
            };

            let span = Span::styled(
                "─".repeat(size.width as usize),
                Style::default().fg(line_color),
            );
            let line = Line::from(vec![span]);
            let text = Paragraph::new(line);
            let area = Rect::new(0, y, size.width, 1);
            f.render_widget(text, area);
        }
    }

    // Draw vertical perspective lines
    for i in 0..size.width {
        if i % 4 == 0 {
            let x = i;
            let span = Span::styled("│", Style::default().fg(color));
            let line = Line::from(vec![span]);
            let text = Paragraph::new(line);
            for y in 0..size.height {
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(text.clone(), area);
            }
        }
    }
}

// Simplex noise function for Perlin flow
fn noise(x: f32, y: f32) -> f32 {
    let s = x.sin() + y.cos();
    (s + 2.0) / 4.0 // Normalize to 0-1
}

fn render_perlin_flow(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let offset = state.perlin_offset;
    let scale = 0.05;

    for y in 0..size.height {
        for x in 0..size.width {
            let nx = x as f32 * scale + offset;
            let ny = y as f32 * scale + offset * 0.5;

            let n = noise(nx, ny);
            let n2 = noise(nx * 2.0, ny * 2.0);
            let combined = (n + n2 * 0.5) / 1.5;

            if combined > 0.6 {
                let intensity = ((combined - 0.6) / 0.4 * 255.0) as u8;
                let pixel_color = match color {
                    Color::Rgb(r, g, b) => {
                        let nr = ((r as f32) * intensity as f32 / 255.0) as u8;
                        let ng = ((g as f32) * intensity as f32 / 255.0) as u8;
                        let nb = ((b as f32) * intensity as f32 / 255.0) as u8;
                        Color::Rgb(nr, ng, nb)
                    }
                    _ => color,
                };

                let chars = ['·', '∙', '•', '◦'];
                let char_idx = (combined * chars.len() as f32) as usize % chars.len();
                let span = Span::styled(
                    chars[char_idx].to_string(),
                    Style::default().fg(pixel_color),
                );
                let line = Line::from(vec![span]);
                let text = Paragraph::new(line);
                let area = Rect::new(x, y, 1, 1);
                f.render_widget(text, area);
            }
        }
    }
}

fn render_cube_3d(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    let center_x = size.width as f32 / 2.0;
    let center_y = size.height as f32 / 2.0;
    let scale = (size.width.min(size.height) as f32 / 4.0).min(8.0);

    // Cube vertices
    let vertices: [(f32, f32, f32); 8] = [
        (-1.0, -1.0, -1.0),
        (1.0, -1.0, -1.0),
        (1.0, 1.0, -1.0),
        (-1.0, 1.0, -1.0),
        (-1.0, -1.0, 1.0),
        (1.0, -1.0, 1.0),
        (1.0, 1.0, 1.0),
        (-1.0, 1.0, 1.0),
    ];

    // Edges connecting vertices
    let edges: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // Back face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // Front face
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // Connecting edges
    ];

    let angle_x = state.cube_rotation.angle_x;
    let angle_y = state.cube_rotation.angle_y;

    // Rotation matrices
    let cos_x = angle_x.cos();
    let sin_x = angle_x.sin();
    let cos_y = angle_y.cos();
    let sin_y = angle_y.sin();

    // Transform vertices
    let mut transformed: Vec<(f32, f32)> = Vec::new();
    for (x, y, z) in &vertices {
        // Rotate around X
        let y1 = y * cos_x - z * sin_x;
        let z1 = y * sin_x + z * cos_x;

        // Rotate around Y
        let x2 = x * cos_y + z1 * sin_y;
        let z2 = -x * sin_y + z1 * cos_y;

        // Project to 2D
        let distance = 4.0;
        let factor = distance / (distance + z2);
        let px = center_x + x2 * scale * factor;
        let py = center_y + y1 * scale * factor * 0.5; // 0.5 for aspect ratio correction

        transformed.push((px, py));
    }

    // Draw edges
    for (i, j) in &edges {
        let (x1, y1) = transformed[*i];
        let (x2, y2) = transformed[*j];

        // Simple line drawing
        let dx = (x2 - x1).abs();
        let dy = (y2 - y1).abs();
        let steps = (dx.max(dy) as usize).max(1);

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let px = (x1 + (x2 - x1) * t) as u16;
            let py = (y1 + (y2 - y1) * t) as u16;

            if px < size.width && py < size.height {
                let span = Span::styled("█", Style::default().fg(color));
                let line = Line::from(vec![span]);
                let text = Paragraph::new(line);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(text, area);
            }
        }
    }

    // Draw vertices
    for (px, py) in &transformed {
        let x = *px as u16;
        let y = *py as u16;
        if x < size.width && y < size.height {
            let span = Span::styled("◆", Style::default().fg(Color::White));
            let line = Line::from(vec![span]);
            let text = Paragraph::new(line);
            let area = Rect::new(x, y, 1, 1);
            f.render_widget(text, area);
        }
    }
}

fn render_fractals(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(parse_color("black")));
    f.render_widget(bg_fill, size);

    // Render a simple Mandelbrot-like pattern
    let offset_x = state.fractal_offset.0;
    let offset_y = state.fractal_offset.1;
    let zoom = 2.0;

    let max_iter = 20;

    for py in 0..size.height {
        for px in 0..size.width {
            // Map pixel to complex plane
            let x0 = (px as f32 / size.width as f32 - 0.5) * zoom * 2.0 + offset_x;
            let y0 = (py as f32 / size.height as f32 - 0.5) * zoom + offset_y;

            let mut x = 0.0;
            let mut y = 0.0;
            let mut iter = 0;

            while x * x + y * y <= 4.0 && iter < max_iter {
                let xtemp = x * x - y * y + x0;
                y = 2.0 * x * y + y0;
                x = xtemp;
                iter += 1;
            }

            if iter < max_iter {
                let intensity = (iter as f32 / max_iter as f32 * 255.0) as u8;
                let pixel_color = match color {
                    Color::Rgb(r, g, b) => {
                        let nr = ((r as f32) * intensity as f32 / 255.0) as u8;
                        let ng = ((g as f32) * intensity as f32 / 255.0) as u8;
                        let nb = ((b as f32) * intensity as f32 / 255.0) as u8;
                        Color::Rgb(nr, ng, nb)
                    }
                    _ => color,
                };

                let chars = [' ', '·', ':', '-', '=', '+', '*', '#', '%', '@'];
                let char_idx =
                    ((iter as f32 / max_iter as f32) * (chars.len() - 1) as f32) as usize;
                let c = chars[char_idx.min(chars.len() - 1)];

                let span = Span::styled(c.to_string(), Style::default().fg(pixel_color));
                let line = Line::from(vec![span]);
                let text = Paragraph::new(line);
                let area = Rect::new(px, py, 1, 1);
                f.render_widget(text, area);
            }
        }
    }
}

fn calculate_auto_layout(_f: &mut Frame, app: &App, size: Rect) -> Rect {
    let config = &app.config;

    // Calculate content dimensions
    let max_label_width = app
        .actions
        .iter()
        .map(|action| action.display_text(true).chars().count())
        .max()
        .unwrap_or(0) as u16;

    // Calculate menu dimensions
    let padding = config.layout.padding;
    let border_width = if config.border.enabled { 2 } else { 0 };
    let title_width = config.title.chars().count() as u16;

    // Content width + padding on both sides + borders
    let content_width = max_label_width.max(title_width.saturating_sub(2));
    let menu_width = content_width + (padding * 2) + border_width;

    // Apply max_width limit
    let final_width = if config.layout.max_width > 0 {
        menu_width.min(config.layout.max_width)
    } else {
        menu_width
    };

    // Ensure minimum width
    let final_width = final_width.max(config.layout.min_width);

    // Calculate height based on number of actions + borders + padding
    let action_count = app.actions.len() as u16;
    let menu_height = action_count + (padding * 2) + border_width;
    let final_height = menu_height.max(config.layout.min_height);

    // Center the menu
    let x = (size.width.saturating_sub(final_width)) / 2;
    let y = (size.height.saturating_sub(final_height)) / 2;

    Rect {
        x,
        y,
        width: final_width,
        height: final_height,
    }
}

fn calculate_fixed_layout(_f: &mut Frame, app: &App, size: Rect) -> Rect {
    let config = &app.config;

    // Calculate layout using percentage-based margins
    let vertical_constraints = vec![
        Constraint::Percentage(config.layout.vertical_margin),
        Constraint::Min(config.layout.min_height),
        Constraint::Percentage(config.layout.vertical_margin),
    ];

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vertical_constraints)
        .split(size);

    let horizontal_constraints = vec![
        Constraint::Percentage(config.layout.horizontal_margin),
        Constraint::Min(config.layout.min_width),
        Constraint::Percentage(config.layout.horizontal_margin),
    ];

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(horizontal_constraints)
        .split(vertical_chunks[1]);

    horizontal_chunks[1]
}

fn render_help_text(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;
    let help_config = &config.help_text;

    let help_key_fg = parse_color(&config.colors.help_key_fg);
    let help_fg = parse_color(&config.colors.help_fg);
    let help_key_modifier = parse_modifier(&config.colors.help_key_modifier);

    // Build help text from key config
    let up_keys = config.keys.up.join("/");
    let down_keys = config.keys.down.join("/");
    let select_keys = config.keys.select.join("/");
    let quit_keys = config.keys.quit.join("/");

    let help_spans = vec![
        Span::styled(
            format!("{}/{}", up_keys, down_keys),
            Style::default()
                .fg(help_key_fg)
                .add_modifier(help_key_modifier),
        ),
        Span::styled(" Navigate", Style::default().fg(help_fg)),
        Span::raw(&help_config.separator),
        Span::styled(
            select_keys,
            Style::default()
                .fg(help_key_fg)
                .add_modifier(help_key_modifier),
        ),
        Span::styled(" Select", Style::default().fg(help_fg)),
        Span::raw(&help_config.separator),
        Span::styled(
            quit_keys,
            Style::default()
                .fg(help_key_fg)
                .add_modifier(help_key_modifier),
        ),
        Span::styled(" Quit", Style::default().fg(help_fg)),
    ];

    let help_area = Rect {
        x: 0,
        y: size.height.saturating_sub(1),
        width: size.width,
        height: 1,
    };

    let help_text = Paragraph::new(Line::from(help_spans))
        .alignment(Alignment::Center)
        .style(Style::default().fg(help_fg));

    f.render_widget(help_text, help_area);
}

// ============================================================================
// MAIN
// ============================================================================

#[derive(Parser)]
#[command(name = "rexit")]
#[command(author = "Ninso112")]
#[command(version = "1.1.6")]
#[command(about = "A rice-ready TUI power menu for Linux with multi-WM support", long_about = None)]
struct Cli {
    /// Generate default configuration file
    #[arg(short, long)]
    init: bool,

    /// Specify custom config file path
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Specify theme to use (loads from ~/.config/rexit/themes/<name>.toml)
    #[arg(short, long, value_name = "NAME")]
    theme: Option<String>,

    /// List available themes
    #[arg(long)]
    list_themes: bool,

    /// Validate configuration file and exit
    #[arg(long)]
    check_config: bool,

    /// Use emoji icons instead of Nerd Fonts
    #[arg(long)]
    emoji: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --init flag
    if cli.init {
        return generate_config_file();
    }

    // Handle --list-themes flag
    if cli.list_themes {
        println!("Available themes:");
        let themes = list_available_themes();
        if themes.is_empty() {
            println!("  No themes found in ~/.config/rexit/themes/");
            println!("  Create theme files with .toml extension in that directory.");
        } else {
            for theme in themes {
                println!("  - {}", theme);
            }
        }
        return Ok(());
    }

    // Load configuration
    let mut config = if let Some(config_path) = cli.config {
        load_config_from_path(&config_path)?
    } else {
        load_config()
    };

    // Handle --theme flag
    if let Some(theme_name) = cli.theme {
        if let Some(theme) = load_theme(&theme_name) {
            merge_theme_into_config(&mut config, theme);
        }
    } else if let Some(ref theme_name) = config.theme {
        // Load theme from config file if specified
        if let Some(theme) = load_theme(theme_name) {
            merge_theme_into_config(&mut config, theme);
        }
    }

    // Handle --emoji flag
    if cli.emoji {
        config.use_emoji_icons = Some(true);
    }

    // Handle --check-config flag
    if cli.check_config {
        println!("Configuration is valid!");
        if let Some(ref theme) = config.theme {
            println!("Active theme: {}", theme);
        }
        println!("Layout mode: {}", config.layout_mode);
        println!("Animation: {}", config.animation.animation_type);
        println!(
            "Actions enabled: {}",
            config.actions.values().filter(|a| a.enabled).count()
        );
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to setup terminal")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Run the app
    let mut app = App::new(config);
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .context("Failed to restore terminal")?;
    terminal.show_cursor().context("Failed to show cursor")?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn generate_config_file() -> Result<()> {
    let config_path = get_config_path().context("Could not determine config directory")?;

    let config_dir = config_path.parent().context("Invalid config path")?;

    fs::create_dir_all(config_dir).with_context(|| {
        format!(
            "Failed to create config directory: {}",
            config_dir.display()
        )
    })?;

    let default_config = generate_default_config();

    fs::write(&config_path, default_config)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    println!(
        "Default configuration file created at: {}",
        config_path.display()
    );
    println!("Edit this file to customize rexit's appearance and behavior.");

    Ok(())
}

fn load_config_from_path(path: &PathBuf) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

    Ok(config)
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    // Calculate responsive layout mode based on terminal size
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let size = Rect::new(0, 0, cols, rows);

    // Check if terminal is too small
    if app.config.responsive.enabled {
        if cols < app.config.responsive.min_terminal_width
            || rows < app.config.responsive.min_terminal_height
        {
            return Err(anyhow::anyhow!(
                "Terminal too small: {}x{}. Minimum required: {}x{}",
                cols,
                rows,
                app.config.responsive.min_terminal_width,
                app.config.responsive.min_terminal_height
            ));
        }

        // Auto-switch to compact layout if terminal is narrow
        if app.config.layout_mode == "vertical" && cols < app.config.responsive.compact_threshold {
            app.config.layout_mode = "compact".to_string();
        }

        // Auto-switch to minimal (horizontal) if terminal is very narrow
        if app.config.layout_mode == "compact" && cols < app.config.responsive.minimal_threshold {
            app.config.layout_mode = "horizontal".to_string();
        }

        // Disable border if terminal is small
        if app.config.responsive.hide_border_when_small && (cols < 60 || rows < 15) {
            app.config.border.enabled = false;
        }
    }

    // Initialize animation with actual terminal size
    app.animation_state.init(&app.config, size);

    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.should_quit {
            break;
        }

        // Update grace period countdown
        if matches!(app.state, AppState::GracePeriod { .. }) {
            if app.update_grace_period()? {
                break; // Grace period expired and action executed
            }
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        // Handle different states
                        match &app.state {
                            AppState::Confirming { .. } => {
                                handle_confirmation_input(app, &key)?;
                            }
                            AppState::GracePeriod { .. } => {
                                handle_grace_period_input(app, &key)?;
                            }
                            AppState::AnimationMenu => {
                                handle_animation_menu_input(app, &key)?;
                            }
                            AppState::Selecting => {
                                handle_selecting_input(app, &key)?;
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse_input(app, mouse)?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn handle_animation_menu_input(app: &mut App, key: &crossterm::event::KeyEvent) -> Result<()> {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Up => {
            app.previous_animation();
        }
        KeyCode::Down => {
            app.next_animation();
        }
        KeyCode::Enter => {
            // Get actual terminal size
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            let size = Rect::new(0, 0, cols, rows);
            app.select_animation(size);
        }
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
            app.close_animation_menu();
        }
        _ => {}
    }

    Ok(())
}

fn handle_confirmation_input(app: &mut App, key: &crossterm::event::KeyEvent) -> Result<()> {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            app.confirm_yes()?;
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            app.confirm_no();
        }
        KeyCode::Enter => {
            // Enter defaults to No (cancel)
            app.confirm_no();
        }
        KeyCode::Esc => {
            app.confirm_no();
        }
        _ => {}
    }
    Ok(())
}

fn handle_grace_period_input(app: &mut App, _key: &crossterm::event::KeyEvent) -> Result<()> {
    // Any key press cancels the grace period
    app.cancel_grace_period();
    Ok(())
}

fn handle_selecting_input(app: &mut App, key: &crossterm::event::KeyEvent) -> Result<()> {
    // Check for Konami code sequence first
    if app.easter_egg.check_konami(key.code) {
        // Konami code activated! Rainbow mode toggled
        return Ok(());
    }

    // Check for animation menu hotkey (hidden feature - 'a' key)
    if let KeyCode::Char('a') = key.code {
        app.open_animation_menu();
        return Ok(());
    }

    // Check quit keys
    for key_str in &app.config.keys.quit {
        if app.check_key(key_str, key) {
            app.quit();
            return Ok(());
        }
    }

    // Check up keys (for vertical layout) or left keys (for horizontal layout)
    for key_str in &app.config.keys.up {
        if app.check_key(key_str, key) {
            match app.config.layout_mode.as_str() {
                "horizontal" | "compact" => app.previous_horizontal(),
                "grid" => app.previous_grid(2),
                _ => app.previous(),
            }
            return Ok(());
        }
    }

    // Check down keys (for vertical layout) or right keys (for horizontal layout)
    for key_str in &app.config.keys.down {
        if app.check_key(key_str, key) {
            match app.config.layout_mode.as_str() {
                "horizontal" | "compact" => app.next_horizontal(),
                "grid" => app.next_grid(2),
                _ => app.next(),
            }
            return Ok(());
        }
    }

    // Check select keys
    for key_str in &app.config.keys.select {
        if app.check_key(key_str, key) {
            app.select()?;
            return Ok(());
        }
    }

    // Check action shortcuts
    if let KeyCode::Char(c) = key.code {
        if let Some(index) = app
            .actions
            .iter()
            .position(|a| a.shortcut.to_lowercase() == c.to_lowercase().to_string())
        {
            app.select_at_index(index)?;
            return Ok(());
        }
    }

    Ok(())
}

fn handle_mouse_input(app: &mut App, mouse: MouseEvent) -> Result<()> {
    use crossterm::event::MouseEventKind;

    match app.state {
        AppState::Selecting => {
            match mouse.kind {
                MouseEventKind::Down(_) => {
                    // Check if click is within the menu area
                    let (cols, rows) = terminal::size().unwrap_or((80, 24));
                    let size = Rect::new(0, 0, cols, rows);

                    // Calculate menu area based on layout mode
                    let menu_area = if app.config.layout.auto_scale {
                        calculate_auto_layout_menu_area(app, size)
                    } else {
                        calculate_fixed_layout_menu_area(app, size)
                    };

                    // Check if click is inside menu area
                    if mouse.column >= menu_area.x
                        && mouse.column < menu_area.x + menu_area.width
                        && mouse.row >= menu_area.y
                        && mouse.row < menu_area.y + menu_area.height
                    {
                        // Calculate which item was clicked
                        let relative_y = mouse.row.saturating_sub(menu_area.y);
                        let border_offset = if app.config.border.enabled { 1 } else { 0 };
                        let padding = app.config.layout.padding;
                        let item_index =
                            (relative_y.saturating_sub(border_offset + padding)) as usize;

                        if item_index < app.actions.len() {
                            app.selected_index = item_index;
                            app.select()?;
                        }
                    }
                }
                MouseEventKind::ScrollUp => {
                    app.previous();
                }
                MouseEventKind::ScrollDown => {
                    app.next();
                }
                _ => {}
            }
        }
        AppState::Confirming { action_index: _ } => {
            match mouse.kind {
                MouseEventKind::Down(_) => {
                    // Simple click anywhere cancels confirmation
                    app.confirm_no();
                }
                _ => {}
            }
        }
        AppState::GracePeriod { .. } => {
            match mouse.kind {
                MouseEventKind::Down(_) => {
                    // Any click cancels grace period
                    app.cancel_grace_period();
                }
                _ => {}
            }
        }
        AppState::AnimationMenu => match mouse.kind {
            MouseEventKind::ScrollUp => {
                app.previous_animation();
            }
            MouseEventKind::ScrollDown => {
                app.next_animation();
            }
            _ => {}
        },
    }

    Ok(())
}

// Helper function to calculate menu area for mouse input (auto layout)
fn calculate_auto_layout_menu_area(app: &App, size: Rect) -> Rect {
    let config = &app.config;

    // Calculate content dimensions
    let max_label_width = app
        .actions
        .iter()
        .map(|action| action.display_text(true).chars().count())
        .max()
        .unwrap_or(0) as u16;

    let padding = config.layout.padding;
    let border_width = if config.border.enabled { 2 } else { 0 };
    let title_width = config.title.chars().count() as u16;

    let content_width = max_label_width.max(title_width.saturating_sub(2));
    let menu_width = content_width + (padding * 2) + border_width;

    let final_width = if config.layout.max_width > 0 {
        menu_width.min(config.layout.max_width)
    } else {
        menu_width
    };
    let final_width = final_width.max(config.layout.min_width);

    let action_count = app.actions.len() as u16;
    let menu_height = action_count + (padding * 2) + border_width;
    let final_height = menu_height.max(config.layout.min_height);

    let x = (size.width.saturating_sub(final_width)) / 2;
    let y = (size.height.saturating_sub(final_height)) / 2;

    Rect {
        x,
        y,
        width: final_width,
        height: final_height,
    }
}

// Helper function to calculate menu area for mouse input (fixed layout)
fn calculate_fixed_layout_menu_area(app: &App, size: Rect) -> Rect {
    let config = &app.config;

    let vertical_constraints = vec![
        Constraint::Percentage(config.layout.vertical_margin),
        Constraint::Min(config.layout.min_height),
        Constraint::Percentage(config.layout.vertical_margin),
    ];

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vertical_constraints)
        .split(size);

    let horizontal_constraints = vec![
        Constraint::Percentage(config.layout.horizontal_margin),
        Constraint::Min(config.layout.min_width),
        Constraint::Percentage(config.layout.horizontal_margin),
    ];

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(horizontal_constraints)
        .split(vertical_chunks[1]);

    horizontal_chunks[1]
}
