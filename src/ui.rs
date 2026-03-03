use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::ProxyType;
use crate::App;

pub struct Ui;

impl Ui {
    pub fn draw(f: &mut Frame, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)])
            .split(f.area());

        Self::draw_header(f, chunks[0], app);
        Self::draw_main(f, chunks[1], app);
        Self::draw_footer(f, chunks[2]);
    }

    fn draw_header(f: &mut Frame, area: Rect, app: &App) {
        let mode_color = match app.mode.as_str() {
            "rule" => Color::Green,
            "global" => Color::Yellow,
            "direct" => Color::Blue,
            _ => Color::Gray,
        };

        // Mode description
        let mode_hint = match app.mode.as_str() {
            "rule" => " (Rule-based proxy)",
            "global" => " (Use GLOBAL group)",
            "direct" => " (Direct connection)",
            _ => "",
        };

        let header = Paragraph::new(vec![
            Line::from(vec![
                // Left: shortcuts hint
                Span::styled(" r:", Style::default().fg(Color::Green)),
                Span::styled("Rule ", Style::default().fg(Color::Gray)),
                Span::styled(" g:", Style::default().fg(Color::Yellow)),
                Span::styled("Global ", Style::default().fg(Color::Gray)),
                Span::styled(" d:", Style::default().fg(Color::Blue)),
                Span::styled("Direct ", Style::default().fg(Color::Gray)),
                Span::raw(" | "),
                // Center: mode display
                Span::raw(" Mode: "),
                Span::styled(
                    app.mode.to_uppercase(),
                    Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    mode_hint,
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(" "),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
        )
        .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn draw_main(f: &mut Frame, area: Rect, app: &App) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        Self::draw_groups(f, chunks[0], app);
        Self::draw_proxies(f, chunks[1], app);
    }

    fn draw_groups(f: &mut Frame, area: Rect, app: &App) {
        // Highlight GLOBAL group in global mode
        let is_global_mode = app.mode == "global";

        let items: Vec<ListItem> = app
            .proxies
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                let proxy_type = app.proxy_types.get(i).copied().unwrap_or(ProxyType::Selector);
                let is_selected = i == app.selected_group;
                let is_last_updated = app.last_updated_group == Some(i);
                let is_global_group = is_global_mode && name == "GLOBAL";

                // Set color based on state
                let (name_color, marker_color) = if is_selected {
                    (Color::Green, Color::Green)
                } else if is_global_group {
                    // GLOBAL group highlighted in cyan in global mode
                    (Color::Cyan, Color::Cyan)
                } else if is_last_updated {
                    (Color::Yellow, Color::Yellow)
                } else {
                    // Selector type
                    (Color::White, Color::Cyan)
                };

                let style = Style::default().fg(name_color);
                let marker_style = Style::default().fg(marker_color);

                // Add [ACTIVE] mark for GLOBAL group in global mode
                let mark = if is_global_group {
                    "[ACTIVE] "
                } else if is_last_updated {
                    "★ "
                } else {
                    ""
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(mark.to_string(), Style::default().fg(Color::Cyan)),
                        Span::styled(proxy_type.marker(), marker_style.add_modifier(Modifier::BOLD)),
                        Span::raw(" "),
                        Span::styled(name.clone(), if is_selected { style.add_modifier(Modifier::BOLD) } else { style }),
                    ])
                ])
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Groups [S=Selectable] ")
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(list, area);
    }

    fn draw_proxies(f: &mut Frame, area: Rect, app: &App) {
        if let Some((group_name, proxies)) = app.proxies.get(app.selected_group) {
            // Get current group selected node and delays
            let current_proxy = app.current_proxies.get(app.selected_group);
            let delays = app.proxy_delays.get(app.selected_group);

            // Calculate available height (minus borders)
            let available_height = area.height.saturating_sub(2) as usize;

            let items: Vec<ListItem> = proxies
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let is_selected = i == app.selected_proxy;
                    let is_current = current_proxy.map_or(false, |cp| cp == name);

                    // Get delay info
                    let delay = delays.and_then(|d| d.get(i)).copied().flatten();

                    // Set color based on delay
                    let delay_color = delay.map_or(Color::DarkGray, |d| {
                        if d < 200 {
                            Color::Green
                        } else if d < 500 {
                            Color::Yellow
                        } else {
                            Color::Red
                        }
                    });

                    let delay_str = delay.map_or_else(
                        || " -- ".to_string(),
                        |d| format!(" {:3}ms", d),
                    );

                    let style = if is_selected {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else if is_current {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    // Add [✓] mark for currently selected node
                    let prefix = if is_current { "[✓]" } else { "   " };

                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(prefix, Style::default().fg(Color::Cyan)),
                            Span::styled(format!(" {} ", name), style),
                            Span::styled(delay_str, Style::default().fg(delay_color)),
                        ])
                    ])
                })
                .collect();

            // Calculate scroll offset to ensure selected item is visible
            let offset = if items.len() <= available_height {
                0
            } else if app.selected_proxy >= available_height - 1 {
                app.selected_proxy.saturating_sub(available_height) + 1
            } else {
                0
            };

            let mut list_state = ListState::default().with_offset(offset).with_selected(Some(app.selected_proxy));

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan))
                        .title(format!(" {} ", group_name))
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                );

            f.render_stateful_widget(list, area, &mut list_state);
        } else {
            let empty = Paragraph::new("No proxies available")
                .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
        }
    }

    fn draw_footer(f: &mut Frame, area: Rect) {
        let help_text = vec![
            Line::from(vec![
                Span::styled(" q ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw(" quit "),
                Span::raw(" | "),
                Span::styled(" Tab ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" next group "),
                Span::raw(" | "),
                Span::styled(" j/k ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" up/down node "),
                Span::raw(" | "),
                Span::styled(" s ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" switch "),
                Span::raw(" | "),
                Span::styled(" f ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw(" refresh delays "),
            ]),
            Line::from(vec![
                Span::styled(" r ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" rule "),
                Span::raw(" | "),
                Span::styled(" g ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" global "),
                Span::raw(" | "),
                Span::styled(" d ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::raw(" direct mode "),
            ]),
        ];

        let footer = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(footer, area);
    }
}
