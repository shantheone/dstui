use crate::config::{Config, config_path};
use crate::event::{AppEvent, Event, EventHandler, TICK_FPS};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    crossterm::{cursor, execute},
    style::{Color, Style},
    widgets::{Block, BorderType, TableState},
};
use ratatui_explorer::{FileExplorer, FileExplorerBuilder, Theme};
use std::io::stdout;
use syno_download_station::{
    client::SynoDS,
    entities::{Task, TaskStatus},
};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler as InputEventHandler;

// Spinner frames
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, PartialEq, Clone)]
pub enum SortColumn {
    Name,
    Size,
    Downloaded,
    Uploaded,
    Progress,
    UploadSpeed,
    DownloadSpeed,
    Ratio,
    Status,
}

#[derive(Debug, PartialEq, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, PartialEq)]
pub enum ActivePanel {
    Tasks,
    Info,
}

/// Enum for handling the popups
#[derive(Debug, Clone)]
pub struct PopupState {
    pub lines: Vec<String>,
    pub error: bool, // true = red border, false = normal
    pub scroll: usize,
}

/// Enum for confirmation
#[derive(Debug, Clone)]
pub enum PendingAction {
    DeleteTask(String), // stores the task id
}

pub struct App {
    pub running: bool,
    pub active_panel: ActivePanel,
    pub headers: Vec<&'static str>,
    pub refreshing_tasks: bool,
    pub events: EventHandler,
    pub tabs: Vec<&'static str>,
    pub selected_tab: usize,
    pub selected_task: TableState,
    pub selected_file: TableState,
    pub selected_peer: TableState,
    pub tasks: Vec<Task>,
    pub client: Option<SynoDS>,
    pub destination: String,
    pub tick_count: u64,
    pub refresh_interval: Option<u64>, // number of ticks between refreshes, None means disabled
    pub tracker_scroll: usize,
    pub peer_scroll: usize,
    pub file_scroll: usize,
    pub tracker_count: usize,
    pub peer_count: usize,
    pub file_count: usize,
    pub file_explorer: Option<FileExplorer>,
    pub url_input: Option<Input>,
    pub popup: Option<PopupState>,
    pub url_input_cursor_pos: Option<(u16, u16)>,
    // Tracking scrollable areas
    pub popup_inner_height: usize,
    pub tracker_inner_height: usize,
    pub peer_inner_height: usize,
    pub file_inner_height: usize,
    pub pending_action: Option<PendingAction>,
    pub spinner_frame: usize,
    pub loading: bool,
    pub config_path: String,
    pub sort_column: SortColumn,
    pub sort_order: SortOrder,
}

fn move_next(state: &mut TableState, row_count: usize) {
    if row_count == 0 {
        return;
    }
    let next = match state.selected() {
        Some(i) => (i + 1).min(row_count - 1),
        None => 0,
    };
    state.select(Some(next));
}

fn move_previous(state: &mut TableState) {
    let prev = match state.selected() {
        Some(0) | None => 0,
        Some(i) => i - 1,
    };
    state.select(Some(prev));
}

impl App {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let client = SynoDS::builder()
            .url(&config.connection.url)
            .username(&config.connection.username)
            .password(&config.connection.password)
            .danger_accept_invalid_certs(config.connection.accept_invalid_certs)
            .build()?;

        client.authorize().await?;

        let config_path = config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let destination = config.downloads.destination.clone();
        let refresh_interval = config
            .downloads
            .refresh_interval
            .filter(|&s| s > 0)
            .map(|s| (s as f64 * TICK_FPS) as u64);

        let mut app = Self {
            running: true,
            headers: vec![
                "Name",
                "Size",
                "Downloaded",
                "Uploaded",
                "Progress",
                "Upload Speed",
                "Download Speed",
                "Ratio",
                "Status",
            ],
            active_panel: ActivePanel::Tasks,
            refreshing_tasks: false,
            events: EventHandler::new(),
            tabs: vec!["General", "Transfer", "Tracker", "Peers", "Files"],
            selected_tab: 0,
            selected_task: TableState::default(),
            selected_file: TableState::default(),
            selected_peer: TableState::default(),
            tasks: vec![],
            client: Some(client),
            destination,
            tick_count: 0,
            refresh_interval,
            tracker_scroll: 0,
            peer_scroll: 0,
            file_scroll: 0,
            tracker_count: 0,
            peer_count: 0,
            file_count: 0,
            file_explorer: None,
            url_input: None,
            popup: None,
            url_input_cursor_pos: None,
            // Scrollable areas custom defaults
            popup_inner_height: 5,
            tracker_inner_height: 5,
            peer_inner_height: 5,
            file_inner_height: 5,
            pending_action: None,
            spinner_frame: 0,
            loading: false,
            config_path,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
        };

        app.refresh_tasks().await?;
        Ok(app)
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> anyhow::Result<()> {
        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            // Show blinking cursor when URL input is active, hide otherwise
            if self.url_input.is_some() {
                execute!(stdout(), cursor::Show, cursor::EnableBlinking)?;
            } else {
                execute!(stdout(), cursor::Hide)?;
            }

            if self.url_input.is_some() {
                if let Some((x, y)) = self.url_input_cursor_pos {
                    execute!(
                        stdout(),
                        cursor::Show,
                        cursor::EnableBlinking,
                        cursor::MoveTo(x, y)
                    )?;
                }
            } else {
                execute!(stdout(), cursor::Hide)?;
            }

            match self.events.next().await? {
                Event::Tick => self.tick().await?,
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event)
                        if key_event.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        self.handle_key_events(key_event)?
                    }
                    _ => {}
                },
                Event::App(app_event) => match app_event {
                    AppEvent::Quit => self.quit(),
                    AppEvent::Refresh => {
                        match self.refresh_tasks().await {
                            Ok(_) => {}
                            Err(e) => {
                                self.show_popup(
                                    vec!["Failed to refresh tasks:".into(), e.to_string()],
                                    true,
                                );
                            }
                        }
                        self.loading = false;
                    }
                    AppEvent::Next => self.next_task_row(),
                    AppEvent::Previous => self.previous_task_row(),
                    AppEvent::OpenFilePicker => self.open_file_picker(),
                    AppEvent::SubmitFile => {
                        if let Err(e) = self.submit_selected_file().await {
                            self.show_popup(
                                vec!["Failed to submit file:".into(), e.to_string()],
                                true,
                            );
                        }
                    }
                    AppEvent::OpenUrlInput => self.open_url_input(),
                    AppEvent::SubmitUrl => {
                        if let Err(e) = self.submit_url().await {
                            self.show_popup(
                                vec!["Failed to submit URL:".into(), e.to_string()],
                                true,
                            );
                        }
                    }
                    AppEvent::ToggleTask => {
                        if let Err(e) = self.toggle_task().await {
                            self.show_popup(
                                vec!["Failed to toggle task status:".into(), e.to_string()],
                                true,
                            );
                        }
                    }
                    AppEvent::CompleteTask => {
                        if let Err(e) = self.complete_task().await {
                            self.show_popup(
                                vec!["Failed to complete task:".into(), e.to_string()],
                                true,
                            );
                        }
                    }
                    AppEvent::ClearCompleted => {
                        if let Err(e) = self.clear_completed().await {
                            self.show_popup(
                                vec!["Failed to clear completed task(s):".into(), e.to_string()],
                                true,
                            );
                        }
                    }
                    AppEvent::PopUp => self.show_popup(
                        vec![
                            String::new(),
                            "Help:".into(),
                            "j / k     — navigate tasks and info panel rows, scroll help text"
                                .into(),
                            "p         — pause / resume selected task".into(),
                            "c         — complete selected task".into(),
                            "C         — clear completed tasks".into(),
                            "a         — add file (.torrent, .nzb and .txt is supported)".into(),
                            "A         — add task by URL".into(),
                            "d         — delete selected task".into(),
                            "r         — manually refresh tasks".into(),
                            "1-9       — sort by column (again to reverse)".into(),
                            "Tab       — switch panels".into(),
                            "?         — toggle this help popup".into(),
                            "q / Esc   — quit".into(),
                            String::new(),
                            format!("Config:   {}", self.config_path),
                        ],
                        false,
                    ),
                    AppEvent::DeleteTask => self.request_delete_task(),
                    AppEvent::ConfirmAction => self.confirm_action().await?,
                    AppEvent::CancelAction => self.cancel_action(),
                },
            }
        }

        // Make sure cursor is restored when app exits
        execute!(stdout(), cursor::Show, cursor::EnableBlinking)?;
        Ok(())
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> anyhow::Result<()> {
        // Confirmation popup blocks all other input
        if self.pending_action.is_some() {
            match key_event.code {
                KeyCode::Char('y') | KeyCode::Enter => self.events.send(AppEvent::ConfirmAction),
                KeyCode::Char('n') | KeyCode::Esc => self.events.send(AppEvent::CancelAction),
                _ => {}
            }
            return Ok(());
        }

        // Popup is the next in the blocking chain
        if let Some(popup) = &mut self.popup {
            match key_event.code {
                KeyCode::Esc => self.close_popup(),
                KeyCode::Char('j') => {
                    let max = popup.lines.len().saturating_sub(self.popup_inner_height);
                    popup.scroll = (popup.scroll + 1).min(max);
                }
                KeyCode::Char('k') => {
                    popup.scroll = popup.scroll.saturating_sub(1);
                }
                _ => {}
            }
            return Ok(());
        }

        // Then we will handle the file picker
        if let Some(explorer) = &mut self.file_explorer {
            match key_event.code {
                KeyCode::Enter => self.events.send(AppEvent::SubmitFile),
                KeyCode::Esc => self.file_explorer = None,
                _ => {
                    explorer
                        .handle(&crossterm::event::Event::Key(key_event))
                        .unwrap();
                }
            }
            return Ok(());
        }

        // Then the URL input gets priority when open
        if self.url_input.is_some() {
            match key_event.code {
                KeyCode::Enter => self.events.send(AppEvent::SubmitUrl),
                KeyCode::Esc => self.url_input = None,
                _ => {
                    if let Some(input) = &mut self.url_input {
                        input.handle_event(&crossterm::event::Event::Key(key_event));
                    }
                }
            }
            return Ok(());
        }

        // Finally, normal key handling
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('?') => self.events.send(AppEvent::PopUp),
            KeyCode::Char('r') => self.events.send(AppEvent::Refresh),
            KeyCode::Char('a') => self.events.send(AppEvent::OpenFilePicker),
            KeyCode::Char('A') => self.events.send(AppEvent::OpenUrlInput),
            KeyCode::Char('j') if self.active_panel == ActivePanel::Tasks => {
                self.events.send(AppEvent::Next)
            }
            KeyCode::Char('k') if self.active_panel == ActivePanel::Tasks => {
                self.events.send(AppEvent::Previous)
            }
            KeyCode::Char('h') if self.active_panel == ActivePanel::Info => {
                self.selected_tab = self.selected_tab.saturating_sub(1);
            }
            KeyCode::Char('l') if self.active_panel == ActivePanel::Info => {
                self.selected_tab = (self.selected_tab + 1).min(self.tabs.len() - 1);
            }
            KeyCode::Char('j') if self.active_panel == ActivePanel::Info => {
                self.scroll_info_down();
            }
            KeyCode::Char('k') if self.active_panel == ActivePanel::Info => {
                self.scroll_info_up();
            }
            KeyCode::Char('p') => self.events.send(AppEvent::ToggleTask),
            KeyCode::Char('c') => self.events.send(AppEvent::CompleteTask),
            KeyCode::Char('C') => self.events.send(AppEvent::ClearCompleted),
            KeyCode::Tab => {
                self.active_panel = match self.active_panel {
                    ActivePanel::Tasks => ActivePanel::Info,
                    ActivePanel::Info => ActivePanel::Tasks,
                }
            }
            KeyCode::Char('d') => self.events.send(AppEvent::DeleteTask),
            // Keys for sorting the columns
            KeyCode::Char('1') => self.sort_by(SortColumn::Name),
            KeyCode::Char('2') => self.sort_by(SortColumn::Size),
            KeyCode::Char('3') => self.sort_by(SortColumn::Downloaded),
            KeyCode::Char('4') => self.sort_by(SortColumn::Uploaded),
            KeyCode::Char('5') => self.sort_by(SortColumn::Progress),
            KeyCode::Char('6') => self.sort_by(SortColumn::UploadSpeed),
            KeyCode::Char('7') => self.sort_by(SortColumn::DownloadSpeed),
            KeyCode::Char('8') => self.sort_by(SortColumn::Ratio),
            KeyCode::Char('9') => self.sort_by(SortColumn::Status),
            _ => {}
        }
        Ok(())
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        if self.loading {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }
        if let Some(interval) = self.refresh_interval {
            self.tick_count += 1;
            if self.tick_count >= interval {
                self.tick_count = 0;
                self.refresh_tasks().await?;
            }
        }
        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn refresh_tasks(&mut self) -> anyhow::Result<()> {
        if let Some(client) = &self.client {
            self.loading = true;
            self.refreshing_tasks = true;
            let result = client.get_tasks().await;
            self.loading = false;
            self.refreshing_tasks = false;
            match result {
                Ok(result) => {
                    self.tasks = result.task;
                    if !self.tasks.is_empty() {
                        self.selected_task.select(Some(0));
                        self.update_info_counts();
                    }
                }
                Err(e) => {
                    self.show_popup(vec!["Failed to get tasks:".into(), e.to_string()], true);
                }
            }
        }
        Ok(())
    }

    // Task panel scroll and row selection
    pub fn next_task_row(&mut self) {
        move_next(&mut self.selected_task, self.tasks.len());
        self.reset_info_scroll();
        self.update_info_counts();
    }
    pub fn previous_task_row(&mut self) {
        move_previous(&mut self.selected_task);
        self.reset_info_scroll();
        self.update_info_counts();
    }

    // Info Panel scroll and row selection
    fn reset_info_scroll(&mut self) {
        self.tracker_scroll = 0;
        self.peer_scroll = 0;
        self.file_scroll = 0;
    }

    pub fn scroll_info_down(&mut self) {
        match self.selected_tab {
            2 => {
                let max = self.tracker_count.saturating_sub(self.tracker_inner_height);
                self.tracker_scroll = (self.tracker_scroll + 1).min(max);
            }
            3 => {
                let max = self.peer_count.saturating_sub(self.peer_inner_height);
                self.peer_scroll = (self.peer_scroll + 1).min(max);
            }
            4 => {
                let max = self.file_count.saturating_sub(self.file_inner_height);
                self.file_scroll = (self.file_scroll + 1).min(max);
            }
            _ => {}
        }
    }

    pub fn scroll_info_up(&mut self) {
        match self.selected_tab {
            2 => self.tracker_scroll = self.tracker_scroll.saturating_sub(1),
            3 => self.peer_scroll = self.peer_scroll.saturating_sub(1),
            4 => self.file_scroll = self.file_scroll.saturating_sub(1),
            _ => {}
        }
    }

    pub fn selected_task_index(&self) -> Option<usize> {
        self.selected_task.selected()
    }

    pub fn show_popup(&mut self, lines: Vec<String>, error: bool) {
        self.popup = Some(PopupState {
            lines,
            error,
            scroll: 0,
        });
    }

    pub fn close_popup(&mut self) {
        self.popup = None;
    }

    pub fn update_info_counts(&mut self) {
        if let Some(real_idx) = self.selected_task_in_sorted()
            && let Some(task) = self.tasks.get(real_idx)
        {
            self.tracker_count = task
                .additional
                .as_ref()
                .and_then(|a| a.tracker.as_ref())
                .map(|t| t.len())
                .unwrap_or(0);
            self.peer_count = task
                .additional
                .as_ref()
                .and_then(|a| a.peer.as_ref())
                .map(|p| p.len())
                .unwrap_or(0);
            self.file_count = task
                .additional
                .as_ref()
                .and_then(|a| a.file.as_ref())
                .map(|f| f.len())
                .unwrap_or(0);
        }
    }

    // File picker methods
    pub fn open_file_picker(&mut self) {
        let theme = Theme::default()
            .add_default_title()
            .with_block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" File Picker "),
            )
            .with_title_bottom(|_| {
                " .torrent / .nzb / .txt · Enter to select · Esc to cancel ".into()
            });
        self.file_explorer = Some(FileExplorerBuilder::build_with_theme(theme).unwrap());
    }
    pub async fn submit_selected_file(&mut self) -> anyhow::Result<()> {
        let allowed_extensions = ["torrent", "nzb", "txt"];

        // Extract everything we need from the explorer before any API calls
        let file_data = if let Some(explorer) = &self.file_explorer {
            let path = explorer.current();
            if path.is_file() {
                let ext = path.path.extension().and_then(|e| e.to_str());
                if ext
                    .map(|e| allowed_extensions.contains(&e))
                    .unwrap_or(false)
                {
                    let filename = path
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("upload.torrent")
                        .to_string();
                    match std::fs::read(&path.path) {
                        Ok(bytes) => Some((bytes, filename)),
                        Err(e) => {
                            self.show_popup(
                                vec!["Failed to read file:".into(), e.to_string()],
                                true,
                            );
                            None
                        }
                    }
                } else {
                    self.show_popup(
                        vec![
                            "Unsupported file type.".into(),
                            format!("Got: .{}", ext.unwrap_or("unknown")),
                            "Only .torrent, .nzb and .txt are supported.".into(),
                        ],
                        true,
                    );
                    None
                }
            } else {
                // selected a directory, do nothing silently
                None
            }
        } else {
            None
        };

        self.file_explorer = None;

        if let Some((file_bytes, filename)) = file_data
            && let Some(client) = &self.client
        {
            let _ = client
                .create_task_from_file(&file_bytes, &filename, &self.destination)
                .await;
            if let Err(e) = self.refresh_tasks().await {
                self.show_popup(
                    vec!["Task added but refresh failed:".into(), e.to_string()],
                    true,
                );
            }
        }

        Ok(())
    }

    // Add by URL methods
    pub fn open_url_input(&mut self) {
        self.url_input = Some(Input::default());
    }

    pub async fn submit_url(&mut self) -> anyhow::Result<()> {
        if let Some(input) = &self.url_input {
            let url = input.value().trim().to_string();
            if url.is_empty() {
                self.show_popup(vec!["Please enter a URL.".into()], true);
            } else if !url.starts_with("http://")
                && !url.starts_with("https://")
                && !url.starts_with("magnet:")
            {
                self.show_popup(
                    vec![
                        "Invalid URL format.".into(),
                        "Must start with http://, https://, or magnet:".into(),
                    ],
                    true,
                );
            } else if let Some(client) = &self.client {
                self.loading = true;
                let _ = client.create_task(&url, &self.destination).await;
                self.events.send(AppEvent::Refresh);
            }
        }
        self.url_input = None;
        Ok(())
    }

    // Toggle task status (pause/resume)
    pub async fn toggle_task(&mut self) -> anyhow::Result<()> {
        // Extract task id and status before borrowing client
        let task_info = if let Some(real_idx) = self.selected_task_in_sorted() {
            self.tasks.get(real_idx).map(|task| {
                let should_pause = matches!(
                    task.status,
                    TaskStatus::Downloading | TaskStatus::Waiting | TaskStatus::Seeding
                );
                let is_paused = matches!(task.status, TaskStatus::Paused | TaskStatus::Finished);
                (task.id.clone(), should_pause, is_paused)
            })
        } else {
            None
        };

        if let Some((task_id, should_pause, is_paused)) = task_info {
            if let Some(client) = &self.client {
                let result = if should_pause {
                    client.pause(&task_id).await.map(|_| ())
                } else if is_paused {
                    client.resume(&task_id).await.map(|_| ())
                } else {
                    return Ok(());
                };

                if let Err(e) = result {
                    self.show_popup(vec!["Failed to toggle task:".into(), e.to_string()], true);
                    return Ok(());
                }
            }

            if let Err(e) = self.refresh_tasks().await {
                self.show_popup(vec!["Failed to refresh tasks:".into(), e.to_string()], true);
            }
        }

        Ok(())
    }

    // Complete task
    pub async fn complete_task(&mut self) -> anyhow::Result<()> {
        if let Some(real_idx) = self.selected_task_in_sorted()
            && let Some(task) = self.tasks.get(real_idx)
            && let Some(client) = &self.client
        {
            if let Err(e) = client.complete(&task.id).await {
                self.show_popup(vec!["Failed to complete task:".into(), e.to_string()], true);
            }

            if let Err(e) = self.refresh_tasks().await {
                self.show_popup(vec!["Failed to refresh tasks:".into(), e.to_string()], true);
            }
        }
        Ok(())
    }

    /// Clear completed tasks
    pub async fn clear_completed(&mut self) -> anyhow::Result<()> {
        if let Some(client) = &self.client {
            if let Err(e) = client.clear_completed().await {
                self.show_popup(
                    vec!["Failed to clear completed task(s):".into(), e.to_string()],
                    true,
                );
            }
            if let Err(e) = self.refresh_tasks().await {
                self.show_popup(vec!["Failed to refresh tasks:".into(), e.to_string()], true);
            }
        }
        Ok(())
    }

    /// Delete selected task
    pub fn request_delete_task(&mut self) {
        if let Some(real_idx) = self.selected_task_in_sorted()
            && let Some(task) = self.tasks.get(real_idx)
        {
            self.pending_action = Some(PendingAction::DeleteTask(task.id.clone()));
            self.show_popup(
                vec![
                    format!("Delete task: {}?", task.title),
                    String::new(),
                    "  y / Enter — confirm".into(),
                    "  n / Esc   — cancel".into(),
                ],
                false,
            );
        }
    }

    /// Confirm action popup (delete uses this only at the moment)
    pub async fn confirm_action(&mut self) -> anyhow::Result<()> {
        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::DeleteTask(task_id) => {
                    self.close_popup();
                    if let Some(client) = &self.client {
                        match client.delete_task(&task_id, false).await {
                            Ok(_) => {
                                if let Err(e) = self.refresh_tasks().await {
                                    self.show_popup(
                                        vec![
                                            "Task deleted but refresh failed:".into(),
                                            e.to_string(),
                                        ],
                                        true,
                                    );
                                }
                            }
                            Err(e) => {
                                self.show_popup(
                                    vec!["Failed to delete task:".into(), e.to_string()],
                                    true,
                                );
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Cancel action
    pub fn cancel_action(&mut self) {
        self.pending_action = None;
        self.close_popup();
    }

    pub fn sort_by(&mut self, column: SortColumn) {
        if self.sort_column == column {
            // Same column — toggle order
            self.sort_order = match self.sort_order {
                SortOrder::Ascending => SortOrder::Descending,
                SortOrder::Descending => SortOrder::Ascending,
            };
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        self.selected_task.select(Some(0));
        self.reset_info_scroll();
    }

    pub fn sorted_tasks(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.iter().collect();

        tasks.sort_by(|a, b| {
            let ord = match self.sort_column {
                SortColumn::Name => a.title.cmp(&b.title),
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::Progress => a
                    .calculate_progress()
                    .partial_cmp(&b.calculate_progress())
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Ratio => a
                    .calculate_ratio()
                    .partial_cmp(&b.calculate_ratio())
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Status => format!("{:?}", a.status).cmp(&format!("{:?}", b.status)),
                SortColumn::Downloaded => {
                    let a_dl = a
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.size_downloaded)
                        .unwrap_or(0);
                    let b_dl = b
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.size_downloaded)
                        .unwrap_or(0);
                    a_dl.cmp(&b_dl)
                }
                SortColumn::Uploaded => {
                    let a_ul = a
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.size_uploaded)
                        .unwrap_or(0);
                    let b_ul = b
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.size_uploaded)
                        .unwrap_or(0);
                    a_ul.cmp(&b_ul)
                }
                SortColumn::UploadSpeed => {
                    let a_us = a
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.speed_upload)
                        .unwrap_or(0);
                    let b_us = b
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.speed_upload)
                        .unwrap_or(0);
                    a_us.cmp(&b_us)
                }
                SortColumn::DownloadSpeed => {
                    let a_ds = a
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.speed_download)
                        .unwrap_or(0);
                    let b_ds = b
                        .additional
                        .as_ref()
                        .and_then(|x| x.transfer.as_ref())
                        .map(|t| t.speed_download)
                        .unwrap_or(0);
                    a_ds.cmp(&b_ds)
                }
            };

            match self.sort_order {
                SortOrder::Ascending => ord,
                SortOrder::Descending => ord.reverse(),
            }
        });

        tasks
    }

    pub fn selected_task_in_sorted(&self) -> Option<usize> {
        // Returns the index into self.tasks of the currently selected sorted row
        let idx = self.selected_task.selected()?;
        let sorted = self.sorted_tasks();
        sorted
            .get(idx)
            .and_then(|task| self.tasks.iter().position(|t| t.id == task.id))
    }
}
