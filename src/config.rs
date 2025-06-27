use crate::ui::centered_rect;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Stdout},
    path::PathBuf,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub server_url: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub refresh_interval: u64,
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir() // Use the OS agnostic config dir on all systems
            .unwrap_or_else(|| PathBuf::from("."))
            .join("dstui")
            .join("config.toml")
    }

    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml::to_string_pretty(self).unwrap())
    }
}

// Run a small Ratatui-based form to collect URL, port, user, pass.
pub fn run_config_wizard(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> io::Result<AppConfig> {
    use crossterm::event::{self, Event, KeyCode};

    let labels = [
        " Server URL ",
        " Port ",
        " Username ",
        " Password ",
        " Refresh Interval (in seconds)",
    ];
    let mut inputs = [
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    let mut active = 0;
    let mut error_msg: Option<String> = None;

    terminal.show_cursor()?;

    loop {
        terminal.draw(|f| {
            // Centered dialog area
            let size = f.area();
            let area = centered_rect(60, 70, size);

            // Outer dialog box
            let dialog = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    " DSTUI Setup ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .style(Style::default().bg(Color::Black).fg(Color::White));
            f.render_widget(dialog, area);

            // Split into 6 rows: 5 fields + footer
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints(
                    [
                        Constraint::Length(3), // Server URL input field
                        Constraint::Length(3), // Port input field
                        Constraint::Length(3), // Username input field
                        Constraint::Length(3), // Password input field
                        Constraint::Length(3), // Refresh interval field
                        Constraint::Length(3), // Instructions
                    ]
                    .as_ref(),
                )
                .split(area);

            // Render each input box
            for (i, label) in labels.iter().enumerate() {
                let display_value = if i == 3 {
                    "*".repeat(inputs[i].len())
                } else {
                    inputs[i].clone()
                };
                let is_active = i == active;

                // Titled, bordered field
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(if is_active {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    })
                    .title(Span::styled(
                        *label,
                        if is_active {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        },
                    ));

                let paragraph = Paragraph::new(display_value)
                    .style(if is_active {
                        Style::default().fg(Color::White)
                    } else {
                        Style::default().fg(Color::Gray)
                    })
                    .block(block)
                    .alignment(ratatui::layout::Alignment::Left)
                    .wrap(ratatui::widgets::Wrap { trim: false });

                f.render_widget(paragraph, rows[i]);
                let input_rect = rows[active];
                let x = input_rect.x + 1 + inputs[active].len() as u16;
                let y = input_rect.y + 1;
                f.set_cursor_position((x, y));
            }

            // Footer: either error or help
            let footer = if let Some(err) = &error_msg {
                Paragraph::new(Line::from(Span::styled(
                    err.clone(),
                    Style::default().fg(Color::White).bg(Color::Red),
                )))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                )
                .alignment(ratatui::layout::Alignment::Center)
            } else {
                Paragraph::new(Line::from(vec![
                    Span::raw("↑/↓: move  "),
                    Span::raw("type: edit  "),
                    Span::raw("Enter: next field  "),
                    Span::raw("Enter(last): OK"),
                ]))
                .style(Style::default().fg(Color::Gray))
                .alignment(ratatui::layout::Alignment::Center)
            };
            f.render_widget(footer, rows[5]);
        })?;

        // Handle keys
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char(c) => {
                    // Only digits allowed in port *or* refresh fields
                    if (active == 1 || active == 4) && c.is_ascii_digit() {
                        inputs[active].push(c);
                    } else if active != 1 && active != 4 {
                        // free‐form text fields
                        inputs[active].push(c);
                    }
                    error_msg = None;
                }
                KeyCode::Backspace => {
                    inputs[active].pop();
                    error_msg = None;
                }
                KeyCode::Up => {
                    active = active.saturating_sub(1);
                    error_msg = None;
                }
                KeyCode::Down => {
                    if active < labels.len() - 1 {
                        active += 1;
                    }
                    error_msg = None;
                }
                KeyCode::Enter => {
                    if active < labels.len() - 1 {
                        active += 1;
                        error_msg = None;
                        continue;
                    } else {
                        // Validate port
                        if inputs[1].parse::<u16>().is_err() {
                            error_msg = Some("Port must be a number 0–65535".into());
                            continue;
                        }

                        // Validate refresh_interval
                        if inputs[4].parse::<u64>().is_err() {
                            error_msg = Some("Refresh interval must be a number".into());
                            continue;
                        }
                    }
                    break;
                }
                KeyCode::Esc => {
                    // Abort
                    ratatui::init().show_cursor()?; // Must use, otherwise cursor will not be shown
                    // after pressing Esc
                    ratatui::restore();
                    // Graceful Exit with error code 0, this is an expected exit, no need to raise
                    // an error
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    }

    // All is well, restore terminal
    ratatui::restore();

    // Build final config
    Ok(AppConfig {
        server_url: inputs[0].clone().trim_end_matches('/').to_string(), // Remove trailing '/' if
        // there is one
        port: inputs[1].parse().unwrap_or(0),
        username: inputs[2].clone(),
        password: inputs[3].clone(),
        refresh_interval: inputs[4].clone().parse().unwrap_or(60),
    })
}
