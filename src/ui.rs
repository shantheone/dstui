use crate::app::{ActivePanel, App, SPINNER_FRAMES, SortColumn, SortOrder};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, StatefulWidget, Table, TableState, Tabs, Widget, WidgetRef, Wrap,
    },
};
use syno_download_station::entities::{Task, TaskStatus};

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chunks =
            Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).split(area);

        // Compute total speeds
        let (total_down, total_up) = self.tasks.iter().fold((0u64, 0u64), |acc, task| {
            task.additional
                .as_ref()
                .and_then(|a| a.transfer.as_ref())
                .map(|t| (acc.0 + t.speed_download, acc.1 + t.speed_upload))
                .unwrap_or(acc)
        });

        // Build headers with sort indicator
        let sort_indicator = |col: &SortColumn| -> &str {
            if &self.sort_column == col {
                match self.sort_order {
                    SortOrder::Ascending => " ▲",
                    SortOrder::Descending => " ▼",
                }
            } else {
                ""
            }
        };

        let spinner = SPINNER_FRAMES[self.spinner_frame];
        let auto_refresh_title;
        let spinner_title;
        let stats_title;
        let title_text = if self.loading {
            spinner_title = format!(" {} DownloadStation TUI Client ", spinner);
            spinner_title.as_str().bold().light_cyan()
        } else if total_down > 0 || total_up > 0 {
            stats_title = format!(
                " DownloadStation TUI Client  ↓ {}  ↑ {} ",
                format_speed(total_down),
                format_speed(total_up),
            );
            stats_title.as_str().bold().light_cyan()
        } else {
            match self.refresh_interval {
                Some(ticks) => {
                    auto_refresh_title = format!(
                        " DownloadStation TUI Client - [Auto-refresh: {}s] ",
                        ticks / 30
                    );
                    auto_refresh_title.as_str().bold().light_cyan()
                }
                None => " DownloadStation TUI Client - [Auto-refresh: off] "
                    .bold()
                    .light_cyan(),
            }
        };
        let table_block = Block::bordered()
            .title(title_text)
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(match self.active_panel {
                ActivePanel::Tasks => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            });

        let task_row_index = self.selected_task_index();

        let header = Row::new(vec![
            Cell::from(format!("Name{}", sort_indicator(&SortColumn::Name)))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!("Size{}", sort_indicator(&SortColumn::Size)))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!(
                "Downloaded{}",
                sort_indicator(&SortColumn::Downloaded)
            ))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!("Uploaded{}", sort_indicator(&SortColumn::Uploaded)))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!("Progress{}", sort_indicator(&SortColumn::Progress)))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!(
                "Up Speed{}",
                sort_indicator(&SortColumn::UploadSpeed)
            ))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!(
                "Down Speed{}",
                sort_indicator(&SortColumn::DownloadSpeed)
            ))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!("Ratio{}", sort_indicator(&SortColumn::Ratio)))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
            Cell::from(format!("Status{}", sort_indicator(&SortColumn::Status)))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray).bold()),
        ]);

        let rows: Vec<Row> = self
            .sorted_tasks()
            .iter()
            .map(|task| {
                let progress = task.calculate_progress();

                let status_style = match task.status {
                    TaskStatus::Downloading => Style::default().fg(Color::Green),
                    TaskStatus::Seeding => Style::default().fg(Color::Cyan),
                    TaskStatus::Waiting => Style::default().fg(Color::Yellow),
                    TaskStatus::Paused => Style::default().fg(Color::DarkGray),
                    TaskStatus::Finishing => Style::default().fg(Color::LightGreen),
                    TaskStatus::Finished => Style::default().fg(Color::DarkGray),
                    TaskStatus::HashChecking => Style::default().fg(Color::Yellow),
                    TaskStatus::Error => Style::default().fg(Color::Red),
                    _ => Style::default().fg(Color::White),
                };

                let row_style = match task.status {
                    TaskStatus::Paused | TaskStatus::Finished => {
                        Style::default().fg(Color::DarkGray)
                    }
                    _ => Style::default(),
                };

                Row::new(vec![
                    Cell::from(truncate(&task.title, 40)),
                    Cell::from(task.calculate_size()),
                    Cell::from(
                        task.additional
                            .as_ref()
                            .and_then(|a| a.transfer.as_ref())
                            .map(|t| format!("{:.1} MB", t.size_downloaded as f64 / 1_048_576.0))
                            .unwrap_or_default(),
                    ),
                    Cell::from(
                        task.additional
                            .as_ref()
                            .and_then(|a| a.transfer.as_ref())
                            .map(|t| format!("{:.1} MB", t.size_uploaded as f64 / 1_048_576.0))
                            .unwrap_or_default(),
                    ),
                    Cell::from(Line::from(render_progress_bar(progress, 8))),
                    Cell::from(
                        task.additional
                            .as_ref()
                            .and_then(|a| a.transfer.as_ref())
                            .map(|t| format_speed(t.speed_upload))
                            .unwrap_or_default(),
                    ),
                    Cell::from(
                        task.additional
                            .as_ref()
                            .and_then(|a| a.transfer.as_ref())
                            .map(|t| format_speed(t.speed_download))
                            .unwrap_or_default(),
                    ),
                    Cell::from(format!("{:.2}", task.calculate_ratio())),
                    Cell::from(format!("{:?}", task.status)).style(status_style),
                ])
                .style(row_style)
            })
            .collect();
        let row_count = rows.len();

        let widths = [
            Constraint::Percentage(22), // Name
            Constraint::Percentage(8),  // Size
            Constraint::Percentage(8),  // Downloaded
            Constraint::Percentage(8),  // Uploaded
            Constraint::Percentage(14), // Progress (wider for bar)
            Constraint::Percentage(10), // Upload Speed
            Constraint::Percentage(10), // Download Speed
            Constraint::Percentage(5),  // Ratio
            Constraint::Percentage(10), // Status
        ];

        let table_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut table_scrollbar_state = ScrollbarState::new(row_count)
            .position(task_row_index.unwrap_or(0))
            .viewport_content_length(1);

        let table = Table::new(rows, widths)
            .block(table_block)
            .header(header)
            .row_highlight_style(Style::new().reversed())
            .column_spacing(1);

        StatefulWidget::render(table, chunks[0], buf, &mut self.selected_task);
        StatefulWidget::render(
            table_scrollbar,
            chunks[0].inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut table_scrollbar_state,
        );

        // Info panel
        let info_block = Block::bordered()
            .title(Line::from(vec![
                Span::styled(" Info ", Style::default().bold()),
                Span::styled(
                    format!("— {} ", self.tabs[self.selected_tab]),
                    Style::default().fg(Color::Yellow).bold(),
                ),
            ]))
            .title_alignment(Alignment::Center)
            .title_bottom(" Tab to switch panels ")
            .border_type(BorderType::Rounded)
            .border_style(match self.active_panel {
                ActivePanel::Info => Style::default().fg(Color::Yellow),
                _ => Style::default(),
            });
        info_block.render(chunks[1], buf);

        let inner_area = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(chunks[1]);

        let tab_titles = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if i == self.selected_tab {
                    Line::from(Span::styled(
                        format!(" {} ", t),
                        Style::default().fg(Color::Black).bg(Color::Yellow).bold(),
                    ))
                } else {
                    Line::from(Span::styled(
                        format!(" {} ", t),
                        Style::default().fg(Color::White),
                    ))
                }
            })
            .collect::<Vec<_>>();

        let tabs = Tabs::new(tab_titles)
            .select(self.selected_tab)
            .divider("│")
            .bg(Color::DarkGray);
        tabs.render(inner_area[0], buf);

        // Render tab content for the selected task
        if let Some(real_idx) = self.selected_task_in_sorted()
            && let Some(task) = self.tasks.get(real_idx)
        {
            match self.selected_tab {
                0 => render_general_tab(task, inner_area[1], buf),
                1 => render_transfer_tab(task, inner_area[1], buf),
                2 => {
                    self.tracker_inner_height = inner_area[1].height as usize;
                    render_tracker_tab(
                        task,
                        inner_area[1],
                        buf,
                        self.tracker_scroll,
                        self.tracker_count,
                    );
                }
                3 => {
                    self.peer_inner_height = inner_area[1].height as usize;
                    render_peers_tab(task, inner_area[1], buf, self.peer_scroll, self.peer_count);
                }
                4 => {
                    self.file_inner_height = inner_area[1].height as usize;
                    render_files_tab(task, inner_area[1], buf, self.file_scroll, self.file_count);
                }
                _ => {}
            }
        }

        // File picker
        if let Some(explorer) = &self.file_explorer {
            let picker_area = area.centered(Constraint::Percentage(70), Constraint::Percentage(80));
            Clear.render(picker_area, buf);
            explorer.widget().render_ref(picker_area, buf);
        }

        // URL input field
        if let Some(input) = &self.url_input {
            let input_area = Rect {
                x: area.x,
                y: area.y + area.height - 3,
                width: area.width,
                height: 3,
            };
            Clear.render(input_area, buf);

            let input_block = Block::bordered()
                .title(" Add URL (Enter to confirm · Esc to cancel) ")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Yellow));

            let inner = input_block.inner(input_area);
            input_block.render(input_area, buf);

            Paragraph::new(input.value()).render(inner, buf);

            // Store cursor position: inner area start + cursor offset within input
            self.url_input_cursor_pos = Some((inner.x + input.visual_cursor() as u16, inner.y));
        } else {
            self.url_input_cursor_pos = None;
        }

        // Popup
        if let Some(popup) = &self.popup {
            let popup_area = area.centered(Constraint::Percentage(60), Constraint::Percentage(60));
            Clear.render(popup_area, buf);

            let border_style = if popup.error {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Yellow)
            };

            let title = if popup.error { " Error " } else { " Help " };

            let lines: Vec<Line> = popup
                .lines
                .iter()
                .map(|l| {
                    Line::from(Span::styled(
                        l.clone(),
                        if popup.error {
                            Style::default().fg(Color::LightRed)
                        } else {
                            Style::default().fg(Color::LightYellow)
                        },
                    ))
                })
                .collect();

            let block = Block::bordered()
                .title(title)
                .title_bottom(" j / k to scroll · Esc to close ")
                .border_type(BorderType::Rounded)
                .border_style(border_style);

            // Split inner area to leave room for scrollbar
            let inner = block.inner(popup_area);
            let chunks =
                Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).split(inner);

            // Store visible height for scroll clamping
            self.popup_inner_height = chunks[0].height as usize;

            block.render(popup_area, buf);

            Paragraph::new(lines.clone())
                .scroll((popup.scroll as u16, 0))
                .wrap(Wrap { trim: true })
                .render(chunks[0], buf);

            let area_height = chunks[0].height as usize;
            let mut scrollbar_state =
                ScrollbarState::new(popup.lines.len().saturating_sub(area_height))
                    .position(popup.scroll);
            StatefulWidget::render(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓")),
                chunks[1],
                buf,
                &mut scrollbar_state,
            );
        }
    }
}

fn render_general_tab(task: &Task, area: Rect, buf: &mut Buffer) {
    let destination = task
        .additional
        .as_ref()
        .and_then(|a| a.detail.as_ref())
        .map(|d| d.destination.clone())
        .unwrap_or_else(|| "N/A".to_string());

    let created_time = task
        .additional
        .as_ref()
        .and_then(|a| a.detail.as_ref())
        .map(|d| d.created_time.to_string())
        .unwrap_or_else(|| "N/A".to_string());

    let text = vec![
        Line::from(vec![
            Span::styled("Title:       ", Style::default().fg(Color::LightCyan)),
            Span::styled(task.title.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("ID:          ", Style::default().fg(Color::LightCyan)),
            Span::styled(task.id.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Destination: ", Style::default().fg(Color::LightCyan)),
            Span::styled(destination, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Created:     ", Style::default().fg(Color::LightCyan)),
            Span::styled(created_time, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("User:        ", Style::default().fg(Color::LightCyan)),
            Span::styled(task.username.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Size:        ", Style::default().fg(Color::LightCyan)),
            Span::styled(task.calculate_size(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Status:      ", Style::default().fg(Color::LightCyan)),
            Span::styled(
                format!("{:?}", task.status),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("ETA:         ", Style::default().fg(Color::LightCyan)),
            Span::styled(
                task.calculate_time_left(),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Ratio:       ", Style::default().fg(Color::LightCyan)),
            Span::styled(
                format!("{:.2}", task.calculate_ratio()),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    Paragraph::new(text).render(area, buf);
}

fn render_transfer_tab(task: &Task, area: Rect, buf: &mut Buffer) {
    let (downloaded, uploaded, speed_up, speed_down) = task
        .additional
        .as_ref()
        .and_then(|a| a.transfer.as_ref())
        .map(|t| {
            (
                format!("{:.1} MB", t.size_downloaded as f64 / 1_048_576.0),
                format!("{:.1} MB", t.size_uploaded as f64 / 1_048_576.0),
                format_speed(t.speed_upload),
                format_speed(t.speed_download),
            )
        })
        .unwrap_or_else(|| ("N/A".into(), "N/A".into(), "N/A".into(), "N/A".into()));

    let progress = task.calculate_progress();

    let text = vec![
        Line::from(vec![
            Span::styled("Downloaded:  ", Style::default().fg(Color::LightCyan)),
            Span::styled(downloaded, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Uploaded:    ", Style::default().fg(Color::LightCyan)),
            Span::styled(uploaded, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Up Speed:    ", Style::default().fg(Color::LightCyan)),
            Span::styled(speed_up, Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Down Speed:  ", Style::default().fg(Color::LightCyan)),
            Span::styled(speed_down, Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Progress:    ", Style::default().fg(Color::LightCyan)),
            render_progress_bar(progress, 20),
        ]),
        Line::from(vec![
            Span::styled("Ratio:       ", Style::default().fg(Color::LightCyan)),
            Span::styled(
                format!("{:.2}", task.calculate_ratio()),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("ETA:         ", Style::default().fg(Color::LightCyan)),
            Span::styled(
                task.calculate_time_left(),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    Paragraph::new(text).render(area, buf);
}

fn render_tracker_tab(task: &Task, area: Rect, buf: &mut Buffer, scroll: usize, count: usize) {
    let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).split(area);

    let rows: Vec<Row> = task
        .additional
        .as_ref()
        .and_then(|a| a.tracker.as_ref())
        .map(|trackers| {
            trackers
                .iter()
                .map(|t| {
                    Row::new(vec![
                        Cell::from(t.url.clone()).style(Style::default().fg(Color::White)),
                        Cell::from(format!("{:?}", t.status))
                            .style(Style::default().fg(Color::Yellow)),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();

    let header = Row::new(vec![
        Cell::from("URL").style(Style::default().fg(Color::Yellow).underlined()),
        Cell::from("Status").style(Style::default().fg(Color::Yellow).underlined()),
    ]);

    let widths = [Constraint::Percentage(80), Constraint::Percentage(20)];

    let area_height = chunks[0].height as usize;

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    let mut state = TableState::default().with_offset(scroll);
    StatefulWidget::render(table, chunks[0], buf, &mut state);

    let mut scrollbar_state =
        ScrollbarState::new(count.saturating_sub(area_height)).position(scroll);
    StatefulWidget::render(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        chunks[1],
        buf,
        &mut scrollbar_state,
    );
}

fn render_peers_tab(task: &Task, area: Rect, buf: &mut Buffer, scroll: usize, count: usize) {
    let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).split(area);

    let rows: Vec<Row> = task
        .additional
        .as_ref()
        .and_then(|a| a.peer.as_ref())
        .map(|peers| {
            peers
                .iter()
                .map(|p| {
                    Row::new(vec![
                        Cell::from(p.address.clone()).style(Style::default().fg(Color::White)),
                        Cell::from(format_speed(p.speed_download))
                            .style(Style::default().fg(Color::Green)),
                        Cell::from(format_speed(p.speed_upload))
                            .style(Style::default().fg(Color::Green)),
                        Cell::from(p.agent.clone()).style(Style::default().fg(Color::Yellow)),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();

    let header = Row::new(vec![
        Cell::from("Address").style(Style::default().fg(Color::Yellow).underlined()),
        Cell::from("Down").style(Style::default().fg(Color::Yellow).underlined()),
        Cell::from("Up").style(Style::default().fg(Color::Yellow).underlined()),
        Cell::from("Client").style(Style::default().fg(Color::Yellow).underlined()),
    ]);

    let widths = [
        Constraint::Percentage(35),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(35),
    ];

    let area_height = chunks[0].height as usize;

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    let mut state = TableState::default().with_offset(scroll);
    StatefulWidget::render(table, chunks[0], buf, &mut state);

    let mut scrollbar_state =
        ScrollbarState::new(count.saturating_sub(area_height)).position(scroll);
    StatefulWidget::render(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        chunks[1],
        buf,
        &mut scrollbar_state,
    );
}

fn render_files_tab(task: &Task, area: Rect, buf: &mut Buffer, scroll: usize, count: usize) {
    let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).split(area);

    let rows: Vec<Row> = task
        .additional
        .as_ref()
        .and_then(|a| a.file.as_ref())
        .map(|files| {
            files
                .iter()
                .map(|f| {
                    let progress = if f.size > 0 {
                        format!("{:.1}%", f.size_downloaded as f64 / f.size as f64 * 100.0)
                    } else {
                        "N/A".to_string()
                    };
                    Row::new(vec![
                        Cell::from(f.filename.clone()).style(Style::default().fg(Color::White)),
                        Cell::from(progress).style(Style::default().fg(Color::Yellow)),
                    ])
                })
                .collect()
        })
        .unwrap_or_default();

    let header = Row::new(vec![
        Cell::from("Filename").style(Style::default().fg(Color::Yellow).underlined()),
        Cell::from("Progress").style(Style::default().fg(Color::Yellow).underlined()),
    ]);

    let widths = [Constraint::Percentage(90), Constraint::Percentage(10)];

    let area_height = chunks[0].height as usize;

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    let mut state = TableState::default().with_offset(scroll);
    StatefulWidget::render(table, chunks[0], buf, &mut state);

    let mut scrollbar_state =
        ScrollbarState::new(count.saturating_sub(area_height)).position(scroll);
    StatefulWidget::render(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        chunks[1],
        buf,
        &mut scrollbar_state,
    );
}

fn render_progress_bar(progress: f64, width: usize) -> Span<'static> {
    let filled = (progress / 100.0 * width as f64).round() as usize;
    let label = format!("{:>3.0}%", progress);

    // Build the bar with the label centered inside it
    let bar: String = (0..width)
        .map(|i| {
            let label_start = (width.saturating_sub(label.len())) / 2;
            let label_idx = i.wrapping_sub(label_start);
            if label_idx < label.len() {
                label.chars().nth(label_idx).unwrap_or(' ')
            } else if i < filled {
                '█'
            } else {
                '░'
            }
        })
        .collect();

    let color = if progress >= 100.0 {
        Color::Green
    } else if progress >= 50.0 {
        Color::Yellow
    } else {
        Color::Red
    };
    Span::styled(bar, Style::default().fg(color))
}

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec >= 1_048_576 {
        format!("{:.1} MB/s", bytes_per_sec as f64 / 1_048_576.0)
    } else if bytes_per_sec >= 1024 {
        format!("{:.0} KB/s", bytes_per_sec as f64 / 1024.0)
    } else if bytes_per_sec > 0 {
        format!("{} B/s", bytes_per_sec)
    } else {
        String::new() // show nothing when idle
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars - 1).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}
