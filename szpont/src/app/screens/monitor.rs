use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Wrap,
};

use crate::app::{App, Pending, Screen, tree_components};
use crate::core::snapshot::SessionRow;
use crate::core::{Liveness, format_age, format_tokens, now_ms, truncate};

const POPUP_WIDTH: u16 = 72;
const PICKER_WIDTH: u16 = 44;
const REPO_WIDTH: u16 = 68;
const FLAT_FIXED_COLUMNS: [u16; 7] = [5, 1, 7, 7, REPO_WIDTH, 9, 20];
const ARCHIVE_EXTRA_COLUMN: u16 = 16;
const TREE_FIXED_COLUMNS: [u16; 6] = [1, 7, 7, 9, 20, 5];

fn fixed_table_width(columns: &[u16], extra: Option<u16>) -> u16 {
    let base: u16 = columns.iter().sum();
    let count = columns.len() as u16 + u16::from(extra.is_some());
    base + extra.unwrap_or(0) + count
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let tall = frame.area().height >= 22 && frame.area().width >= 90;
    let width = frame.area().width.max(1) as usize;
    let footer = footer_line(app);
    let footer_height = wrapped_height(footer.width(), width, MAX_FOOTER_LINES);
    let header_height = if tall {
        crate::app::widgets::logo::HEIGHT
    } else {
        wrapped_height(header_line(app, true).width(), width, MAX_HEADER_LINES)
    };
    let [header_area, table_area, separator_area, footer_area] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(footer_height),
    ])
    .areas(frame.area());
    frame.render_widget(
        Paragraph::new("─".repeat(frame.area().width as usize))
            .style(Style::new().fg(Color::DarkGray)),
        separator_area,
    );
    if tall {
        let [logo_area, info_area] = Layout::horizontal([
            Constraint::Length(crate::app::widgets::logo::WIDTH),
            Constraint::Min(0),
        ])
        .areas(header_area);
        let flapped = app.scanning && (now_ms() / 300) % 2 == 0;
        frame.render_widget(
            Paragraph::new(crate::app::widgets::logo::carton(flapped)),
            logo_area,
        );
        let [title_line, info_lines, scan_line, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(info_area);
        frame.render_widget(
            Paragraph::new(Span::styled(
                " szpont machen",
                Style::new().fg(Color::Yellow).bold(),
            )),
            title_line,
        );
        frame.render_widget(
            Paragraph::new(header_line(app, false)).wrap(Wrap { trim: true }),
            info_lines,
        );
        if let Some(scan) = &app.scan_status {
            frame.render_widget(
                Paragraph::new(format!(" {scan}")).style(Style::new().fg(Color::DarkGray)),
                scan_line,
            );
        }
    } else {
        frame.render_widget(
            Paragraph::new(header_line(app, true)).wrap(Wrap { trim: true }),
            header_area,
        );
    }
    draw_table(frame, app, table_area);
    frame.render_widget(
        Paragraph::new(footer).wrap(Wrap { trim: true }),
        footer_area,
    );
    if let Some(Pending::NewSession { tools, selected }) = &app.pending {
        draw_new_session_popup(frame, app, tools, *selected);
    }
    if app.show_limits {
        draw_limits_popup(frame, app);
    }
    if app.show_detail {
        draw_detail_popup(frame, app);
    }
}

fn draw_detail_popup(frame: &mut Frame, app: &App) {
    let Some(row) = app.selected_row() else {
        return;
    };
    let now = now_ms();
    let dim = Style::new().fg(Color::DarkGray);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("tool     ", dim),
            Span::raw(row.session.key.tool.display_name().to_string()),
        ]),
        Line::from(vec![
            Span::styled("session  ", dim),
            Span::raw(row.session.key.id.clone()),
        ]),
        Line::from(vec![
            Span::styled("title    ", dim),
            Span::raw(row.session.title.clone().unwrap_or_else(|| "-".to_string())),
        ]),
        Line::from(vec![
            Span::styled("cwd      ", dim),
            Span::raw(
                row.session
                    .cwd
                    .as_deref()
                    .map_or_else(|| "?".to_string(), |p| p.display().to_string()),
            ),
        ]),
        Line::from(vec![
            Span::styled("model    ", dim),
            Span::raw(row.session.model.clone().unwrap_or_else(|| "-".to_string())),
        ]),
        Line::from(vec![
            Span::styled("updated  ", dim),
            Span::raw(format!(
                "{} ago",
                format_age(now - row.session.updated_at_ms)
            )),
        ]),
    ];
    if let Some(tokens) = row.context_tokens.filter(|t| *t > 0) {
        let text = match row.context_window {
            Some(window) if window > 0 => format!(
                "{} of {} ({:.0}%)",
                format_tokens(tokens),
                format_tokens(window),
                (tokens as f64 / window as f64 * 100.0).min(100.0)
            ),
            _ => format!("{} (window unknown)", format_tokens(tokens)),
        };
        lines.push(Line::from(vec![
            Span::styled("context  ", dim),
            Span::raw(text),
        ]));
    }
    if let Some(usage) = &row.usage {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("token usage", Style::new().bold())));
        for (label, value) in [
            ("input", usage.input_uncached),
            ("cache read", usage.input_cache_read),
            ("cache write", usage.input_cache_write),
            ("output", usage.output),
        ] {
            lines.push(Line::from(vec![
                Span::styled(format!("  {label:<12}"), dim),
                Span::raw(format_tokens(value)),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled("  total       ".to_string(), dim),
            Span::styled(format_tokens(usage.total()), Style::new().bold()),
        ]));
    } else if let Some(tokens) = row.session.native_tokens_used {
        lines.push(Line::from(vec![
            Span::styled("tokens   ", dim),
            Span::raw(format!("{} (reported by the tool)", format_tokens(tokens))),
        ]));
    }
    if let Some(preview) = &row.session.preview {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("last prompt", Style::new().bold())));
        lines.push(Line::from(format!("  {}", truncate(preview, 66))));
    }
    let area = popup_area(frame.area(), POPUP_WIDTH, lines.len());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(" session ")
                .border_style(Style::new().fg(Color::Blue)),
        ),
        area,
    );
}

fn popup_area(screen: Rect, width: u16, content_lines: usize) -> Rect {
    let height = (content_lines as u16 + 2).min(screen.height.saturating_sub(2));
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(screen);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}

fn draw_limits_popup(frame: &mut Frame, app: &App) {
    let now = now_ms();
    let mut lines: Vec<Line> = Vec::new();
    if app.limits.is_empty() {
        lines.push(Line::from("no limit data yet"));
    }
    for tool in &app.limits {
        let mut header = vec![Span::styled(
            tool.tool.display_name().to_string(),
            Style::new().bold(),
        )];
        if let Some(plan) = &tool.plan {
            header.push(Span::styled(
                format!("  ({plan} plan)"),
                Style::new().fg(Color::DarkGray),
            ));
        }
        header.push(Span::styled(
            format!("  via {}", tool.source),
            Style::new().fg(Color::DarkGray),
        ));
        lines.push(Line::from(header));
        if tool.windows.is_empty() {
            lines.push(Line::from(Span::styled(
                "  no limit data",
                Style::new().fg(Color::DarkGray),
            )));
        }
        for window in &tool.windows {
            let mut spans = vec![Span::raw(format!("  {:<5}", window.label))];
            match window.used_percent {
                Some(pct) => {
                    let color = usage_color(pct);
                    spans.push(Span::styled(usage_bar(pct, 24), Style::new().fg(color)));
                    let marker = if window.estimated { " est" } else { "" };
                    spans.push(Span::styled(
                        format!(" {pct:.0}%{marker}"),
                        Style::new().fg(color),
                    ));
                }
                None => {
                    spans.push(Span::styled(
                        "no ceiling observed yet".to_string(),
                        Style::new().fg(Color::DarkGray),
                    ));
                }
            }
            if let Some(tokens) = window.tokens {
                spans.push(Span::styled(
                    format!("  {} tokens", format_tokens(tokens)),
                    Style::new().fg(Color::DarkGray),
                ));
            }
            if let Some(resets_at) = window.resets_at {
                let remaining_ms = resets_at.saturating_mul(1000).saturating_sub(now);
                if remaining_ms > 0 {
                    spans.push(Span::styled(
                        format!("  ↻ in {}", format_age(remaining_ms)),
                        Style::new().fg(Color::Cyan),
                    ));
                }
            }
            lines.push(Line::from(spans));
        }
        if let Some(note) = &tool.note {
            lines.push(Line::from(Span::styled(
                format!("  {note}"),
                Style::new().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(""));
    }
    let area = popup_area(frame.area(), POPUP_WIDTH, lines.len());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .title(" limit usage ")
                .border_style(Style::new().fg(Color::Blue)),
        ),
        area,
    );
}

fn draw_new_session_popup(
    frame: &mut Frame,
    app: &App,
    tools: &[crate::core::ToolId],
    selected: usize,
) {
    let area = popup_area(frame.area(), PICKER_WIDTH, tools.len());
    frame.render_widget(Clear, area);
    let where_label = app
        .repo
        .as_ref()
        .filter(|_| app.screen == Screen::Repo)
        .map_or_else(|| "current directory".to_string(), |ctx| ctx.name.clone());
    let items: Vec<ListItem> = tools
        .iter()
        .enumerate()
        .map(|(i, tool)| {
            let marker = if i == selected { "▸" } else { " " };
            let mut style = Style::new().fg(tool_color(*tool));
            if i == selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            ListItem::new(format!(" {marker} {}. {}", i + 1, tool.display_name())).style(style)
        })
        .collect();
    let list = List::new(items).block(
        Block::bordered()
            .title(format!(" new session in {} ", truncate(&where_label, 32)))
            .border_style(Style::new().fg(Color::Green)),
    );
    frame.render_widget(list, area);
}

fn header_line(app: &App, with_logo: bool) -> Line<'static> {
    let visible = app.visible_indices();
    let mut running = 0;
    let mut waiting = 0;
    let mut open = 0;
    for &i in &visible {
        match app.rows[i].liveness {
            Liveness::Running => running += 1,
            Liveness::WaitingForInput => waiting += 1,
            Liveness::Open => open += 1,
            Liveness::Idle => {}
        }
    }
    let scope = match (app.screen, &app.repo) {
        (Screen::Archive, _) => "archive".to_string(),
        (Screen::Tree, _) if app.tree_of_archive() => "archive by location".to_string(),
        (Screen::Tree, _) => "by location".to_string(),
        (Screen::Repo, Some(ctx)) => format!("repo: {}", ctx.name),
        _ => "all repos".to_string(),
    };
    let mut spans = Vec::new();
    if with_logo {
        spans.push(crate::app::widgets::logo::compact());
        spans.push(Span::raw("  "));
    } else {
        spans.push(Span::raw(" "));
    }
    spans.extend([
        Span::styled(scope, Style::new().fg(Color::Magenta).bold()),
        Span::raw("   "),
        Span::styled(format!("{running} RUNNING"), Style::new().fg(Color::Green)),
        Span::raw("   "),
        Span::styled(format!("{waiting} BLOCKED"), Style::new().fg(Color::Yellow)),
        Span::raw("   "),
        Span::styled(format!("{open} IDLE"), Style::new().fg(Color::Cyan)),
        Span::raw("   "),
        Span::styled(
            format!("{} sessions", visible.len()),
            Style::new().fg(Color::DarkGray),
        ),
    ]);
    if app.pending_snapshot.is_some() {
        let bright_phase = (now_ms() / 600) % 2 == 0;
        let style = if bright_phase {
            Style::new().fg(Color::Yellow).bold()
        } else {
            Style::new().fg(Color::Yellow).dim()
        };
        spans.push(Span::styled(
            "   ⟳ list changed — press r to refresh",
            style,
        ));
    }
    if let Some(tool) = app.tool_filter {
        spans.push(Span::styled(
            format!("   [tool: {}]", tool.as_str()),
            Style::new().fg(Color::Blue).bold(),
        ));
    }
    if let Some(filter) = &app.filter {
        spans.push(Span::styled(
            format!("   [filter: {filter}]"),
            Style::new().fg(Color::Blue).bold(),
        ));
    }
    spans.push(Span::styled("   │", Style::new().fg(Color::DarkGray)));
    spans.extend(limits_spans(app));
    if with_logo && let Some(scan) = &app.scan_status {
        spans.push(Span::styled(
            format!("   ⋯ {scan}"),
            Style::new().fg(Color::DarkGray),
        ));
    }
    Line::from(spans)
}

fn limits_spans(app: &App) -> Vec<Span<'static>> {
    if app.limits.is_empty() {
        return vec![Span::styled(
            " limits: measuring…",
            Style::new().fg(Color::DarkGray),
        )];
    }
    let mut spans = vec![Span::raw(" ")];
    let mut first = true;
    for tool in &app.limits {
        if !first {
            spans.push(Span::styled("  │  ", Style::new().fg(Color::DarkGray)));
        }
        first = false;
        spans.push(Span::styled(
            format!("{} ", tool.tool.as_str()),
            Style::new().fg(Color::DarkGray).bold(),
        ));
        if tool.windows.is_empty() {
            spans.push(Span::styled("—", Style::new().fg(Color::DarkGray)));
            continue;
        }
        for (i, window) in tool.windows.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            match window.used_percent {
                Some(pct) => {
                    let color = usage_color(pct);
                    let marker = if window.estimated { "~" } else { "" };
                    spans.push(Span::raw(format!("{} ", window.label)));
                    spans.push(Span::styled(usage_bar(pct, 7), Style::new().fg(color)));
                    spans.push(Span::styled(
                        format!(" {pct:.0}%{marker}"),
                        Style::new().fg(color),
                    ));
                }
                None => {
                    spans.push(Span::raw(format!(
                        "{} {}",
                        window.label,
                        window.tokens.map_or_else(|| "?".to_string(), format_tokens)
                    )));
                }
            }
        }
    }
    spans
}

fn usage_color(pct: f64) -> Color {
    if pct >= 90.0 {
        Color::Red
    } else if pct >= 70.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn usage_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64)
        .round()
        .clamp(0.0, width as f64) as usize;
    format!("{}{}", "▓".repeat(filled), "░".repeat(width - filled))
}

fn draw_tree_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let visible = app.visible_indices();
    if visible.is_empty() {
        let message = if app.received_first_snapshot {
            "no active sessions — start claude, codex or kimi in a repo"
        } else {
            "scanning sessions…"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::new().fg(Color::DarkGray))
                .centered(),
            area,
        );
        return;
    }
    let now = now_ms();
    let title_width = area
        .width
        .saturating_sub(fixed_table_width(&TREE_FIXED_COLUMNS, None))
        .max(10) as usize;
    let mut roots = TreeDir::new(String::new());
    for &i in &visible {
        let components = tree_components(app.rows[i].session.cwd.as_deref());
        roots.insert(&components, i);
    }
    let mut table_rows: Vec<Row> = Vec::new();
    let mut selected_render = 0usize;
    let mut session_position = 0usize;
    let root_count = roots.children.len();
    for (index, root) in roots.children.iter().enumerate() {
        render_tree_dir(
            app,
            root,
            "",
            index + 1 == root_count,
            true,
            now,
            title_width,
            &mut table_rows,
            &mut session_position,
            &mut selected_render,
        );
    }
    let table = Table::new(
        table_rows,
        [
            Constraint::Length(TREE_FIXED_COLUMNS[0]),
            Constraint::Min(24),
            Constraint::Length(TREE_FIXED_COLUMNS[1]),
            Constraint::Length(TREE_FIXED_COLUMNS[2]),
            Constraint::Length(TREE_FIXED_COLUMNS[3]),
            Constraint::Length(TREE_FIXED_COLUMNS[4]),
            Constraint::Length(TREE_FIXED_COLUMNS[5]),
        ],
    )
    .header(
        Row::new([
            "",
            "LOCATION / TITLE",
            "STATUS",
            "TOOL",
            "TOKENS",
            "CTX",
            "LAST",
        ])
        .style(
            Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    let mut state = TableState::default()
        .with_offset(app.scroll)
        .with_selected(Some(selected_render));
    frame.render_stateful_widget(table, area, &mut state);
    app.scroll = state.offset();
}

struct TreeDir {
    name: String,
    children: Vec<TreeDir>,
    sessions: Vec<usize>,
}

impl TreeDir {
    fn new(name: String) -> TreeDir {
        TreeDir {
            name,
            children: Vec::new(),
            sessions: Vec::new(),
        }
    }

    fn insert(&mut self, components: &[String], row: usize) {
        let Some(first) = components.first() else {
            self.sessions.push(row);
            return;
        };
        let position = self.children.iter().position(|c| c.name == *first);
        let child = if let Some(pos) = position {
            &mut self.children[pos]
        } else {
            self.children.push(TreeDir::new(first.clone()));
            self.children.last_mut().unwrap()
        };
        child.insert(&components[1..], row);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tree_dir(
    app: &App,
    dir: &TreeDir,
    prefix: &str,
    is_last: bool,
    is_root: bool,
    now: i64,
    title_width: usize,
    table_rows: &mut Vec<Row<'static>>,
    session_position: &mut usize,
    selected_render: &mut usize,
) {
    let branch = if is_root {
        ""
    } else if is_last {
        "└─ "
    } else {
        "├─ "
    };
    table_rows.push(
        Row::new(vec![
            Cell::from(""),
            Cell::from(format!("{prefix}{branch}{}", dir.name)),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ])
        .style(Style::new().fg(Color::Blue).bold()),
    );
    let child_prefix = if is_root {
        String::new()
    } else {
        format!("{prefix}{}", if is_last { "   " } else { "│  " })
    };
    let total = dir.sessions.len() + dir.children.len();
    for (position, &row_index) in dir.sessions.iter().enumerate() {
        let item_last = position + 1 == total;
        let item_branch = if item_last { "└─ " } else { "├─ " };
        if *session_position == app.selected {
            *selected_render = table_rows.len();
        }
        *session_position += 1;
        let row = &app.rows[row_index];
        let (status_label, status_style) = liveness_label(row.liveness);
        let mark = if app.marked.contains(&row.session.key) {
            "✓"
        } else {
            " "
        };
        let title = row
            .session
            .title
            .as_deref()
            .or(row.session.preview.as_deref())
            .unwrap_or("-");
        let label_width = title_width.saturating_sub(child_prefix.chars().count() + 3);
        table_rows.push(
            Row::new(vec![
                Cell::from(mark).style(Style::new().fg(Color::Magenta).bold()),
                Cell::from(Line::from(vec![
                    Span::styled(
                        format!("{child_prefix}{item_branch}"),
                        Style::new().fg(Color::DarkGray),
                    ),
                    Span::raw(truncate(title, label_width)),
                ])),
                Cell::from(status_label).style(status_style),
                Cell::from(row.session.key.tool.as_str())
                    .style(Style::new().fg(tool_color(row.session.key.tool))),
                Cell::from(
                    row.tokens_total()
                        .map_or_else(|| "-".to_string(), format_tokens),
                ),
                context_cell(row),
                Cell::from(format_age(now - row.session.updated_at_ms)),
            ])
            .style(match row.liveness {
                Liveness::Idle => Style::new().fg(Color::Gray),
                Liveness::WaitingForInput => Style::new().add_modifier(Modifier::BOLD),
                _ => Style::new(),
            }),
        );
    }
    let child_count = dir.children.len();
    for (position, child) in dir.children.iter().enumerate() {
        render_tree_dir(
            app,
            child,
            &child_prefix,
            position + 1 == child_count,
            false,
            now,
            title_width,
            table_rows,
            session_position,
            selected_render,
        );
    }
}

fn frame_style(app: &App) -> (Color, &'static str) {
    if let Some(mode) = app.marking {
        return match mode {
            crate::app::Marking::Complete => (Color::Yellow, " marking as completed "),
            crate::app::Marking::Delete => (Color::Red, " marking for deletion "),
        };
    }
    if matches!(app.pending, Some(Pending::NewSession { .. })) {
        return (Color::Green, " new session ");
    }
    match app.screen {
        Screen::Archive => (Color::Magenta, " archive "),
        Screen::Tree if app.tree_of_archive() => (Color::Magenta, " archive by location "),
        Screen::Tree => (Color::Blue, " sessions by location "),
        _ => (Color::DarkGray, " sessions "),
    }
}

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let (frame_color, frame_title) = frame_style(app);
    let block = Block::bordered()
        .border_style(Style::new().fg(frame_color))
        .title(frame_title)
        .title_style(Style::new().fg(frame_color).bold());
    let area = {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    };
    if app.screen == Screen::Tree {
        draw_tree_table(frame, app, area);
        return;
    }
    let visible = app.visible_indices();
    if visible.is_empty() {
        let message = if !app.received_first_snapshot {
            "scanning sessions…"
        } else if app.screen == Screen::Archive {
            "archive is empty — mark sessions as completed with c"
        } else {
            "no active sessions — start claude, codex or kimi in a repo"
        };
        frame.render_widget(
            Paragraph::new(message)
                .style(Style::new().fg(Color::DarkGray))
                .centered(),
            area,
        );
        return;
    }
    let now = now_ms();
    let archive = app.screen == Screen::Archive;
    let extra = archive.then_some(ARCHIVE_EXTRA_COLUMN);
    let fixed_width = fixed_table_width(&FLAT_FIXED_COLUMNS, extra);
    let title_width = area.width.saturating_sub(fixed_width).max(10) as usize;
    let rows: Vec<Row> = visible
        .iter()
        .map(|&i| {
            let row = &app.rows[i];
            session_row(
                row,
                now,
                archive,
                app.marked.contains(&row.session.key),
                title_width,
            )
        })
        .collect();
    let mut widths = vec![
        Constraint::Length(FLAT_FIXED_COLUMNS[0]),
        Constraint::Length(FLAT_FIXED_COLUMNS[1]),
        Constraint::Length(FLAT_FIXED_COLUMNS[2]),
        Constraint::Length(FLAT_FIXED_COLUMNS[3]),
        Constraint::Min(24),
        Constraint::Length(FLAT_FIXED_COLUMNS[4]),
        Constraint::Length(FLAT_FIXED_COLUMNS[5]),
        Constraint::Length(FLAT_FIXED_COLUMNS[6]),
    ];
    let mut header = vec![
        "LAST", "", "STATUS", "TOOL", "TITLE", "REPO", "TOKENS", "CTX",
    ];
    if archive {
        widths.push(Constraint::Length(ARCHIVE_EXTRA_COLUMN));
        header.push("COMPLETED");
    }
    let table = Table::new(rows, widths)
        .header(
            Row::new(header).style(
                Style::new()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    let mut state = TableState::default()
        .with_offset(app.scroll)
        .with_selected(Some(app.selected));
    frame.render_stateful_widget(table, area, &mut state);
    app.scroll = state.offset();
}

fn session_row(
    row: &SessionRow,
    now: i64,
    archive: bool,
    marked: bool,
    title_width: usize,
) -> Row<'static> {
    let (status_label, status_style) = if archive {
        ("", Style::new())
    } else {
        liveness_label(row.liveness)
    };
    let title = row
        .session
        .title
        .as_deref()
        .or(row.session.preview.as_deref())
        .unwrap_or("-");
    let base = if archive {
        Style::new()
    } else {
        match row.liveness {
            Liveness::Idle => Style::new().fg(Color::Gray),
            Liveness::WaitingForInput => Style::new().add_modifier(Modifier::BOLD),
            _ => Style::new(),
        }
    };
    let mark = if marked { "✓" } else { " " };
    let mut cells = vec![
        Cell::from(format_age(now - row.session.updated_at_ms)),
        Cell::from(mark).style(Style::new().fg(Color::Magenta).bold()),
        Cell::from(status_label).style(status_style),
        Cell::from(row.session.key.tool.as_str())
            .style(Style::new().fg(tool_color(row.session.key.tool))),
        Cell::from(truncate(title, title_width)),
        Cell::from(repo_label(row)),
        Cell::from(
            row.tokens_total()
                .map_or_else(|| "-".to_string(), format_tokens),
        ),
        context_cell(row),
    ];
    if archive {
        let completed = match (row.completed_at, row.session.native_archived) {
            (Some(at), _) => format!("{} ago", format_age(now - at)),
            (None, true) => format!("[{}]", row.session.key.tool.as_str()),
            (None, false) => "-".to_string(),
        };
        cells.push(Cell::from(completed));
    }
    Row::new(cells).style(base)
}

fn tool_color(tool: crate::core::ToolId) -> Color {
    match tool {
        crate::core::ToolId::Claude => Color::LightRed,
        crate::core::ToolId::Codex => Color::LightCyan,
        crate::core::ToolId::Kimi => Color::LightMagenta,
    }
}

fn liveness_label(liveness: Liveness) -> (&'static str, Style) {
    match liveness {
        Liveness::Running => ("RUNNING", Style::new().fg(Color::Green).bold()),
        Liveness::WaitingForInput => ("BLOCKED", Style::new().fg(Color::Yellow).bold()),
        Liveness::Open => ("IDLE", Style::new().fg(Color::Cyan)),
        Liveness::Idle => ("", Style::new()),
    }
}

const MAX_FOOTER_LINES: u16 = 3;
const MAX_HEADER_LINES: u16 = 2;

fn wrapped_height(content_width: usize, area_width: usize, cap: u16) -> u16 {
    let lines = content_width.div_ceil(area_width.max(1)).max(1);
    (lines as u16).min(cap)
}

fn footer_line(app: &App) -> Line<'static> {
    let status_style = Style::new().fg(Color::Yellow);
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    if let Some(input) = &app.filter_input {
        let matches = app.visible_indices().len();
        spans.push(Span::styled(
            format!("find: {}", input.display_with_cursor()),
            status_style,
        ));
        spans.push(Span::styled(
            format!("   {matches} match(es)   "),
            Style::new().fg(Color::DarkGray),
        ));
        spans.extend(hint_spans(&[("enter", "keep"), ("esc", "cancel")]));
    } else if let Some(input) = &app.window_input {
        spans.push(Span::styled(
            format!(
                "context window for {}: {}",
                input.model,
                input.editor.display_with_cursor()
            ),
            status_style,
        ));
        spans.push(Span::raw("   "));
        spans.extend(hint_spans(&[("enter", "save"), ("esc", "cancel")]));
    } else if let Some(input) = &app.rename_input {
        spans.push(Span::styled(
            format!("rename: {}", input.editor.display_with_cursor()),
            status_style,
        ));
        spans.push(Span::raw("   "));
        spans.extend(hint_spans(&[("enter", "save"), ("esc", "cancel")]));
    } else if let Some(mode) = app.marking {
        match mode {
            crate::app::Marking::Complete => {
                let confirm_label = format!("confirm {} marked as completed", app.marked.len());
                spans.extend(hint_spans(&[
                    ("c", "mark"),
                    ("↑/↓", "move"),
                    ("enter", confirm_label.as_str()),
                    ("esc", "cancel"),
                ]));
            }
            crate::app::Marking::Delete => {
                let confirm_label = format!("PERMANENTLY delete {} marked", app.marked.len());
                spans.extend(hint_spans(&[
                    ("d", "mark"),
                    ("↑/↓", "move"),
                    ("enter", confirm_label.as_str()),
                    ("esc", "cancel"),
                ]));
            }
        }
    } else if let Some(Pending::NewSession { tools, .. }) = &app.pending {
        let pick_label = if tools.len() == 1 {
            "1".to_string()
        } else {
            format!("1-{}", tools.len().min(9))
        };
        spans.extend(hint_spans(&[
            ("↑/↓", "choose"),
            ("enter", "start"),
            (pick_label.as_str(), "direct pick"),
            ("esc", "cancel"),
        ]));
    } else {
        if let Some(status) = &app.status {
            spans.push(Span::styled(status.clone(), status_style));
            spans.push(Span::styled("   │   ", Style::new().fg(Color::DarkGray)));
        }
        let tree = app.tree_label();
        let hints: Vec<(&str, &str)> = if app.screen == Screen::Archive {
            vec![
                ("enter", "resume"),
                ("u", "reopen"),
                ("d", "mark for deletion"),
                ("m", "rename"),
                ("v", tree),
                ("esc", "back"),
                ("ctrl+c", "quit"),
            ]
        } else if app.screen == Screen::Tree {
            let complete_hint: (&str, &str) = if app.tree_of_archive() {
                ("u", "reopen")
            } else {
                ("c", "mark as completed")
            };
            vec![
                ("enter", "resume"),
                complete_hint,
                ("d", "mark for deletion"),
                ("m", "rename"),
                ("v", tree),
                ("f", "find"),
                ("esc", "back"),
                ("ctrl+c", "quit"),
            ]
        } else {
            let mut h = vec![
                ("enter", "resume"),
                ("c", "mark as completed"),
                ("d", "mark for deletion"),
                ("m", "rename"),
                ("n", "new session"),
                ("a", "archive"),
                ("v", tree),
            ];
            if let Some(scope) = app.scope_label() {
                h.push(("p", scope));
            }
            h.extend([
                ("f", "find"),
                ("o", "options"),
                ("r", "refresh"),
                ("ctrl+c", "quit"),
            ]);
            h
        };
        spans.extend(hint_spans(&hints));
    }
    Line::from(spans)
}

fn hint_spans<'a>(hints: &'a [(&'a str, &'a str)]) -> Vec<Span<'static>> {
    let key_style = Style::new().fg(Color::Cyan).bold();
    let label_style = Style::new().fg(Color::Gray);
    let mnemonic_style = Style::new()
        .fg(Color::Yellow)
        .bold()
        .add_modifier(Modifier::UNDERLINED);
    let mut spans = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Style::new().fg(Color::DarkGray)));
        }
        let mnemonic = key
            .chars()
            .next()
            .filter(|_| key.chars().count() == 1)
            .and_then(|k| {
                label
                    .char_indices()
                    .find(|(_, c)| c.eq_ignore_ascii_case(&k))
            });
        if let Some((pos, c)) = mnemonic {
            let (before, rest) = label.split_at(pos);
            let after = &rest[c.len_utf8()..];
            if !before.is_empty() {
                spans.push(Span::styled(before.to_string(), label_style));
            }
            spans.push(Span::styled(c.to_string(), mnemonic_style));
            if !after.is_empty() {
                spans.push(Span::styled(after.to_string(), label_style));
            }
        } else {
            spans.push(Span::styled(key.to_string(), key_style));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(label.to_string(), label_style));
        }
    }
    spans
}

fn repo_label(row: &SessionRow) -> String {
    let Some(cwd) = row.session.cwd.as_deref() else {
        return "?".to_string();
    };
    let text = match dirs::home_dir().and_then(|home| {
        cwd.strip_prefix(&home)
            .ok()
            .map(std::path::Path::to_path_buf)
    }) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => cwd.display().to_string(),
    };
    truncate_left(&text, REPO_WIDTH as usize)
}

fn truncate_left(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let tail: String = s.chars().skip(count - max.saturating_sub(1)).collect();
    format!("…{tail}")
}

fn context_cell(row: &SessionRow) -> Cell<'static> {
    match (row.context_tokens.filter(|t| *t > 0), row.context_window) {
        (Some(tokens), Some(window)) if window > 0 => {
            let pct = (tokens as f64 / window as f64 * 100.0).min(100.0);
            Cell::from(Line::from(vec![
                Span::raw(format!(
                    "{:>13} ",
                    format!("{}/{}", format_tokens(tokens), format_tokens(window))
                )),
                Span::styled(usage_bar(pct, 6), Style::new().fg(usage_color(pct))),
            ]))
        }
        (Some(tokens), _) => Cell::from(format!("{:>13}", format_tokens(tokens))),
        _ => Cell::from(format!("{:>13}", "-")),
    }
}
