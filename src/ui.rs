use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
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

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" Mihomo TUI - Mode: "),
                Span::styled(
                    app.mode.to_uppercase(),
                    Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
                ),
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
        let items: Vec<ListItem> = app
            .proxies
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                let proxy_type = app.proxy_types.get(i).copied().unwrap_or(ProxyType::Selector);
                let is_selected = i == app.selected_group;

                // 根据类型设置颜色
                let (name_color, marker_color) = if is_selected {
                    (Color::Green, Color::Green)
                } else {
                    match proxy_type {
                        ProxyType::Selector => (Color::White, Color::Cyan),
                        ProxyType::UrlTest => (Color::Gray, Color::Yellow),
                        ProxyType::Fallback => (Color::Gray, Color::Magenta),
                    }
                };

                let style = Style::default().fg(name_color);
                let marker_style = Style::default().fg(marker_color);

                ListItem::new(vec![
                    Line::from(vec![
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
                    .title(" Groups [S=可切换 A=自动 F=故障转移] ")
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
            // 获取当前组选中的节点名称
            let current_proxy = app.current_proxies.get(app.selected_group);

            let items: Vec<ListItem> = proxies
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let is_selected = i == app.selected_proxy;
                    let is_current = current_proxy.map_or(false, |cp| cp == name);

                    let style = if is_selected {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else if is_current {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    // 当前选中的节点前面添加 [✓] 标识
                    let display_name = if is_current {
                        format!("[✓] {}", name)
                    } else {
                        format!("    {}", name)
                    };

                    ListItem::new(vec![Line::styled(display_name, style)])
                })
                .collect();

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

            f.render_widget(list, area);
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
                Span::styled(" ↑↓ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" select group "),
                Span::raw(" | "),
                Span::styled(" ←→ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" select node "),
                Span::raw(" | "),
                Span::styled(" s ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" switch node "),
            ]),
            Line::from(vec![
                Span::styled(" r ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" rule mode "),
                Span::raw(" | "),
                Span::styled(" g ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" global mode "),
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
