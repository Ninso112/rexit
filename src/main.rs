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
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
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
    /// Animation type: "matrix", "rain", "thunder", "snow", "stars", "fireflies", "none"
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

[actions.reboot]
icon = "\u{21BB}"
label = "Reboot"
command = "systemctl"
args = ["reboot"]
enabled = true

[actions.suspend]
icon = "\u{23FE}"
label = "Suspend"
command = "systemctl"
args = ["suspend"]
enabled = true

[actions.lock]
icon = "\u{1F512}"
label = "Lock"
command = "hyprlock"
args = []
enabled = true

[actions.logout]
icon = "\u{21E5}"
label = "Logout"
command = "hyprctl"
args = ["dispatch", "exit"]
enabled = true

[actions.cancel]
icon = "\u{2715}"
label = "Cancel"
command = ""
args = []
enabled = true

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
## Animation types: "matrix", "rain", "thunder", "snow", "stars", "fireflies", "none"
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
}

impl Action {
    fn display_text(&self) -> String {
        format!("{} {}", self.icon, self.label)
    }

    fn execute(&self) -> Result<()> {
        if self.command.is_empty() {
            return Ok(());
        }

        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);

        cmd.spawn()
            .with_context(|| format!("Failed to execute command: {}", self.command))?;

        Ok(())
    }
}

// ============================================================================
// APPLICATION STATE
// ============================================================================

struct App {
    actions: Vec<Action>,
    selected_index: usize,
    should_quit: bool,
    config: Config,
    animation_state: AnimationState,
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
    /// Thunder flash state
    thunder_flash: u8,
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

impl App {
    fn new(config: Config) -> Self {
        let actions: Vec<Action> = config
            .actions
            .iter()
            .filter(|(_, action_config)| action_config.enabled)
            .map(|(_id, action_config)| Action {
                icon: action_config.icon.clone(),
                label: action_config.label.clone(),
                command: action_config.command.clone(),
                args: action_config.args.clone(),
            })
            .collect();

        let mut app = Self {
            actions,
            selected_index: 0,
            should_quit: false,
            config,
            animation_state: AnimationState::new(),
        };

        // Initialize animation based on terminal size
        let terminal_size = ratatui::layout::Rect::new(0, 0, 80, 24); // Default, will update on first render
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
            action.execute()?;
        }
        self.should_quit = true;
        Ok(())
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
            _ => {}
        }
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
            thunder_flash: 0,
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
                        twinkle_offset: rng.gen_range(0.0..6.28),
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
            if self.tick % 3 == 0 {
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
        if self.tick % 60 == 0 && rng.gen_bool(0.1) {
            let target_count = ((200 * config.animation.density as usize) / 100).max(5);
            if self.stars.len() < target_count && !self.stars.is_empty() {
                self.stars.push(Star {
                    x: rng.gen_range(0..200),
                    y: rng.gen_range(0..60),
                    brightness: rng.gen_range(50..255),
                    twinkle_speed: rng.gen_range(0.05..0.2),
                    twinkle_offset: rng.gen_range(0.0..6.28),
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

    render_background_animation(f, app, size);

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

    // Create list items
    let items: Vec<ListItem> = app
        .actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let content = action.display_text();
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

    // Render help text
    if render_help {
        render_help_text(f, app, size);
    }
}

fn render_background_animation(f: &mut Frame, app: &App, size: Rect) {
    let config = &app.config;

    if !config.animation.enabled || config.animation.animation_type == "none" {
        return;
    }

    let animation_color = parse_color(&config.animation.color);
    let bg_color = parse_color(&config.colors.background);

    match config.animation.animation_type.as_str() {
        "matrix" => render_matrix(f, &app.animation_state, size, animation_color, bg_color),
        "rain" => render_rain(f, &app.animation_state, size, animation_color, bg_color),
        "thunder" => render_thunder(f, &app.animation_state, size, animation_color, bg_color),
        "snow" => render_snow(f, &app.animation_state, size, animation_color, bg_color),
        "stars" => render_stars(f, &app.animation_state, size, animation_color, bg_color),
        "fireflies" => render_fireflies(f, &app.animation_state, size, animation_color, bg_color),
        _ => {}
    }
}

fn render_matrix(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    let mut spans = vec![];

    for col in &state.matrix_columns {
        let y = col.y as u16;
        if y < size.height {
            let intensity = ((col.y / size.height as f32) * 255.0) as u8;
            let char_color = match color {
                Color::Green => Color::Rgb(0, intensity.max(100), 0),
                Color::Blue => Color::Rgb(0, 0, intensity.max(100)),
                Color::Cyan => Color::Rgb(0, intensity.max(100), intensity.max(100)),
                _ => color,
            };

            let line_idx = y as usize;
            while spans.len() <= line_idx {
                spans.push(vec![]);
            }

            let char_str = MATRIX_CHARS[col.char_idx].to_string();
            spans[line_idx].push(Span::styled(char_str, Style::default().fg(char_color)));
        }
    }

    // Render spans at appropriate positions
    for (y, line_spans) in spans.iter().enumerate() {
        if !line_spans.is_empty() && y < size.height as usize {
            let text = Line::from(line_spans.clone());
            let paragraph = Paragraph::new(text).style(Style::default().bg(parse_color("black")));
            let area = Rect::new(0, y as u16, size.width, 1);
            f.render_widget(paragraph, area);
        }
    }

    // Render trailing trails
    for col in &state.matrix_columns {
        let head_y = col.y as u16;
        let trail_length = 5u16;

        for i in 1..=trail_length {
            let trail_y = head_y.saturating_sub(i);
            if trail_y < size.height {
                let trail_intensity = ((trail_length - i) * 40) as u8;
                let trail_color = match color {
                    Color::Green => Color::Rgb(0, trail_intensity + 20, 0),
                    Color::Blue => Color::Rgb(0, 0, trail_intensity + 20),
                    Color::Cyan => Color::Rgb(0, trail_intensity + 20, trail_intensity + 20),
                    _ => color,
                };

                let span = Span::styled("│", Style::default().fg(trail_color));

                let text = Line::from(vec![span]);
                let paragraph = Paragraph::new(text);
                let area = Rect::new(col.x, trail_y, 1, 1);
                f.render_widget(paragraph, area);
            }
        }
    }
}

fn render_rain(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    for drop in &state.rain_drops {
        let y = drop.y as u16;
        if y < size.height {
            let rain_char = if drop.speed > 1.5 { "│" } else { "┆" };
            let intensity = (100 + (drop.speed * 50.0) as u8).min(255);

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
        let clear = Clear;
        f.render_widget(clear, size);

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

        // Fill background
        let clear = Clear;
        f.render_widget(clear, size);

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
    for flake in &state.snow_flakes {
        let y = flake.y as u16;
        let x = flake.x as u16;
        if y < size.height && x < size.width {
            let snow_char = match flake.size {
                1 => "·",
                2 => "•",
                _ => "*",
            };

            let intensity = (150 + flake.size * 30) as u8;
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

fn render_fireflies(f: &mut Frame, state: &AnimationState, size: Rect, color: Color, _bg: Color) {
    for firefly in &state.fireflies {
        let y = firefly.y as u16;
        let x = firefly.x as u16;
        if y < size.height && x < size.width {
            let intensity = firefly.brightness;
            let firefly_color = match color {
                Color::Yellow => Color::Rgb(intensity, intensity, 0),
                Color::Green => Color::Rgb(0, intensity, 0),
                _ => Color::Rgb(intensity, intensity, intensity / 2),
            };

            let span = Span::styled("●", Style::default().fg(firefly_color));
            let text = Line::from(vec![span]);
            let paragraph = Paragraph::new(text);
            let area = Rect::new(x, y, 1, 1);
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
        .map(|action| action.display_text().chars().count())
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
#[command(version = "0.3.0")]
#[command(about = "A rice-ready TUI power menu for Linux, optimized for Hyprland", long_about = None)]
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
                    // Check quit keys
                    let mut action_taken = false;

                    for key_str in &app.config.keys.quit {
                        if app.check_key(key_str, &key) {
                            app.quit();
                            action_taken = true;
                            break;
                        }
                    }

                    if !action_taken {
                        // Check up keys
                        for key_str in &app.config.keys.up {
                            if app.check_key(key_str, &key) {
                                app.previous();
                                action_taken = true;
                                break;
                            }
                        }
                    }

                    if !action_taken {
                        // Check down keys
                        for key_str in &app.config.keys.down {
                            if app.check_key(key_str, &key) {
                                app.next();
                                action_taken = true;
                                break;
                            }
                        }
                    }

                    if !action_taken {
                        // Check select keys
                        for key_str in &app.config.keys.select {
                            if app.check_key(key_str, &key) {
                                app.select()?;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
