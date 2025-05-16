use crate::api::{ConfigData, DownloadTask, SynologyClient};
use crate::config;
use crate::config::AppConfig;
use crate::ui::centered_rect;
use crate::util::{calculate_elapsed_time, format_bytes, format_seconds, format_timestamp};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, HighlightSpacing, Paragraph, Row, Table, TableState, Tabs, Widget, Wrap},
};
use std::io;
use tokio::time::{Duration, interval};

#[derive(Debug, Default)]
pub struct App {
    should_exit: bool,
    show_help: bool,
    show_server_info: bool,
    show_add_task: bool,
    loading: bool,
    table_state: TableState,
    items: Vec<DownloadTask>,
    dsconfig: Option<ConfigData>,
    error_popup: Option<String>,
    selected_tab: usize,
    tabs: Vec<&'static str>,
}

impl App {
    pub fn new() -> Self {
        // App defaults
        Self {
            should_exit: false,
            show_help: false,
            show_server_info: false,
            show_add_task: false,
            loading: false,
            table_state: TableState::default(),
            items: vec![],
            dsconfig: None,
            error_popup: None,
            selected_tab: 0,
            tabs: vec!["General", "Transfer", "Tracker", "Peers", "File"],
        }
    }

    pub fn load_config(&self) -> config::AppConfig {
        let config = AppConfig::load();
        // Want to panic if config file is not readable, since we already read or created it during
        // startup
        config.expect("❌ Config file missing or unavailable, aborting.")
    }

    // Loads tasks from DownloadStation
    pub async fn load_tasks(&mut self, client: &SynologyClient) {
        // Storing last active task id for restoring after the refresh
        let prev_id = self
            .table_state
            .selected()
            .and_then(|i| self.items.get(i).map(|t| t.id.clone()));

        match client.list_download_tasks().await {
            Ok(tasks) => self.items = tasks,
            Err(e) => {
                self.error_popup = Some(format!("Failed to fetch tasks: {}\n", e));
                self.items.clear();
            }
        }

        if !self.items.is_empty() {
            let new_selection = prev_id
                // If we had an ID, try to find it in the refreshed list
                .and_then(|id| self.items.iter().position(|t| t.id == id))
                // Otherwise (or if not found), default to the top
                .or(Some(0));
            self.table_state.select(new_selection);
        } else {
            // No tasks at all -> clear selection
            self.table_state.select(None);
        }
    }

    // Main app logic
    pub async fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        client: &mut SynologyClient,
        refresh_interval: u64,
    ) -> io::Result<()> {
        let mut refresher = interval(Duration::from_secs(refresh_interval));

        let mut events = EventStream::new();

        // Initial load & draw
        self.loading = true;
        terminal.draw(|f| self.draw(f, self.show_add_task))?;
        self.load_tasks(client).await;
        self.loading = false;

        self.dsconfig = match client.get_config().await {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                self.error_popup = Some(format!("Failed to fetch server config:\n{}", e));
                None
            }
        };

        terminal.draw(|f| self.draw(f, self.show_add_task))?;

        // Main app loop for actions that require data refresh
        loop {
            tokio::select! {
                // Auto-refresh arm
                _ = refresher.tick() => {
                    self.loading = true;
                    terminal.draw(|f| self.draw(f, self.show_add_task))?;
                    self.load_tasks(client).await;
                    self.loading = false;
                    // redraw right away
                    terminal.draw(|f| self.draw(f, self.show_add_task))?;
                },

                // User input arm
                maybe_event = events.next() => {
                    if let Some(Ok(Event::Key(key))) = maybe_event {
                        // If help, server-info or error_popup panel is up, only allow <q> to close it
                        if self.show_help || self.show_server_info || self.show_add_task || self.error_popup.is_some() {
                            if let KeyCode::Char('q') = key.code {
                                // close whichever is open
                                self.show_help = false;
                                self.show_server_info = false;
                                self.error_popup = None;
                            }
                            if let KeyCode::Esc = key.code {
                                self.show_add_task = false;
                            }
                            // Redraw (so the overlay goes away) but do nothing else
                            terminal.draw(|f| self.draw(f, self.show_add_task))?;
                            continue;
                        }


                        // Otherwise handle normal keys:
                        match key.code {
                            KeyCode::Char('r') => {
                                // Manual refresh
                                self.loading = true;
                                terminal.draw(|f| self.draw(f, self.show_add_task))?;
                                self.load_tasks(client).await;
                                self.loading = false;
                            }
                            // Show add URL popup
                            KeyCode::Char('a') => {
                                terminal.draw(|f| self.draw(f, self.show_add_task))?;
                                self.show_add_task = true;
                                self.handle_add_task_key_event(key);
                            }
                            // Pause Task
                            KeyCode::Char('p') => {
                                if let Some(idx) = self.table_state.selected() {
                                    let id = &self.items[idx].id;
                                    self.loading = true;
                                    terminal.draw(|f| self.draw(f, self.show_add_task))?;

                                    // Decide if currently paused -> resume, else pause
                                    let is_paused = self.items[idx].status.label() == "paused";
                                    let result = if is_paused {
                                        client.resume_task(id).await
                                    } else {
                                        client.pause_task(id).await
                                    };

                                    if let Err(e) = result {
                                        // Stash error for the popup
                                        self.error_popup = Some(format!(
                                            "{} failed for {}:\n{}",
                                            if is_paused { "Resume" } else { "Pause" },
                                            id,
                                            e
                                        ));
                                    }

                                    // Refresh list after the action
                                    self.load_tasks(client).await;
                                    self.loading = false;
                                    terminal.draw(|f| self.draw(f, self.show_add_task))?;
                                }
                            }

                            // Delete task
                            KeyCode::Char('d') => {
                                if let Some(idx) = self.table_state.selected() {
                                    let id = &self.items[idx].id;
                                    self.loading = true;
                                    terminal.draw(|f| self.draw(f, self.show_add_task))?;

                                    let result = client.delete_task(id).await;

                                    if let Err(e) = result {
                                        // Stash error for the popup
                                        self.error_popup = Some(format!(
                                            "Delete failed for {}:\n{}",
                                            id,
                                            e
                                        ));
                                    }

                                    // Refresh list after the action
                                    self.load_tasks(client).await;
                                    self.loading = false;
                                    terminal.draw(|f| self.draw(f, self.show_add_task))?;
                                }

                            }

                            KeyCode::Char('i') => {
                                // Fetch the config
                                match client.get_config().await {
                                    Ok(cfg) => {
                                        self.dsconfig = Some(cfg);
                                        self.show_server_info = true;
                                    }
                                    Err(e) => {
                                        self.error_popup = Some(format!("Fetch config failed:\n{}", e));
                                    }
                                }
                            }
                            // Pass all other keypresses to handle_key_event
                            _ => self.handle_key_event(key),
                        }
                        // Always redraw after handling a key
                        terminal.draw(|f| self.draw(f, self.show_add_task))?;
                    }
                },
            }

            // If <q> is pressed break loop and exit app
            if self.should_exit {
                break;
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame, show_cursor: bool) {
        if show_cursor {
            let raw_w = frame.area().width.saturating_mul(60) / 100;
            let w = raw_w.max(3).min(frame.area().width);
            let x = frame.area().x + (frame.area().width.saturating_sub(w)) / 2;

            let raw_h = frame.area().height.saturating_mul(5) / 100;
            let h = raw_h.max(3).min(frame.area().height);
            let y = frame.area().y + (frame.area().height.saturating_sub(h)) / 2;

            frame.set_cursor_position((x + 1, y + 1));
        }
        frame.render_widget(self, frame.area());
    }

    // Handle every other keypresses, these are not interfering with the data refresh
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Exit
        match key_event.code {
            KeyCode::Char('q') => {
                if self.show_help {
                    self.show_help = false;
                } else if self.show_server_info {
                    self.show_server_info = false;
                } else if self.error_popup.is_some() {
                    self.error_popup = None;
                } else {
                    self.exit();
                }
            }
            // Move down in the task list
            KeyCode::Char('j') => self.next(),
            // Move up in the task list
            KeyCode::Char('k') => self.previous(),
            // Move left in the tabs of the selected task
            KeyCode::Char('h') => {
                if self.selected_tab == 0 {
                    self.selected_tab = self.tabs.len() - 1;
                } else {
                    self.selected_tab -= 1;
                }
            }
            // Move right in the tabs of the selected task
            KeyCode::Char('l') => {
                self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
            }
            // Show help screen
            KeyCode::Char('?') => self.show_help = true,
            // Drop every other keypresses
            _ => {}
        }
    }

    // Handle every other keypresses, these are not interfering with the data refresh
    fn handle_add_task_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter => {
                self.show_add_task = false;
            }
            KeyCode::Up => {
                println!("up!");
            }
            _ => {}
        }
    }

    // Next task
    pub fn next(&mut self) {
        let item_count = self.items.len();
        let i = match self.table_state.selected() {
            Some(i) => (i + 1) % item_count,
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    // Previous task
    pub fn previous(&mut self) {
        let item_count = self.items.len();
        let i = match self.table_state.selected() {
            Some(i) => (i + item_count - 1) % item_count,
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn exit(&mut self) {
        self.should_exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Setting up the Layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let table_area = chunks[0];
        let table_height = table_area.height.saturating_sub(3); // 3 lines for header+block borders
        let total_items = self.items.len();
        let selected_idx = self.table_state.selected().unwrap_or(0);

        // Calculate the start and end indexes for the visible window
        let start = if selected_idx >= table_height as usize {
            selected_idx + 1 - table_height as usize
        } else {
            0
        };
        let end = (start + table_height as usize).min(total_items);

        // Get only the visible items
        let visible_items = &self.items[start..end];

        // Table for the download tasks...
        // ...when loading
        let title_text = if self.loading {
            " DownloadStation TUI Client - [Loading...] ".bold().blue()
        // ...when not loading
        } else {
            " DownloadStation TUI Client ".bold().blue()
        };
        let title = Line::from(title_text);
        // Instructions
        let instructions = Line::from(
            " Move down: <j> | Move up: <k> | Help: <?> | Quit: <q> "
                .bold()
                .yellow(),
        );

        // Upper block
        let block = Block::bordered()
            .title(title)
            .title_bottom(instructions.centered())
            .border_set(border::ROUNDED);

        // Table selection
        let selected = self.table_state.selected();
        let rows = visible_items.iter().enumerate().map(|(i, item)| {
            let actual_idx = start + i;
            // Turn the DownloadTask into a Vec<String> for each column:
            let cells = item.to_row_cells();
            let mut row = Row::new(cells);
            if Some(actual_idx) == selected {
                row = row.style(
                    Style::default()
                        .bg(Color::LightBlue)
                        .fg(Color::DarkGray)
                        .bold(),
                );
            }
            row
        });

        // Column spacing for the task table
        let widths = [
            Constraint::Percentage(25), // Name
            Constraint::Percentage(10), // Size
            Constraint::Percentage(10), // Downloaded
            Constraint::Percentage(10), // Uploaded
            Constraint::Percentage(10), // Progress
            Constraint::Percentage(10), // Upload Speed
            Constraint::Percentage(10), // Download Speed
            Constraint::Percentage(5),  // Ratio
            Constraint::Percentage(10), // Status
        ];

        // Table header
        let header = Row::new(vec![
            "Name",
            "Size",
            "Downloaded",
            "Uploaded",
            "Progress",
            "Upload Speed",
            "Download Speed",
            "Ratio",
            "Status",
        ])
        .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold());

        // The table itself
        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .widths(widths)
            .highlight_spacing(HighlightSpacing::Always)
            .row_highlight_style(Style::new().reversed())
            .column_spacing(1);

        // Render the table into the top chunk
        table.render(chunks[0], buf);

        // Create scrollbar
        let scrollbar_x = table_area.right() - 1;
        let scrollbar_top = table_area.top() + 1;
        let scrollbar_bottom = table_area.bottom() - 1;
        let scrollbar_height = scrollbar_bottom - scrollbar_top;
        let total_items = self.items.len();

        // Only draw scrollbar if content is taller than viewport
        if total_items > table_height as usize && scrollbar_height > 0 {
            let thumb_height = (table_height as f64 / total_items as f64 * scrollbar_height as f64)
                .round()
                .max(1.0) as u16;

            let max_offset = total_items.saturating_sub(table_height as usize) as f64;
            let offset = start as f64;

            let thumb_offset = if max_offset == 0.0 {
                0
            } else {
                ((offset / max_offset) * (scrollbar_height - thumb_height) as f64)
                    .round()
                    .min((scrollbar_height - thumb_height) as f64) as u16
            };

            // Draw track
            for y in scrollbar_top..scrollbar_bottom {
                buf[(scrollbar_x, y)].set_char('│').set_fg(Color::DarkGray);
            }

            // Draw thumb
            for y in scrollbar_top + thumb_offset..scrollbar_top + thumb_offset + thumb_height {
                if y < scrollbar_bottom {
                    buf[(scrollbar_x, y)].set_char('█').set_fg(Color::Yellow);
                }
            }
        }

        // Render the info part in the botton chunk
        let info_block = Block::bordered()
            .title(" Info ".bold().blue())
            .border_set(border::ROUNDED);
        info_block.render(chunks[1], buf);

        let task = &self.items[selected_idx];

        // Get all the details we need from each task and make them visible
        let content = match self.selected_tab {
            0 => {
                if let Some(add) = &task.additional {
                    let t = add.detail.as_ref();
                    Text::from(vec![
                        Line::from(""),
                        Line::from(format!("ID                 : {}", task.id)),
                        Line::from(format!("Username           : {}", task.username)),
                        Line::from(format!(
                            "URL                : {}",
                            t.and_then(|t| t.uri.as_deref()).unwrap_or("-")
                        )),
                        Line::from(format!("Title              : {}", task.title)),
                        Line::from(format!(
                            "Destination        : {}",
                            t.and_then(|t| t.destination.clone())
                                .unwrap_or("-".to_string())
                        )),
                        Line::from(format!("Size               : {}", format_bytes(task.size))),
                        Line::from(format!("Task type          : {}", task.task_type)),
                        Line::from(format!(
                            "Created time       : {}",
                            format_timestamp(t.and_then(|t| t.create_time).unwrap_or(0)),
                        )),
                        Line::from(format!(
                            "Started time       : {}",
                            format_timestamp(t.and_then(|t| t.started_time).unwrap_or(0)),
                        )),
                        Line::from(format!(
                            "Completed time     : {}",
                            format_timestamp(t.and_then(|t| t.completed_time).unwrap_or(0)),
                        )),
                        Line::from(format!(
                            "Elapsed time       : {}",
                            calculate_elapsed_time(
                                t.and_then(|t| t.create_time).unwrap_or(0),
                                t.and_then(|t| t.completed_time).unwrap_or(0)
                            )
                        )),
                        Line::from(format!(
                            "Estimated wait time: {}",
                            format_seconds(t.and_then(|t| t.waiting_seconds).unwrap_or(0)),
                        )),
                        Line::from(format!(
                            "Total pieces       : {}",
                            t.and_then(|t| t.total_pieces).unwrap_or(0)
                        )),
                    ])
                } else {
                    Text::from(vec![Line::from(""), Line::from("No info")])
                }
            }
            1 => {
                // Transfer
                if let Some(add) = &task.additional {
                    let ratio_string;
                    if let Some(ratio) = DownloadTask::upload_download_ratio(task) {
                        ratio_string = format!("{:.2}", ratio);
                    } else {
                        ratio_string = "-".into();
                    }
                    let t = add.transfer.as_ref();
                    Text::from(vec![
                        Line::from(""),
                        Line::from(format!("Status             : {}", task.status.label())),
                        Line::from(format!(
                            "Transferred (UL/DL): {} / {} (Ratio: {})",
                            format_bytes(t.and_then(|t| t.size_uploaded).unwrap_or(0)),
                            format_bytes(t.and_then(|t| t.size_downloaded).unwrap_or(0)),
                            ratio_string
                        )),
                        Line::from(format!(
                            "DL Speed           : {}",
                            format_bytes(t.and_then(|t| t.speed_download).unwrap_or(0))
                        )),
                        Line::from(format!(
                            "UL Speed           : {}",
                            format_bytes(t.and_then(|t| t.speed_upload).unwrap_or(0))
                        )),
                    ])
                } else {
                    Text::from(vec![Line::from(""), Line::from("No transfer info")])
                }
            }
            2 => {
                // Tracker
                if let Some(add) = &task.additional {
                    if let Some(trackers) = &add.tracker {
                        // Build a Vec<Line> starting with an empty line, then each tracker
                        let lines = std::iter::once(Line::from(""))
                            .chain(trackers.iter().map(|tr| {
                                Line::from(format!(
                                    "{} ({}) | seeds: {}, peers: {}",
                                    tr.url.clone().unwrap_or_default(),
                                    tr.status.clone().unwrap_or_default(),
                                    tr.seeds.unwrap_or(0),
                                    tr.peers.unwrap_or(0),
                                ))
                            }))
                            .collect::<Vec<Line>>();

                        Text::from(lines)
                    } else {
                        Text::from(vec![Line::from(""), Line::from("No tracker info")])
                    }
                } else {
                    Text::from(vec![Line::from(""), Line::from("No additional info")])
                }
            }
            3 => {
                // Peers
                if let Some(add) = &task.additional {
                    if let Some(peers) = &add.peer {
                        // Build a Vec<Line> starting with an empty line, then each peer
                        let lines = std::iter::once(Line::from(""))
                            .chain(peers.iter().map(|p| {
                                Line::from(format!(
                                    "{} | {} | Progress: {:.1}% | dl: {} | ul: {}",
                                    p.address.clone().unwrap_or_default(),
                                    p.agent.clone().unwrap_or_default(),
                                    p.progress.unwrap_or(0.0) * 100.0,
                                    format_bytes(p.speed_download.unwrap_or(0)),
                                    format_bytes(p.speed_upload.unwrap_or(0)),
                                ))
                            }))
                            .collect::<Vec<Line>>();

                        Text::from(lines)
                    } else {
                        Text::from(vec![Line::from(""), Line::from("No peer info")])
                    }
                } else {
                    Text::from(vec![Line::from(""), Line::from("No additional info")])
                }
            }
            4 => {
                // File
                if let Some(add) = &task.additional {
                    if let Some(files) = &add.file {
                        // you could also render a mini table here
                        let lines = std::iter::once(Line::from(""))
                            .chain(files.iter().map(|f| {
                                Line::from(format!(
                                    "{} {} / {}",
                                    f.filename.clone().unwrap_or_default(),
                                    format_bytes(f.size.unwrap_or(0)),
                                    format_bytes(f.size_downloaded.unwrap_or(0))
                                ))
                            }))
                            .collect::<Vec<Line>>();

                        Text::from(lines)
                    } else {
                        Text::from(vec![Line::from(""), Line::from("No file info")])
                    }
                } else {
                    Text::from(vec![Line::from(""), Line::from("No additional info")])
                }
            }
            _ => Text::from(Line::from("")),
        };

        // Outer border
        let info_block = Block::bordered()
            .title(" Info ".bold().blue())
            .border_set(border::ROUNDED);
        info_block.render(chunks[1], buf);

        // Carve out inner area (leave border margin)
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(chunks[1]);

        // Tabs go in inner[0]
        let titles = self.tabs.iter().map(|t| Line::from(*t)).collect::<Vec<_>>();
        let tabs = Tabs::new(titles)
            .select(self.selected_tab)
            .highlight_style(Style::default().fg(Color::Yellow))
            .divider("│");
        tabs.render(inner[0], buf);

        // Paragraph goes in inner[1]
        let paragraph = Paragraph::new(content).wrap(Wrap { trim: true });
        paragraph.render(inner[1], buf);

        // Help popup
        if self.show_help {
            let popup_area = centered_rect(60, 60, area);
            let help_text = Text::from(vec![
                Line::from(" Shortcuts:"),
                Line::from(""),
                Line::from("  j - Move down"),
                Line::from("  k - Move up"),
                Line::from("  h - Show previous tab"),
                Line::from("  l - Show next tab"),
                Line::from("  r - Manually refresh tasks (default refresh is 60 seconds)"),
                Line::from("  p - Pause/resume selected task"),
                Line::from("  d - Delete selected task"),
                Line::from("  i - Show server info"),
                Line::from("  q - Quit (or close this help)"),
                Line::from("  ? - Show this help"),
            ]);

            let help_block = Block::bordered()
                .title(Line::from(" Help ".bold()))
                .border_set(border::THICK)
                .style(Style::default().bg(Color::Black).fg(Color::White))
                .title_bottom(
                    Line::from(" Close this panel with <q> ").alignment(Alignment::Center),
                );

            let help = Paragraph::new(help_text)
                .block(help_block)
                .wrap(ratatui::widgets::Wrap { trim: false });

            // Clear the popup area first
            for y in popup_area.top()..popup_area.bottom() {
                for x in popup_area.left()..popup_area.right() {
                    buf[(x, y)].set_char(' ').set_bg(Color::Black);
                }
            }

            // Then render the help paragraph
            help.render(popup_area, buf);
        }

        // Server info popup
        if self.show_server_info {
            let popup_area = centered_rect(60, 60, area);
            let server_config = self.load_config();
            let server_config_path = AppConfig::config_path()
                .into_os_string()
                .into_string()
                .unwrap();

            if let Some(cfg) = &self.dsconfig {
                let default_dest = cfg.default_destination.as_deref().unwrap_or("<none>");
                let emule_dest = cfg.emule_default_destination.as_deref().unwrap_or("<none>");
                let txt = Text::from(vec![
                    Line::from(format!(
                        " BT Max Download     : {} KB/s",
                        cfg.bt_max_download
                    )),
                    Line::from(format!(" BT Max Upload       : {} KB/s", cfg.bt_max_upload)),
                    Line::from(format!(
                        " HTTP Max Download   : {} KB/s",
                        cfg.http_max_download
                    )),
                    Line::from(format!(
                        " FTP Max Download    : {} KB/s",
                        cfg.ftp_max_download
                    )),
                    Line::from(format!(
                        " NZB Max Download    : {} KB/s",
                        cfg.nzb_max_download
                    )),
                    Line::from(format!(" eMule Enabled       : {}", cfg.emule_enabled)),
                    Line::from(format!(
                        " eMule Max Download  : {} KB/s",
                        cfg.emule_max_download
                    )),
                    Line::from(format!(
                        " eMule Max Upload    : {} KB/s",
                        cfg.emule_max_upload
                    )),
                    Line::from(format!(
                        " Unzip Service       : {}",
                        cfg.unzip_service_enabled
                    )),
                    Line::from(format!(" Default Dest.       : {}", default_dest)),
                    Line::from(format!(" eMule Default Dest. : {}", emule_dest)),
                    Line::from(""),
                    Line::from(format!(" Config file path    : {}", server_config_path)),
                    Line::from(format!(
                        " Server adress       : {}",
                        server_config.server_url
                    )),
                    Line::from(format!(" Server port         : {}", server_config.port)),
                    Line::from(format!(" User name           : {}", server_config.username)),
                    Line::from(format!(
                        " Refresh interval    : {}",
                        server_config.refresh_interval
                    )),
                ]);
                let server_info_block = Block::bordered()
                    .title(Line::from(" Server Info ".bold()))
                    .border_set(border::THICK)
                    .style(Style::default().bg(Color::Black).fg(Color::White))
                    .title_bottom(
                        Line::from(" Close this panel with <q> ").alignment(Alignment::Center),
                    );

                let server_info = Paragraph::new(txt)
                    .block(server_info_block)
                    .wrap(ratatui::widgets::Wrap { trim: false });

                // Clear the popup area first
                for y in popup_area.top()..popup_area.bottom() {
                    for x in popup_area.left()..popup_area.right() {
                        buf[(x, y)].set_char(' ').set_bg(Color::Black);
                    }
                }

                // Then render the server info paragraph
                server_info.render(popup_area, buf);
            }
        }

        // Add task popup
        if self.show_add_task {
            let popup_area = centered_rect(60, 5, area);
            let block = Block::bordered()
                .title(" Add URL and press <Enter>... ")
                .border_set(border::THICK)
                .title_bottom(
                    Line::from(" ...or close this panel with <ESC> ").alignment(Alignment::Center),
                );

            let paragraph = Paragraph::new(Line::from("")).block(block);
            // Clear background
            for y in popup_area.top()..popup_area.bottom() {
                for x in popup_area.left()..popup_area.right() {
                    buf[(x, y)].set_char(' ').set_bg(Color::Black);
                }
            }
            paragraph.render(popup_area, buf);
        }

        // Error popup
        if let Some(msg) = &self.error_popup {
            let popup_area = centered_rect(60, 30, area);
            let block = Block::bordered()
                .title(" Error ".bold().fg(Color::Red))
                .border_set(border::THICK)
                .style(Style::default().bg(Color::Black).fg(Color::Red))
                .title_bottom(
                    Line::from(" Close this panel with <q> ")
                        .fg(Color::Red)
                        .alignment(Alignment::Center),
                );
            let paragraph = Paragraph::new(msg.clone())
                .block(block)
                .wrap(Wrap { trim: true });
            // Clear background
            for y in popup_area.top()..popup_area.bottom() {
                for x in popup_area.left()..popup_area.right() {
                    buf[(x, y)].set_char(' ').set_bg(Color::Black);
                }
            }
            paragraph.render(popup_area, buf);
        }
    }
}
