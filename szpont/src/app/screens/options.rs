use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

use crate::app::App;
use crate::core::format_tokens;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header_area, body_area, separator_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            crate::app::widgets::logo::compact(),
            Span::raw("  "),
            Span::styled("options", Style::new().fg(Color::Cyan).bold()),
        ])),
        header_area,
    );
    let block = Block::bordered()
        .border_style(Style::new().fg(Color::Cyan))
        .title(" options ")
        .title_style(Style::new().fg(Color::Cyan).bold());
    let inner = block.inner(body_area);
    frame.render_widget(block, body_area);
    if app.options.is_empty() {
        frame.render_widget(
            Paragraph::new("no models discovered yet — options appear after the first scan")
                .style(Style::new().fg(Color::DarkGray))
                .centered(),
            inner,
        );
    } else {
        let items: Vec<ListItem> = app
            .options
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let window = match option.window {
                    Some(w) => format_tokens(w),
                    None => "not set".to_string(),
                };
                let source = if option.overridden {
                    "set by you"
                } else if option.window.is_some() {
                    "reported by the tool"
                } else {
                    ""
                };
                let mut style = Style::new();
                if i == app.options_selected {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                ListItem::new(Line::from(vec![
                    Span::raw(format!(" {:<28}", crate::core::truncate(&option.model, 28))),
                    Span::styled("context window: ", Style::new().fg(Color::DarkGray)),
                    Span::styled(format!("{window:<10}"), Style::new().fg(Color::Cyan)),
                    Span::styled(source, Style::new().fg(Color::DarkGray)),
                ]))
                .style(style)
            })
            .collect();
        frame.render_widget(List::new(items), inner);
    }
    frame.render_widget(
        Paragraph::new("─".repeat(frame.area().width as usize))
            .style(Style::new().fg(Color::DarkGray)),
        separator_area,
    );
    let footer = if let Some(input) = &app.window_input {
        Line::from(vec![
            Span::styled(
                format!(
                    " context window for {}: {}",
                    input.model,
                    input.editor.display_with_cursor()
                ),
                Style::new().fg(Color::Yellow),
            ),
            Span::raw("   "),
            Span::styled("enter", Style::new().fg(Color::Cyan).bold()),
            Span::styled(" save  ·  ", Style::new().fg(Color::DarkGray)),
            Span::styled("esc", Style::new().fg(Color::Cyan).bold()),
            Span::styled(" cancel", Style::new().fg(Color::Gray)),
        ])
    } else {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("↑/↓", Style::new().fg(Color::Cyan).bold()),
            Span::styled(" move  ·  ", Style::new().fg(Color::DarkGray)),
            Span::styled("enter", Style::new().fg(Color::Cyan).bold()),
            Span::styled(" set context window  ·  ", Style::new().fg(Color::Gray)),
            Span::styled("esc", Style::new().fg(Color::Cyan).bold()),
            Span::styled(" back  ·  ", Style::new().fg(Color::Gray)),
            Span::styled("ctrl+c", Style::new().fg(Color::Cyan).bold()),
            Span::styled(" quit", Style::new().fg(Color::Gray)),
        ])
    };
    frame.render_widget(Paragraph::new(footer), footer_area);
}
