use crate::app::App;
use crate::util::{
    format_seconds, format_timestamp, get_clipboard, get_files, render_progress_bar,
};
use crate::{AppConfig, util::format_bytes};

use ratatui::style::Styled;
use ratatui::widgets::TableState;
use ratatui::{
    Terminal,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Text},
    widgets::{
        Block, BorderType, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, StatefulWidget, Table, Tabs, Widget,
    },
};

impl Widget for &mut App {
    /// Renders the user interface widgets.
    ///
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui/ratatui/tree/master/examples
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Set up the two-part layout
        let chunks =
            Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).split(area);

        // When refreshing tasks
        let title_text = if self.refreshing_tasks {
            " DownloadStation TUI Client - [Refreshing...] "
                .bold()
                .blue()
        // ...and when not
        } else {
            " DownloadStation TUI Client ".bold().blue()
        };

        // Add a table to the top chunk
        let table_block = Block::bordered()
            .title(title_text)
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        // Row selection
        let table_row_index = self.selected_table_row_index(); // Need the selected row's index later
        let table_state = &mut self.selected_row;

        // Create the table with content and styling
        let header = Row::new(self.headers.clone())
            .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold());

        let download_tasks = App::extend_task_info(self.items.clone());
        let rows = download_tasks.iter().map(|item| {
            let cells = item.to_row_cells();
            Row::new(cells)
        });

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

        // Add a scrollbar
        let table_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut table_scrollbar_state = ScrollbarState::new(rows.len())
            .position(table_row_index)
            .viewport_content_length(1);

        let table = Table::new(rows, widths)
            .block(table_block)
            .header(header)
            .row_highlight_style(Style::new().reversed())
            .column_spacing(1);

        // Render stateful widget of the table
        StatefulWidget::render(table, chunks[0], buf, table_state);
        // Render stateful widget of the table scrollbar
        StatefulWidget::render(
            table_scrollbar,
            chunks[0].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut table_scrollbar_state,
        );

        // Add an info block to the bottom chunk
        let info_block = Block::bordered()
            .title(" Info ".bold())
            .title_alignment(Alignment::Center)
            .title_bottom(" Scroll with ↑ / ↓")
            .border_type(BorderType::Rounded);
        info_block.render(chunks[1], buf);

        // Carve out inner area (leave border margin)
        let inner_area = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(chunks[1]);

        // Tabs go in inner[0]
        let tab_titles = self.tabs.iter().map(|t| Line::from(*t)).collect::<Vec<_>>();
        let tabs = Tabs::new(tab_titles)
            .select(self.selected_tab)
            .highlight_style(Style::default().fg(Color::Yellow))
            .divider("│")
            .bg(Color::DarkGray);
        tabs.render(inner_area[0], buf);

        // Counter for the lines in the text for the info panel scrollbar
        let mut info_text_line_count = 0;

        // Create info panel tab contents
        let tab_content = match self.selected_tab {
            // General tab
            0 => {
                let general_tab = Text::from(vec![
                    Line::from(""),
                    Line::from(format!(
                        "ID                  : {}",
                        download_tasks[self.selected_table_row_index()].task_id
                    )),
                    Line::from(format!(
                        "Title               : {}",
                        download_tasks[self.selected_table_row_index()].task_title
                    )),
                    Line::from(format!(
                        "Destination         : {}",
                        download_tasks[self.selected_table_row_index()].task_destination
                    )),
                    Line::from(format!(
                        "Size                : {}",
                        format_bytes(download_tasks[self.selected_table_row_index()].task_size)
                    )),
                    Line::from(format!(
                        "User name           : {}",
                        download_tasks[self.selected_table_row_index()].task_username
                    )),
                    Line::from(format!(
                        "URL / File Name     : {}",
                        download_tasks[self.selected_table_row_index()].task_uri
                    )),
                    Line::from(format!(
                        "Created time        : {}",
                        format_timestamp(
                            download_tasks[self.selected_table_row_index()].task_create_time
                        )
                    )),
                    Line::from(format!(
                        "Completed time      : {}",
                        format_timestamp(
                            download_tasks[self.selected_table_row_index()].task_completed_time
                        )
                    )),
                    Line::from(format!(
                        "Estimated wait time : {}",
                        format_seconds(
                            download_tasks[self.selected_table_row_index()].waiting_seconds
                        )
                    )),
                ]);
                info_text_line_count = general_tab.height();
                general_tab
            }
            // Transfer tab
            1 => {
                self.info_panel_scroll_position = 0;
                let progress_percentage = (download_tasks[self.selected_table_row_index()]
                    .task_size_downloaded as f64
                    / download_tasks[self.selected_table_row_index()].task_size as f64)
                    * 100.0;
                let transfer_tab = Text::from(vec![
                    Line::from(""),
                    Line::from(format!(
                        "Status              : {}",
                        download_tasks[self.selected_table_row_index()]
                            .task_status
                            .label()
                    )),
                    Line::from(format!(
                        "Tranferred (UL/DL)  : {} / {} Ratio: {:.2}",
                        format_bytes(
                            download_tasks[self.selected_table_row_index()].task_size_uploaded
                        ),
                        format_bytes(
                            download_tasks[self.selected_table_row_index()].task_size_downloaded
                        ),
                        download_tasks[self.selected_table_row_index()].task_ratio
                    )),
                    Line::from(format!(
                        "Progress            : {}",
                        render_progress_bar(progress_percentage as u64, 20)
                    )),
                    Line::from(format!(
                        "Speed DL / UL       : {} / {}",
                        format_bytes(
                            download_tasks[self.selected_table_row_index()].task_speed_download
                        ),
                        format_bytes(
                            download_tasks[self.selected_table_row_index()].task_speed_upload
                        ),
                    )),
                    Line::from(format!(
                        "Peers               : {}",
                        download_tasks[self.selected_table_row_index()].total_peers
                    )),
                    Line::from(format!(
                        "Connected peers     : {}",
                        download_tasks[self.selected_table_row_index()].connected_peers
                    )),
                    Line::from(format!(
                        "Total pieces        : {}",
                        download_tasks[self.selected_table_row_index()].total_pieces
                    )),
                    Line::from(format!(
                        "Downloaded pieces   : {}",
                        download_tasks[self.selected_table_row_index()].task_downloaded_pieces
                    )),
                    Line::from(format!(
                        "Seed elapsed        : {}",
                        format_seconds(
                            download_tasks[self.selected_table_row_index()].task_seedelapsed
                        )
                    )),
                    Line::from(format!(
                        "Seeders / Leechers  : {} / {}",
                        download_tasks[self.selected_table_row_index()].connected_seeders,
                        download_tasks[self.selected_table_row_index()].connected_leechers
                    )),
                    Line::from(format!(
                        "Start time          : {}",
                        format_timestamp(
                            download_tasks[self.selected_table_row_index()].task_started_time
                        )
                    )),
                ]);
                info_text_line_count = transfer_tab.height();
                transfer_tab
            }

            // Trackers
            2 => {
                let mut trackers_tab = Text::from(vec![Line::from("")]);

                for trackers in download_tasks[self.selected_table_row_index()]
                    .trackers
                    .clone()
                {
                    let line = Line::from(format!(
                        "URL: {} | Status: {} | Next Update: {} | Seeds: {} | Peers: {}",
                        trackers.url,
                        trackers.status,
                        format_seconds(trackers.update_timer),
                        trackers.seeds,
                        trackers.peers,
                    ));
                    trackers_tab.push_line(line);
                }
                info_text_line_count = trackers_tab.height();
                trackers_tab
            }

            // Peers
            3 => {
                let mut peers_tab = Text::from(vec![Line::from("")]);

                for peers in download_tasks[self.selected_table_row_index()]
                    .peers
                    .clone()
                {
                    let line = Line::from(format!(
                        "IP: {} | Agent: {} | Progress: {} | Download Speed: {} | Upload Speed: {}",
                        peers.address,
                        peers.agent,
                        render_progress_bar((peers.progress * 100.0) as u64, 10),
                        format_bytes(peers.speed_download),
                        format_bytes(peers.speed_upload),
                    ));
                    peers_tab.push_line(line);
                }
                info_text_line_count = peers_tab.height();
                peers_tab
            }

            // Files
            4 => {
                // File
                let mut files_tab = Text::from(vec![Line::from("")]);

                for files in download_tasks[self.selected_table_row_index()]
                    .files
                    .clone()
                {
                    let line = Line::from(format!(
                        "{} | {} | Downloaded: {} | Priority: {}",
                        files.filename,
                        format_bytes(files.size),
                        format_bytes(files.size_downloaded),
                        files.priority
                    ));
                    files_tab.push_line(line);
                }
                info_text_line_count = files_tab.height();
                files_tab
            }
            _ => Text::from(vec![Line::from("")]),
        };

        // Add a scrollbar to the info panel
        // TODO: Make text wrapped in the info panel and calculate for the extra lines somehow
        let info_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        // Visible lines and Max scroll value for the scrollbar
        let visible_lines = inner_area[1].height.saturating_sub(1);
        let max_scroll = info_text_line_count.saturating_sub(visible_lines as usize);
        let scroll_range = info_text_line_count
            .saturating_sub(visible_lines as usize)
            .max(1);

        let scroll_position = &mut self.info_panel_scroll_position;
        // Clamp the scroll position and update it at the same time so it will not go infinately
        if *scroll_position > max_scroll {
            *scroll_position = max_scroll;
        }

        let mut info_scrollbar_state = self
            .info_panel_scrollbarstate
            .position(*scroll_position)
            .content_length(scroll_range)
            .viewport_content_length(visible_lines as usize);

        // Render stateful widget of the table scrollbar
        StatefulWidget::render(
            info_scrollbar,
            chunks[1].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut info_scrollbar_state,
        );

        let info_paragraph = Paragraph::new(tab_content)
            .fg(Color::White)
            .scroll((*scroll_position as u16, 0));

        info_paragraph.render(inner_area[1], buf);

        // Show help popup
        if self.show_help {
            App::render_help_popup(self, area, buf);
        }

        // Show server info popup
        if self.show_server_info {
            App::render_info_popup(self, area, buf);
        }

        // Show error popup
        if self.show_error_popup {
            App::render_error_popup(self, area, buf);
        }

        // Add task popup
        if self.show_add_task_from_url {
            App::render_add_task_popup(self, area, buf);
        }

        // Show file picker
        if self.show_add_task_from_file {
            App::render_add_task_from_file_popup(self, area, buf);
        }
    }
}

/// Add UI related associated functions
impl App {
    /// Redraws the entire user interface.
    pub fn redraw<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> color_eyre::Result<()> {
        terminal.draw(|frame| {
            // Delegate to render method on `App`
            frame.render_widget(self, frame.area());
        })?;
        Ok(())
    }

    /// Show server info window
    pub fn render_info_popup(&mut self, area: Rect, buf: &mut Buffer) {
        let server_config = self.load_config_file();
        let server_config_path = AppConfig::config_path()
            .into_os_string()
            .into_string()
            .unwrap();
        if let Some(cfg) = &self.dsconfig {
            let default_dest = cfg.default_destination.as_deref().unwrap_or("<none>");
            let emule_dest = cfg.emule_default_destination.as_deref().unwrap_or("<none>");

            let server_info_lines = vec![
                Line::from(format!(" Config file path    : {server_config_path}")),
                Line::from(format!(
                    " Server address      : {}",
                    server_config.server_url
                )),
                Line::from(format!(" Server port         : {}", server_config.port)),
                Line::from(format!(" User name           : {}", server_config.username)),
                Line::from(format!(
                    " Refresh interval    : {} (in seconds)",
                    server_config.refresh_interval
                )),
                Line::from(""),
                Line::from(format!(" BT Max download     : {} KB/s", cfg.bt_max_upload)),
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
                Line::from(format!(" Default Dest.       : {default_dest}")),
                Line::from(format!(" eMule Default Dest. : {emule_dest}")),
            ];

            create_popup(
                " Server Info ",
                server_info_lines,
                &mut self.popup_scroll_position,
                false,
                area,
                buf,
            );
        }
    }

    /// Show help window
    pub fn render_help_popup(&mut self, area: Rect, buf: &mut Buffer) {
        let help_text_lines = vec![
            Line::from("Shortcuts:"),
            Line::from(""),
            Line::from(" a: Add a new task by URL from clipboard"),
            Line::from(" d: Delete task"),
            Line::from(" h: Show previous tab"),
            Line::from(" i: Show server info"),
            Line::from(" j: Move down / Scroll down in popups"),
            Line::from(" k: Move up / Scrull up in popups"),
            Line::from(" l: Show next tab"),
            Line::from(" q: Quit or close any open popup"),
            Line::from(" p: Pause / Resume task"),
            Line::from(" r: Manually refresh tasks"),
            Line::from(" ↓: Scroll down in the info panel"),
            Line::from(" ?: Show this help"),
        ];

        create_popup(
            " Help ",
            help_text_lines,
            &mut self.popup_scroll_position,
            false,
            area,
            buf,
        );
    }

    /// Show add task window
    pub fn render_add_task_popup(&mut self, area: Rect, buf: &mut Buffer) {
        let clipboard_text = get_clipboard();
        let add_task_text_lines = vec![Line::from(clipboard_text)];

        create_popup(
            " URL copied, press <Enter> to add task or close with <q> or ESC ",
            add_task_text_lines,
            &mut self.popup_scroll_position,
            false,
            area,
            buf,
        );
    }

    /// Show File picker
    pub fn render_add_task_from_file_popup(&mut self, area: Rect, buf: &mut Buffer) {
        create_filepicker_popup(
            " File Picker ",
            &mut self.selected_row_filepicker,
            &mut self.popup_scroll_position,
            area,
            buf,
        );
    }

    /// Show error popup
    pub fn render_error_popup(&mut self, area: Rect, buf: &mut Buffer) {
        let error_text = Line::from(self.error_message.clone().unwrap_or_default());
        create_popup(
            " Error ",
            vec![error_text],
            &mut self.popup_scroll_position,
            true,
            area,
            buf,
        );
    }
}

/// Helper function for creating relative sized popups
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    // Raw size of the terminal window
    let raw_w = area.width.saturating_mul(percent_x) / 100;
    let raw_h = area.height.saturating_mul(percent_y) / 100;

    // Enforce minimum of 3 rows and columns (1 border + 1 content + 1 border)
    let w = raw_w.max(3).min(area.width);
    let h = raw_h.max(3).min(area.height);

    // Center it in the rendered area
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;

    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// Create popup
fn create_popup(
    popup_title: &str,
    popup_text_lines: Vec<Line>,
    scroll_position: &mut usize,
    is_error_popup: bool,
    area: Rect,
    buf: &mut Buffer,
) {
    // Count the lines in the text for the scrollbar
    let popup_text_line_count = popup_text_lines.len();

    // Create a Text from the lines
    let popup_text = Text::from(popup_text_lines);

    // Create a popup Rect
    let popup_rect = centered_rect(60, 40, area);

    // Remove anything from the popup's background
    Clear.render(popup_rect, buf);

    // Visible lines and Max scroll value for the scrollbar
    let visible_lines = popup_rect.height.saturating_sub(2);
    let max_scroll = popup_text_line_count.saturating_sub(visible_lines as usize);

    // Clamp the scroll position and update it at the same time so it will not go infinately
    if *scroll_position > max_scroll {
        *scroll_position = max_scroll;
    }

    let popup_block = Block::bordered()
        .title(popup_title)
        .title_bottom(
            Line::from(" Scroll down/up if needed with <j> and <k>, close with <q> ")
                .alignment(Alignment::Center),
        )
        .title_alignment(Alignment::Left)
        .border_type(BorderType::Rounded);

    // Display the text
    let fg_color: Color = if is_error_popup {
        Color::LightRed
    } else {
        Color::White
    };

    let popup_paragraph = Paragraph::new(popup_text)
        .block(popup_block)
        .scroll((*scroll_position as u16, 0))
        .fg(fg_color)
        .bg(Color::Black);

    // Render the popup
    popup_paragraph.render(popup_rect, buf);

    // Render stateful widget of the table scrollbar
    let popup_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let scroll_range = popup_text_line_count
        .saturating_sub(visible_lines as usize)
        .max(1);
    let mut popup_scrollbar_state = ScrollbarState::new(scroll_range)
        .position(*scroll_position)
        .viewport_content_length(visible_lines as usize);

    StatefulWidget::render(
        popup_scrollbar,
        popup_rect.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        buf,
        &mut popup_scrollbar_state,
    );
}

/// Create filepicker popup
fn create_filepicker_popup(
    popup_title: &str,
    selected_row: &mut TableState,
    scroll_position: &mut usize,
    area: Rect,
    buf: &mut Buffer,
) {
    // Create a popup Rect
    let popup_rect = centered_rect(50, 40, area);

    let dir = get_files();
    let rows: Vec<Row> = dir
        .iter()
        .map(|dir| {
            let color = match dir.filetype.as_str() {
                "torrent" => Color::Green,
                _ => Color::Gray,
            };
            Row::new(vec![
                Cell::from(dir.filename.clone()),
                Cell::from(dir.filetype.clone()),
            ])
            .style(Style::default().fg(color))
        })
        .collect();

    // Auto-select the first row if nothing is selected
    if selected_row.selected().is_none() && !rows.is_empty() {
        selected_row.select(Some(0));
    }
    // Remove anything from the popup's background
    Clear.render(popup_rect, buf);

    // Visible lines and Max scroll value for the scrollbar
    let visible_lines = popup_rect.height.saturating_sub(2);
    let max_scroll = rows.len().saturating_sub(visible_lines as usize);

    // Clamp the scroll position and update it at the same time so it will not go infinately
    if *scroll_position > max_scroll {
        *scroll_position = max_scroll;
    }

    let table_block = Block::bordered()
        .title(popup_title)
        .title_alignment(Alignment::Center)
        .border_type(BorderType::Rounded);

    // Row selection
    let table_row_index = selected_row.selected().unwrap_or(0);
    let table_state = selected_row;

    // Create the table with content and styling
    let header_titles = vec!["Filename", "Type"];
    let header =
        Row::new(header_titles).style(Style::default().fg(Color::White).bg(Color::DarkGray).bold());

    let widths = [
        Constraint::Percentage(80), // Filename
        Constraint::Percentage(20), // Extension
    ];

    // Add a scrollbar
    let table_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut table_scrollbar_state = ScrollbarState::new(rows.len())
        .position(table_row_index)
        .viewport_content_length(1);

    let table = Table::new(rows, widths)
        .block(table_block)
        .header(header)
        .row_highlight_style(Style::new().reversed())
        .column_spacing(1);

    // Render stateful widget of the table
    StatefulWidget::render(table, popup_rect, buf, table_state);
    // Render stateful widget of the table scrollbar
    StatefulWidget::render(
        table_scrollbar,
        popup_rect.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        buf,
        &mut table_scrollbar_state,
    );
}
