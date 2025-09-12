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

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Should we show the help?
    pub show_help: bool,
    /// Should we show the server info?
    pub show_server_info: bool,
    /// Refreshing tasks
    pub refreshing_tasks: bool,
    /// Should we show add task popup?
    pub show_add_task_from_url: bool,
    /// Should we show add task file picker?
    pub show_add_task_from_file: bool,
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
    pub show_error_popup: bool,
    /// Should we show the confirmation popup?
    pub show_delete_confirmation_popup: bool,
    /// Is any popup active?
    pub is_popup_active: bool,
    /// Store config info received from the API
    pub dsconfig: Option<ConfigData>,
}

impl Default for App {
    fn default() -> Self {
        // Select the first row by default in the table
        let mut selected_row = TableState::default();
        selected_row.select(Some(0));
        let config = AppConfig::load().expect("Failed to load config file.");

        Self {
            running: true,
            show_help: false,
            show_server_info: false,
            refreshing_tasks: false,
            show_add_task_from_url: false,
            show_add_task_from_file: false,
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
            show_error_popup: false,
            show_delete_confirmation_popup: false,
            is_popup_active: false,
            dsconfig: None,
        }
    }
}

impl App {
    /// Constructs a new instance of [`App`].
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

    /// Handles the key events and updates the state of [`App`].
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Close the popups if they are open
                if self.is_popup_active {
                    self.close_all_popups();
                // Otherwise close the application
                } else {
                    self.events.send(AppEvent::Quit)
                }
            }
            KeyCode::Char('j') => {
                if self.show_add_task_from_file {
                    self.events.send(AppEvent::SelectNextRowFilePicker);
                } else if self.is_popup_active {
                    self.events.send(AppEvent::ScrollDown);
                } else if !self.is_popup_active {
                    self.events.send(AppEvent::SelectNextRow)
                }
            }
            KeyCode::Char('k') => {
                if self.show_add_task_from_file {
                    self.events.send(AppEvent::SelectPreviousRowFilePicker);
                } else if self.is_popup_active {
                    self.events.send(AppEvent::ScrollUp);
                } else if !self.is_popup_active {
                    self.events.send(AppEvent::SelectPreviousRow)
                }
            }
            KeyCode::Down => {
                if self.show_add_task_from_file {
                    self.events.send(AppEvent::SelectNextRowFilePicker);
                } else {
                    self.events.send(AppEvent::ScrollDownInfo);
                }
            }
            KeyCode::Up => {
                if self.show_add_task_from_file {
                    self.events.send(AppEvent::SelectPreviousRowFilePicker);
                } else {
                    self.events.send(AppEvent::ScrollUpInfo);
                }
            }
            KeyCode::Char('a') => self.events.send(AppEvent::ShowAddTaskFromUrl),
            KeyCode::Char('A') => self.events.send(AppEvent::ShowAddTaskFromFile),
            KeyCode::Enter => {
                if self.show_add_task_from_url {
                    self.events.send(AppEvent::AddTaskFromUrl);
                    self.show_add_task_from_url = false;
                }
                if self.show_add_task_from_file {
                    self.events.send(AppEvent::AddTaskFromFile);
                    self.show_add_task_from_file = false;
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if self.show_delete_confirmation_popup {
                    // Confirmation received, delete task
                    self.events.send(AppEvent::DeleteTask);
                    // Close the popup
                    self.show_delete_confirmation_popup = false;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if self.show_delete_confirmation_popup {
                    // No confirmation, leave the taks alone and close the popup
                    self.show_delete_confirmation_popup = false;
                }
            }
            KeyCode::Char('h') => {
                if !self.is_popup_active {
                    self.events.send(AppEvent::SelectPreviousTab)
                }
            }
            KeyCode::Char('l') => {
                if !self.is_popup_active {
                    self.events.send(AppEvent::SelectNextTab)
                }
            }
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
        self.selected_row.select_next();
    }

    /// Select previous row in task list
    pub fn select_previous_row(&mut self) {
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
        self.show_help = true;
        self.is_popup_active = true;
    }

    /// ServerInfo popup state
    pub fn show_server_info_popup(&mut self) {
        self.show_server_info = true;
        self.is_popup_active = true;
    }

    /// Add task popup state
    pub fn show_add_task_popup(&mut self) {
        self.show_add_task_from_url = true;
        self.is_popup_active = true;
    }

    /// Add task from file popup state
    pub fn show_add_task_file_picker(&mut self) {
        self.show_add_task_from_file = true;
        self.is_popup_active = true;
    }

    /// Error popup state
    pub fn show_error_popup(&mut self) {
        self.show_error_popup = true;
        self.is_popup_active = true;
    }

    /// Confirmation popup state
    pub fn show_delete_confirmation_popup(&mut self) {
        self.show_delete_confirmation_popup = true;
        self.is_popup_active = true;
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
                    self.show_error_popup = true;
                }
            },

            Err(e) => {
                self.error_message = Some(e);
                self.show_error_popup = true;
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
                self.show_error_popup = true;
            } else {
                self.load_tasks(client).await;
            }
        } else {
            self.error_message = Some("Failed to read file.".to_string());
            self.show_error_popup = true;
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

    // Close all popup from one place
    pub fn close_all_popups(&mut self) {
        self.show_help = false;
        self.show_server_info = false;
        self.show_add_task_from_url = false;
        self.show_add_task_from_file = false;
        self.show_error_popup = false;
        self.show_delete_confirmation_popup = false;
        self.error_message = None;
        self.is_popup_active = false;
    }
}
