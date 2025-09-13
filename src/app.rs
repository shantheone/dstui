use std::path::PathBuf;

use crate::AppConfig;
use crate::SynologyClient;
use crate::api::ConfigData;
use crate::api::DownloadTask;
use crate::api::ExtendedDownloadTask;
use crate::config;
use crate::event::{AppEvent, Event, EventHandler};
use crate::ui::get_selected_file;
use crate::util::FileAttributes;
use crate::util::get_file_content;
use crate::util::get_files;
use crate::util::{get_clipboard, validate_url};

use ratatui::widgets::ScrollbarState;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent},
    widgets::TableState,
};

/// Storing popups
#[derive(Debug, PartialEq)]
pub enum Popup {
    Help,
    AddTaskFromUrl,
    AddTaskFromFile,
    DeleteConfirmation,
    ServerInfo,
    Error,
}

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Refreshing tasks
    pub refreshing_tasks: bool,
    /// File path for add_task_from_file()
    pub file_path: PathBuf,
    /// So we will be able to handle the table selection state
    pub selected_row: TableState,
    /// So we will be able to handle the filepicker selection state
    pub selected_row_filepicker: TableState,
    /// Keep a list of files
    pub dir_list: Vec<FileAttributes>,
    /// Event handler.
    pub events: EventHandler,
    /// Store scroll offset for popups
    pub popup_scroll_position: usize,
    /// Headers for the tasks panel
    pub headers: Vec<&'static str>,
    /// Store scroll offset for info panel
    pub info_panel_scroll_position: usize,
    /// Info panel ScrollBarState
    pub info_panel_scrollbarstate: ratatui::widgets::ScrollbarState,
    /// Tabs for the info panel
    pub tabs: Vec<&'static str>,
    pub selected_tab: usize,
    /// Store all items in a tasklist
    pub items: Vec<DownloadTask>,
    /// Store config info received from the API
    pub extended_items: Vec<ExtendedDownloadTask>,
    /// Error popup content
    pub error_message: Option<String>,
    /// Store config info received from the API
    pub dsconfig: Option<ConfigData>,
    /// Store active popup information
    pub active_popup: Option<Popup>,
}

impl Default for App {
    fn default() -> Self {
        // Select the first row by default in the table
        let mut selected_row = TableState::default();
        selected_row.select(Some(0));
        let config = AppConfig::load().expect("Failed to load config file.");

        Self {
            running: true,
            refreshing_tasks: false,
            file_path: PathBuf::new(),
            selected_row,
            selected_row_filepicker: TableState::default(),
            dir_list: get_files(),
            events: EventHandler::new(&config),
            popup_scroll_position: 0,
            info_panel_scroll_position: 0,
            info_panel_scrollbarstate: ScrollbarState::new(0),
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
            tabs: vec!["General", "Transfer", "Tracker", "Peers", "File"],
            selected_tab: 0,
            items: vec![],
            extended_items: vec![],
            error_message: None,
            dsconfig: None,
            active_popup: None,
        }
    }
}

impl App {
    /// Constructs a new instance of App.
    pub fn new() -> Self {
        Self::default()
    }

    /// Run the application's main loop.
    pub async fn run(
        mut self,
        terminal: &mut DefaultTerminal,
        client: &mut SynologyClient,
    ) -> color_eyre::Result<()> {
        // Load tasks
        self.load_tasks(client).await;

        // Get server config from the API or raise an error if its unavailable
        self.dsconfig = match client.get_config().await {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                self.error_message = Some(format!("Failed to fetch server config:\n{e}"));
                None
            }
        };

        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                // Automatic refresh
                Event::AutoRefresh => {
                    self.refresh_tasks(client, terminal).await?;
                }
                Event::Crossterm(event) => {
                    if let crossterm::event::Event::Key(key_event) = event {
                        self.handle_key_events(key_event)?
                    }
                }
                Event::App(app_event) => match app_event {
                    AppEvent::SelectNextRow => self.select_next_row(),
                    AppEvent::SelectPreviousRow => self.select_previous_row(),
                    AppEvent::SelectNextRowFilePicker => self.select_next_row_filepicker(),
                    AppEvent::SelectPreviousRowFilePicker => self.select_previous_row_filepicker(),
                    AppEvent::Help => self.show_help_popup(),
                    AppEvent::ServerInfo => self.show_server_info_popup(),
                    AppEvent::ShowAddTaskFromUrl => self.show_add_task_popup(),
                    AppEvent::ShowAddTaskFromFile => self.show_add_task_file_picker(),
                    AppEvent::ShowError => self.show_error_popup(),
                    AppEvent::ShowDeleteConfirmation => self.show_delete_confirmation_popup(),
                    AppEvent::AddTaskFromUrl => self.add_task_from_url(client).await,
                    AppEvent::AddTaskFromFile => {
                        if let Some(selected_file) =
                            get_selected_file(&self.dir_list, &self.selected_row_filepicker)
                        {
                            self.add_task_from_file(client, selected_file.filepath.clone())
                                .await
                        }
                    }
                    AppEvent::PauseResumeTask => {
                        self.pause_task(client).await;
                        self.events.send(AppEvent::ManualRefresh);
                    }
                    AppEvent::DeleteTask => {
                        self.delete_task(client).await;
                        self.events.send(AppEvent::ManualRefresh);
                    }
                    // AppEvent::DeleteTask => self.add_task_from_url(client).await,
                    AppEvent::ManualRefresh => self.refresh_tasks(client, terminal).await?,
                    AppEvent::ScrollDown => self.scroll_down(),
                    AppEvent::ScrollUp => self.scroll_up(),
                    AppEvent::ScrollDownInfo => self.scroll_down_info(),
                    AppEvent::ScrollUpInfo => self.scroll_up_info(),
                    AppEvent::SelectNextTab => self.select_next_tab(),
                    AppEvent::SelectPreviousTab => self.select_previous_tab(),
                    AppEvent::Quit => self.quit(),
                },
            }
        }
        Ok(())
    }

    /// Handles the key events according to is a popup open or not
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        if self.active_popup.is_some() {
            self.handle_popup_keys(key_event)?;
        } else {
            self.handle_global_keys(key_event)?;
        }
        Ok(())
    }

    /// Handles the key events when a popup is open
    fn handle_popup_keys(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close_all_popups();
            }
            KeyCode::Char('j') => {
                if self.active_popup == Some(Popup::AddTaskFromFile) {
                    self.events.send(AppEvent::SelectNextRowFilePicker);
                } else {
                    self.events.send(AppEvent::ScrollDown);
                }
            }
            KeyCode::Char('k') => {
                if self.active_popup == Some(Popup::AddTaskFromFile) {
                    self.events.send(AppEvent::SelectPreviousRowFilePicker);
                } else {
                    self.events.send(AppEvent::ScrollUp);
                }
            }
            KeyCode::Down if self.active_popup == Some(Popup::AddTaskFromFile) => {
                self.events.send(AppEvent::SelectNextRowFilePicker);
            }
            KeyCode::Up if self.active_popup == Some(Popup::AddTaskFromFile) => {
                self.events.send(AppEvent::SelectPreviousRowFilePicker);
            }
            KeyCode::Char('y') | KeyCode::Char('Y')
                if self.active_popup == Some(Popup::DeleteConfirmation) =>
            {
                self.events.send(AppEvent::DeleteTask);
                self.close_all_popups();
            }
            KeyCode::Char('n') | KeyCode::Char('N')
                if self.active_popup == Some(Popup::DeleteConfirmation) =>
            {
                self.close_all_popups();
            }
            KeyCode::Enter if self.active_popup == Some(Popup::AddTaskFromUrl) => {
                self.events.send(AppEvent::AddTaskFromUrl);
                self.close_all_popups();
            }
            KeyCode::Enter if self.active_popup == Some(Popup::AddTaskFromFile) => {
                self.events.send(AppEvent::AddTaskFromFile);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handles global keys
    fn handle_global_keys(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.events.send(AppEvent::Quit);
            }
            KeyCode::Char('j') => self.events.send(AppEvent::SelectNextRow),
            KeyCode::Char('k') => self.events.send(AppEvent::SelectPreviousRow),
            KeyCode::Down => self.events.send(AppEvent::ScrollDownInfo),
            KeyCode::Up => self.events.send(AppEvent::ScrollUpInfo),
            KeyCode::Char('h') => self.events.send(AppEvent::SelectPreviousTab),
            KeyCode::Char('l') => self.events.send(AppEvent::SelectNextTab),
            KeyCode::Char('a') => self.events.send(AppEvent::ShowAddTaskFromUrl),
            KeyCode::Char('A') => self.events.send(AppEvent::ShowAddTaskFromFile),
            KeyCode::Char('i') => self.events.send(AppEvent::ServerInfo),
            KeyCode::Char('p') => self.events.send(AppEvent::PauseResumeTask),
            KeyCode::Char('r') => self.events.send(AppEvent::ManualRefresh),
            KeyCode::Char('d') => self.events.send(AppEvent::ShowDeleteConfirmation),
            KeyCode::Char('?') => self.events.send(AppEvent::Help),
            _ => {}
        }
        Ok(())
    }
    /// Handles the tick event of the terminal.
    ///
    /// The tick event is where you can update the state of your application with any logic that
    /// needs to be updated at a fixed frame rate. E.g. polling a server, updating an animation.
    pub fn tick(&self) {}

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Select next row in task list
    pub fn select_next_row(&mut self) {
        self.info_panel_scroll_position = 0;
        self.selected_row.select_next();
    }

    /// Select previous row in task list
    pub fn select_previous_row(&mut self) {
        self.info_panel_scroll_position = 0;
        self.selected_row.select_previous();
    }

    /// Select next row in the filepicker
    pub fn select_next_row_filepicker(&mut self) {
        self.selected_row_filepicker.select_next();
    }

    /// Select previous row in the filepicker
    pub fn select_previous_row_filepicker(&mut self) {
        self.selected_row_filepicker.select_previous();
    }

    /// Store the index of the selected row in the task list
    pub fn selected_table_row_index(&self) -> usize {
        self.selected_row.selected().unwrap_or(0)
    }

    /// Help popup state
    pub fn show_help_popup(&mut self) {
        self.active_popup = Some(Popup::Help);
    }

    /// ServerInfo popup state
    pub fn show_server_info_popup(&mut self) {
        self.active_popup = Some(Popup::ServerInfo);
    }

    /// Add task popup state
    pub fn show_add_task_popup(&mut self) {
        self.active_popup = Some(Popup::AddTaskFromUrl);
    }

    /// Add task from file popup state
    pub fn show_add_task_file_picker(&mut self) {
        self.active_popup = Some(Popup::AddTaskFromFile);
    }

    /// Error popup state
    pub fn show_error_popup(&mut self) {
        self.active_popup = Some(Popup::Error);
    }

    /// Confirmation popup state
    pub fn show_delete_confirmation_popup(&mut self) {
        self.active_popup = Some(Popup::DeleteConfirmation);
    }

    /// Scroll down in popup windows
    pub fn scroll_down(&mut self) {
        self.popup_scroll_position = self.popup_scroll_position.saturating_add(1);
    }

    /// Scroll up in popup windows
    pub fn scroll_up(&mut self) {
        self.popup_scroll_position = self.popup_scroll_position.saturating_sub(1);
    }

    /// Scroll down in info window
    pub fn scroll_down_info(&mut self) {
        self.info_panel_scroll_position = self.info_panel_scroll_position.saturating_add(1);
        self.info_panel_scrollbarstate = self
            .info_panel_scrollbarstate
            .position(self.info_panel_scroll_position);
    }

    /// Scroll up in info window
    pub fn scroll_up_info(&mut self) {
        self.info_panel_scroll_position = self.info_panel_scroll_position.saturating_sub(1);
        self.info_panel_scrollbarstate = self
            .info_panel_scrollbarstate
            .position(self.info_panel_scroll_position);
    }

    /// Select next tab in the info panel
    pub fn select_next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
    }

    /// Select previous tab in the info panel
    pub fn select_previous_tab(&mut self) {
        if self.selected_tab == 0 {
            self.selected_tab = self.tabs.len() - 1;
        } else {
            self.selected_tab -= 1;
        }
    }

    // Loads tasks from DownloadStation
    pub async fn load_tasks(&mut self, client: &SynologyClient) {
        // Storing last active task id for restoring after the refresh
        let prev_id = self
            .selected_row
            .selected()
            // .and_then(|i| self.items.get(i).map(|t| t.id.clone()));
            .and_then(|i| self.items.get(i).map(|t| t.id.clone()));

        match client.list_download_tasks().await {
            Ok(tasks) => self.items = tasks,
            Err(e) => {
                self.error_message = Some(format!("Failed to fetch tasks: {e}\n"));
                self.items.clear();
            }
        }

        if !self.items.is_empty() {
            let new_selection = prev_id
                // If we had an ID, try to find it in the refreshed list
                .and_then(|id| self.items.iter().position(|t| t.id == id))
                // Otherwise (or if not found), default to the top
                .or(Some(0));
            self.selected_row.select(new_selection);
        } else {
            // No tasks at all -> clear selection
            self.selected_row.select(None);
        }
    }

    pub async fn refresh_tasks(
        &mut self,
        client: &SynologyClient,
        terminal: &mut DefaultTerminal,
    ) -> color_eyre::Result<()> {
        self.refreshing_tasks = true;
        self.redraw(terminal)?;
        self.load_tasks(client).await;
        self.refreshing_tasks = false;
        self.redraw(terminal)?;
        Ok(())
    }

    pub fn extend_task_info(tasks: Vec<DownloadTask>) -> Vec<ExtendedDownloadTask> {
        tasks.into_iter().map(ExtendedDownloadTask::from).collect()
    }

    // Load config for displaying in the info panel
    pub fn load_config_file(&self) -> config::AppConfig {
        let config = AppConfig::load();
        // Want to panic if config file is not readable, since we already read or created it during
        // startup
        config.expect("❌ Config file missing or unaccessible, aborting.")
    }

    // Add task from url
    pub async fn add_task_from_url(&mut self, client: &mut SynologyClient) {
        // Get text content of clipboard
        let clipboard_text = get_clipboard();
        // Validate if it's a URL
        match validate_url(&clipboard_text) {
            Ok(()) => match client.create_task_from_url(&clipboard_text).await {
                Ok(()) => self.load_tasks(client).await,
                Err(e) => {
                    self.error_message = Some(format!("Failed to add task: {e}"));
                    self.show_error_popup();
                }
            },

            Err(e) => {
                self.error_message = Some(e);
                self.show_error_popup();
            }
        }
    }

    /// Return filelist
    pub fn return_file_list(&self) -> &[FileAttributes] {
        &self.dir_list
    }

    /// Add task from file
    pub async fn add_task_from_file(&mut self, client: &mut SynologyClient, file_path: String) {
        if let Ok(file_data) = get_file_content(file_path.clone()) {
            if let Err(e) = client.create_task_from_file(file_path, &file_data).await {
                self.error_message = Some(format!("Failed to add task: {e}"));
                self.show_error_popup();
            } else {
                self.load_tasks(client).await;
                self.close_all_popups();
            }
        } else {
            self.error_message = Some("Failed to read file.".to_string());
            self.show_error_popup();
        }
    }

    pub async fn pause_task(&mut self, client: &mut SynologyClient) {
        let idx = self.selected_table_row_index();
        let id = &self.items[idx].id.clone();

        let is_paused = self.items[idx].status.label() == "paused";
        let result = if is_paused {
            client.resume_task(id).await
        } else {
            client.pause_task(id).await
        };

        if let Err(e) = result {
            self.error_message = Some(format!(
                "{} task failed for: {id}\nError: {e}",
                if is_paused { "Resume" } else { "Pause" }
            ));
        }
    }

    pub async fn delete_task(&mut self, client: &mut SynologyClient) {
        let idx = self.selected_table_row_index();
        let id = &self.items[idx].id.clone();

        let result = client.delete_task(id).await;

        if let Err(e) = result {
            self.error_message = Some(format!("Delete task failed for: {id}\nError: {e}",));
        }
    }

    // Close and reset all popup from one place
    pub fn close_all_popups(&mut self) {
        self.active_popup = None;
        self.error_message = None;
        self.popup_scroll_position = 0;
        self.selected_row_filepicker = TableState::default();
    }
}
