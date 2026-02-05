use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::process::Command;

#[derive(Parser)]
#[command(name = "rexit")]
#[command(author = "Ninso112")]
#[command(version = "0.1.0")]
#[command(about = "A minimalist TUI power menu for Linux, optimized for Hyprland", long_about = None)]
struct Cli {}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PowerAction {
    Shutdown,
    Reboot,
    Suspend,
    Lock,
    Logout,
    Cancel,
}

impl PowerAction {
    fn as_str(&self) -> &str {
        match self {
            PowerAction::Shutdown => "Shutdown",
            PowerAction::Reboot => "Reboot",
            PowerAction::Suspend => "Suspend",
            PowerAction::Lock => "Lock",
            PowerAction::Logout => "Logout",
            PowerAction::Cancel => "Cancel",
        }
    }

    fn icon(&self) -> &str {
        match self {
            PowerAction::Shutdown => "⏻",
            PowerAction::Reboot => "↻",
            PowerAction::Suspend => "⏾",
            PowerAction::Lock => "🔒",
            PowerAction::Logout => "⇥",
            PowerAction::Cancel => "✕",
        }
    }

    fn execute(&self) -> Result<()> {
        match self {
            PowerAction::Shutdown => {
                Command::new("systemctl")
                    .arg("poweroff")
                    .spawn()
                    .context("Failed to execute shutdown command")?;
            }
            PowerAction::Reboot => {
                Command::new("systemctl")
                    .arg("reboot")
                    .spawn()
                    .context("Failed to execute reboot command")?;
            }
            PowerAction::Suspend => {
                Command::new("systemctl")
                    .arg("suspend")
                    .spawn()
                    .context("Failed to execute suspend command")?;
            }
            PowerAction::Lock => {
                Command::new("hyprlock")
                    .spawn()
                    .context("Failed to execute lock command")?;
            }
            PowerAction::Logout => {
                Command::new("hyprctl")
                    .args(["dispatch", "exit"])
                    .spawn()
                    .context("Failed to execute logout command")?;
            }
            PowerAction::Cancel => {
                // Do nothing, just exit
            }
        }
        Ok(())
    }

    fn all() -> Vec<PowerAction> {
        vec![
            PowerAction::Shutdown,
            PowerAction::Reboot,
            PowerAction::Suspend,
            PowerAction::Lock,
            PowerAction::Logout,
            PowerAction::Cancel,
        ]
    }
}

struct App {
    actions: Vec<PowerAction>,
    selected_index: usize,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            actions: PowerAction::all(),
            selected_index: 0,
            should_quit: false,
        }
    }

    fn next(&mut self) {
        self.selected_index = (self.selected_index + 1) % self.actions.len();
    }

    fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.actions.len() - 1;
        }
    }

    fn select(&mut self) -> Result<()> {
        let action = self.actions[self.selected_index];
        action.execute()?;
        self.should_quit = true;
        Ok(())
    }

    fn quit(&mut self) {
        self.should_quit = true;
    }
}

fn main() -> Result<()> {
    let _cli = Cli::parse();

    // Setup terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("Failed to setup terminal")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Run the app
    let mut app = App::new();
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

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.quit();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.next();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.previous();
                        }
                        KeyCode::Enter => {
                            app.select()?;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create centered layout
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(10),
            Constraint::Percentage(30),
        ])
        .split(size);

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Min(30),
            Constraint::Percentage(30),
        ])
        .split(vertical_chunks[1]);

    let center_area = horizontal_chunks[1];

    // Create the list items
    let items: Vec<ListItem> = app
        .actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let content = format!("{} {}", action.icon(), action.as_str());
            let style = if i == app.selected_index {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(content, style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" rexit ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(list, center_area);

    // Render help text at the bottom
    let help_area = Rect {
        x: 0,
        y: size.height.saturating_sub(1),
        width: size.width,
        height: 1,
    };

    let help_text = Paragraph::new(Line::from(vec![
        Span::styled(
            "↑↓/jk",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Navigate | "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Select | "),
        Span::styled(
            "q/Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Quit"),
    ]))
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::Gray));

    f.render_widget(help_text, help_area);
}
