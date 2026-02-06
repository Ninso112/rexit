use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
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

    /// Layout mode: "vertical", "horizontal", "grid", "compact"
    pub layout_mode: String,

    /// Window manager type: "auto", "hyprland", "sway", "i3", "bspwm", "awesome"
    pub wm_type: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationConfig {
    /// Enable background animation
    pub enabled: bool,
    /// Animation type: "matrix", "rain", "thunder", "snow", "stars", "fireflies", "bubbles", "confetti", "wave", "particles", "digital_rain", "heartbeat", "plasma", "scanlines", "aurora", "autumn", "dna", "synthwave", "smoke", "gradient_flow", "constellation", "fish_tank", "typing_code", "vortex", "circuit", "flow_field", "morse", "lissajous", "game_of_life", "none"
    pub animation_type: String,
    /// Animation speed in milliseconds (lower = faster)
    pub speed_ms: u64,
    /// Animation color (for single-color animations)
    pub color: String,
    /// Animation density (0-100, higher = more particles)
    pub density: u8,
}

impl Default for Config {
    fn default() -> Self {
        let mut actions = HashMap::new();

        actions.insert(
            "shutdown".to_string(),
            ActionConfig {
                icon: "\u{23FB}".to_string(),
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
                icon: "\u{21BB}".to_string(),
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
                icon: "\u{23FE}".to_string(),
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
                icon: "\u{1F512}".to_string(),
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
                icon: "\u{21E5}".to_string(),
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
                icon: "\u{2715}".to_string(),
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
            },
            layout_mode: "vertical".to_string(),
            wm_type: "auto".to_string(),
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
icon = "\u{23FB}"
label = "Shutdown"
command = "systemctl"
args = ["poweroff"]
enabled = true
confirm = true      ## Require confirmation before executing
favorite = true     ## Show at top of list
shortcut = "s"      ## Press s to select

[actions.reboot]
icon = "\u{21BB}"
label = "Reboot"
command = "systemctl"
args = ["reboot"]
enabled = true
confirm = true
favorite = true
shortcut = "r"

[actions.suspend]
icon = "\u{23FE}"
label = "Suspend"
command = "systemctl"
args = ["suspend"]
enabled = true
confirm = false
favorite = false
shortcut = "u"

[actions.lock]
icon = "\u{1F512}"
label = "Lock"
command = "hyprlock"
args = []
enabled = true
confirm = false
favorite = false
shortcut = "l"

[actions.logout]
icon = "\u{21E5}"
label = "Logout"
command = "hyprctl"
args = ["dispatch", "exit"]
enabled = true
confirm = true
favorite = false
shortcut = "o"

[actions.cancel]
icon = "\u{2715}"
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

[animation]
## Background animation settings
## Animation types: "matrix", "rain", "thunder", "snow", "stars", "fireflies", "bubbles", "confetti", "wave", "particles", "digital_rain", "heartbeat", "plasma", "scanlines", "aurora", "autumn", "dna", "synthwave", "smoke", "gradient_flow", "constellation", "fish_tank", "typing_code", "vortex", "circuit", "flow_field", "morse", "lissajous", "game_of_life", "none"
enabled = true
animation_type = "matrix"
speed_ms = 80
color = "green"
density = 50
"##,
    )
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
    Confirming { action_index: usize },
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
}

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

struct GameOfLifeCell {
    x: usize,
    y: usize,
    alive: bool,
    next_state: bool,
    age: u8,
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
        let mut actions: Vec<Action> = config
            .actions
            .iter()
            .filter(|(_, action_config)| action_config.enabled)
            .map(|(_id, action_config)| Action {
                icon: action_config.icon.clone(),
                label: action_config.label.clone(),
                command: action_config.command.clone(),
                args: action_config.args.clone(),
                confirm: action_config.confirm,
                favorite: action_config.favorite,
                shortcut: action_config.shortcut.clone(),
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

        let mut app = Self {
            actions,
            selected_index,
            should_quit: false,
            config,
            animation_state: AnimationState::new(),
            state: AppState::Selecting,
            last_executed,
            easter_egg: EasterEggState::new(),
        };

        // Initialize animation based on terminal size
        let terminal_size = ratatui::layout::Rect::new(0, 0, 80, 24);
        app.animation_state.init(&app.config, terminal_size);

        app
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
            if let Some(action) = self.actions.get(action_index) {
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

        let now = std::time::Instant::now();
        let elapsed = now
            .duration_since(self.animation_state.last_update)
            .as_millis() as u64;

        if elapsed < self.config.animation.speed_ms {
            return;
        }

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
        // Type one character every few ticks
        if self.tick.is_multiple_of(3) {
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
}

// Matrix characters for the animation
const MATRIX_CHARS: &[char] = &[
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
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 10, 10)));
    f.render_widget(bg_fill, size);

    // Render scanlines
    for y in 0..size.height {
        let is_scanline = y % 4 == state.scanline_pos % 4;
        let line_color = if is_scanline {
            color
        } else {
            Color::Rgb(20, 20, 20)
        };

        let span = Span::styled("░", Style::default().fg(line_color));
        let text = Line::from(vec![span]);
        let paragraph = Paragraph::new(text);
        let area = Rect::new(0, y, size.width, 1);
        f.render_widget(paragraph, area);
    }

    // Occasional glitch effect
    use rand::Rng;
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.05) {
        let glitch_y = rng.gen_range(0..size.height);
        let glitch_span = Span::styled("█", Style::default().fg(Color::White));
        let glitch_text = Line::from(vec![glitch_span]);
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

    let center_x = size.width / 2;

    for base in &state.dna {
        let y = base.y as u16;
        if y >= size.height {
            continue;
        }

        // Calculate helix offset
        let phase = base.y * 0.3;
        let offset = (phase.sin() * 8.0) as i16;

        let left_x = (center_x as i16 - 5 + offset).max(0) as u16;
        let right_x = (center_x as i16 + 5 + offset).max(0) as u16;

        if left_x < size.width {
            let span = Span::styled(
                base.left_char.to_string(),
                Style::default().fg(Color::Rgb(0, 200, 100)),
            );
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(left_x, y, 1, 1);
            f.render_widget(paragraph, area);
        }

        if right_x < size.width && base.connection {
            // Draw connection
            for cx in left_x..=right_x {
                if cx < size.width {
                    let span = Span::styled("─", Style::default().fg(color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(cx, y, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }

            if right_x < size.width {
                let span = Span::styled(
                    base.right_char.to_string(),
                    Style::default().fg(Color::Rgb(200, 100, 0)),
                );
                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(right_x, y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_synthwave(f: &mut Frame, state: &AnimationState, size: Rect, color: Color) {
    let bg_fill = Block::default().style(Style::default().bg(Color::Rgb(10, 0, 20)));
    f.render_widget(bg_fill, size);

    // Draw sun
    let sun_y = size.height / 3;
    let sun_radius = 6;
    for y in 0..sun_radius {
        let sun_row_y = sun_y + y as u16;
        if sun_row_y < size.height {
            let line_width = (sun_radius - y) * 2;
            let start_x = (size.width / 2).saturating_sub(line_width as u16);
            for x in 0..(line_width as u16 * 2) {
                let px = start_x + x;
                if px < size.width {
                    let span = Span::styled("█", Style::default().fg(Color::Rgb(255, 100, 100)));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(px, sun_row_y, 1, 1);
                    f.render_widget(paragraph, area);
                }
            }
        }
    }

    // Draw grid lines
    let offset = state.synthwave_offset as u16 % 4;
    for y in (sun_y + sun_radius as u16..size.height).step_by(4) {
        let grid_y = y + offset;
        if grid_y < size.height {
            let span = Span::styled("─", Style::default().fg(color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(0, grid_y, size.width, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Vertical perspective lines
    let center_x = size.width / 2;
    for i in 0..10 {
        let x = if i < 5 {
            center_x.saturating_sub((5 - i) as u16 * 4)
        } else {
            center_x + ((i - 5) as u16 * 4)
        };
        if x < size.width {
            for y in sun_y + sun_radius as u16..size.height {
                if (y + offset).is_multiple_of(4) {
                    let span = Span::styled("│", Style::default().fg(color));
                    let text = Line::from(vec![span]);
                    let paragraph = Paragraph::new(text);
                    let area = Rect::new(x, y, 1, 1);
                    f.render_widget(paragraph, area);
                }
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

        if y < size.height && x < size.width {
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
#[command(version = "1.0.0")]
#[command(about = "A rice-ready TUI power menu for Linux with multi-WM support", long_about = None)]
struct Cli {
    /// Generate default configuration file
    #[arg(short, long)]
    init: bool,

    /// Specify custom config file path
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle --init flag
    if cli.init {
        return generate_config_file();
    }

    // Load configuration
    let config = if let Some(config_path) = cli.config {
        load_config_from_path(&config_path)?
    } else {
        load_config()
    };

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
    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Handle different states
                    match &app.state {
                        AppState::Confirming { .. } => {
                            handle_confirmation_input(app, &key)?;
                        }
                        AppState::Selecting => {
                            handle_selecting_input(app, &key)?;
                        }
                    }
                }
            }
        }
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

fn handle_selecting_input(app: &mut App, key: &crossterm::event::KeyEvent) -> Result<()> {
    // Check for Konami code sequence first
    if app.easter_egg.check_konami(key.code) {
        // Konami code activated! Rainbow mode toggled
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
