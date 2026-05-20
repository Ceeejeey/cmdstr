use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, InputMode};

const GREEN: Color = Color::Rgb(0x00, 0xff, 0x00);
const DARK_GREEN: Color = Color::Rgb(0x00, 0x55, 0x00);
const BRIGHT_GREEN: Color = Color::Rgb(0x55, 0xff, 0x55);
const BG: Color = Color::Rgb(0x00, 0x00, 0x00);
const RED: Color = Color::Rgb(0xff, 0x33, 0x33);
const DARK_RED: Color = Color::Rgb(0x55, 0x00, 0x00);
const BRIGHT_RED: Color = Color::Rgb(0xff, 0x66, 0x66);
const YELLOW: Color = Color::Rgb(0xff, 0xcc, 0x00);

fn fg(c: Color) -> Style {
    Style::new().fg(c).bg(BG)
}

fn border_style(sudo: bool) -> Style {
    if sudo { fg(DARK_RED) } else { fg(DARK_GREEN) }
}

fn base_style(sudo: bool) -> Style {
    if sudo { fg(BRIGHT_RED) } else { fg(GREEN) }
}

fn highlight_style(sudo: bool) -> Style {
    let c = if sudo { BRIGHT_RED } else { BRIGHT_GREEN };
    Style::new().fg(c).bg(BG).add_modifier(Modifier::BOLD)
}

fn dim_style(sudo: bool) -> Style {
    let c = if sudo { DARK_RED } else { DARK_GREEN };
    fg(c)
}

fn error_style() -> Style {
    Style::new().fg(RED).bg(BG).add_modifier(Modifier::BOLD)
}

fn title_style(sudo: bool) -> Style {
    let c = if sudo { BRIGHT_RED } else { BRIGHT_GREEN };
    Style::new().fg(c).bg(BG).add_modifier(Modifier::BOLD)
}

fn mode_label(mode: &InputMode, sudo: bool) -> &'static str {
    match mode {
        InputMode::Tool => {
            if sudo { " SUDO MODE " } else { " TOOL MODE " }
        }
        InputMode::Run => " RUN MODE ",
        InputMode::Search => " SEARCH ",
        InputMode::Tag => " TAG ",
        InputMode::Note => " NOTE ",
        InputMode::Password => " PASSWORD ",
        InputMode::Add => " ADD ",
    }
}

pub fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let sudo = app.sudo_mode;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    render_title(f, chunks[0], app);
    render_command_list(f, chunks[1], app, sudo);
    render_detail(f, chunks[2], app, sudo);
    render_input(f, chunks[3], app, sudo);
    render_help(f, chunks[4], app, sudo);

    if app.show_password_popup {
        render_password_popup(f, area, app);
    }

    if app.show_help {
        render_help_popup(f, area);
    }
}

fn render_title(f: &mut Frame, area: Rect, app: &App) {
    let sudo = app.sudo_mode;
    let mode_str = mode_label(&app.mode, sudo);
    let color = if sudo { BRIGHT_RED } else { GREEN };
    let mut spans = vec![
        Span::styled(" cmdstr ", Style::new().fg(color).bg(BG).add_modifier(Modifier::BOLD)),
        Span::styled("│", dim_style(sudo)),
        Span::styled(mode_str, Style::new().fg(color).bg(BG).add_modifier(Modifier::BOLD)),
    ];

    if !app.mode_msg.is_empty() && app.mode != InputMode::Password {
        spans.push(Span::styled(format!(" [{}]", app.mode_msg), fg(YELLOW)));
    }
    if !(app.status_msg.is_empty() || (app.mode == InputMode::Password && app.show_password_popup)) {
        let status_color = if app.status_msg.contains("Running") || app.status_msg.contains("Starting") {
            Style::new().fg(YELLOW).bg(BG).add_modifier(Modifier::BOLD)
        } else if app.status_msg.contains("Error") || app.status_msg.contains("failed") {
            error_style()
        } else {
            dim_style(sudo)
        };
        spans.push(Span::styled(format!(" {}", app.status_msg), status_color));
    }

    let title = Line::from(spans);
    f.render_widget(Paragraph::new(title).style(base_style(sudo)), area);
}

fn render_command_list(f: &mut Frame, area: Rect, app: &mut App, sudo: bool) {
    let bstyle = border_style(sudo);
    let tstyle = title_style(sudo);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(bstyle)
        .title(Line::from(Span::styled(" COMMANDS ", tstyle)))
        .style(Style::new().bg(BG));

    let inner = block.inner(area);
    app.list_inner_height = inner.height as usize;
    f.render_widget(block, area);

    if app.filtered.is_empty() {
        let empty = Paragraph::new(Text::styled("  No commands found.", dim_style(sudo)))
            .style(Style::new().bg(BG));
        f.render_widget(empty, inner);
        return;
    }

    let list_height = inner.height as usize;
    let start = app.list_scroll;
    let end = (start + list_height).min(app.filtered.len());

    let items: Vec<ListItem> = app.filtered[start..end]
        .iter()
        .enumerate()
        .map(|(rel_idx, &cmd_idx)| {
            let cmd = &app.commands[cmd_idx];
            let is_selected = start + rel_idx == app.selected;

            let status = if cmd.exit_code == 0 { "✓" } else { "✗" };
            let status_color = if cmd.exit_code == 0 { GREEN } else { RED };
            let bm = if cmd.is_bookmark { "★" } else { " " };
            let tag_str = if cmd.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", cmd.tags.join(","))
            };
            let cmd_display = truncate(&cmd.command, (inner.width as usize).saturating_sub(24));

            let pointer = if is_selected { "▸" } else { " " };

            let line = Line::from(vec![
                Span::styled(pointer, if is_selected { highlight_style(sudo) } else { dim_style(sudo) }),
                Span::styled(" ", base_style(sudo)),
                Span::styled(status, Style::new().fg(status_color).bg(BG)),
                Span::styled(" ", base_style(sudo)),
                Span::styled(bm, Style::new().fg(YELLOW).bg(BG)),
                Span::styled(" ", base_style(sudo)),
                if is_selected {
                    Span::styled(cmd_display, highlight_style(sudo))
                } else {
                    Span::styled(cmd_display, base_style(sudo))
                },
                Span::styled(tag_str, dim_style(sudo)),
            ]);

            let sel_bg = if sudo {
                Color::Rgb(0x44, 0x00, 0x00)
            } else {
                Color::Rgb(0x00, 0x33, 0x00)
            };

            if is_selected {
                ListItem::new(line).style(Style::new().bg(sel_bg))
            } else {
                ListItem::new(line).style(Style::new().bg(BG))
            }
        })
        .collect();

    let list = List::new(items).style(Style::new().bg(BG));
    f.render_widget(list, inner);
}

fn render_detail(f: &mut Frame, area: Rect, app: &App, sudo: bool) {
    let bstyle = border_style(sudo);
    let tstyle = title_style(sudo);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(bstyle)
        .title(Line::from(Span::styled(" DETAILS / OUTPUT ", tstyle)))
        .style(Style::new().bg(BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let (left, right) = {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(inner);
        (chunks[0], chunks[1])
    };

    render_command_details(f, left, app, sudo);
    render_output(f, right, app, sudo);
}

fn render_command_details(f: &mut Frame, area: Rect, app: &App, sudo: bool) {
    let mut text = Vec::new();

    if let Some(cmd) = app.selected_command() {
        text.push(Line::from(vec![
            Span::styled("ID:      ", dim_style(sudo)),
            Span::styled(short_id(&cmd.id), base_style(sudo)),
        ]));
        let status_text = if cmd.exit_code == 0 {
            "OK".to_string()
        } else {
            format!("FAIL ({})", cmd.exit_code)
        };
        text.push(Line::from(vec![
            Span::styled("Status:  ", dim_style(sudo)),
            Span::styled(
                status_text,
                if cmd.exit_code == 0 { base_style(sudo) } else { error_style() },
            ),
        ]));
        text.push(Line::from(vec![
            Span::styled("Time:    ", dim_style(sudo)),
            Span::styled(format!("{} ms", cmd.duration_ms), base_style(sudo)),
        ]));
        text.push(Line::from(vec![
            Span::styled("Date:    ", dim_style(sudo)),
            Span::styled(short_datetime(&cmd.captured_at), base_style(sudo)),
        ]));
        text.push(Line::from(vec![
            Span::styled("Tags:    ", dim_style(sudo)),
            if cmd.tags.is_empty() {
                Span::styled("(none)", dim_style(sudo))
            } else {
                Span::styled(cmd.tags.join(", "), base_style(sudo))
            },
        ]));
        text.push(Line::from(vec![
            Span::styled("Bookmark:", dim_style(sudo)),
            if cmd.is_bookmark {
                Span::styled(" ★", YELLOW)
            } else {
                Span::styled(" no", dim_style(sudo))
            },
        ]));
        text.push(Line::from(Span::styled("─".repeat(area.width as usize), dim_style(sudo))));
        if let Some(note) = &cmd.note {
            text.push(Line::from(vec![
                Span::styled("Note:    ", dim_style(sudo)),
                Span::styled(note.clone(), Style::new().fg(YELLOW).bg(BG)),
            ]));
        }
        text.push(Line::from(Span::styled("─".repeat(area.width as usize), dim_style(sudo))));
        text.push(Line::from(Span::styled(
            truncate(&cmd.command, area.width as usize),
            Style::new().fg(if sudo { BRIGHT_RED } else { GREEN }).bg(BG).add_modifier(Modifier::DIM),
        )));
    } else {
        text.push(Line::from(Span::styled("  Select a command from the list", dim_style(sudo))));
    }

    let para = Paragraph::new(Text::from(text))
        .style(Style::new().bg(BG))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_output(f: &mut Frame, area: Rect, app: &App, sudo: bool) {
    let bstyle = border_style(sudo);
    let tstyle = title_style(sudo);

    let output_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(bstyle)
        .title(Line::from(Span::styled(" OUTPUT ", tstyle)))
        .style(Style::new().bg(BG));

    let inner = output_block.inner(area);
    f.render_widget(output_block, area);

    let line_count = if app.output.is_empty() {
        0
    } else {
        app.output.lines().count()
    };
    let scroll_max = line_count.saturating_sub(1);

    if app.output.is_empty() {
        let empty = Paragraph::new(Text::styled(
            "  Press Enter to run or 'r' to type a command",
            dim_style(sudo),
        )).style(Style::new().bg(BG));
        f.render_widget(empty, inner);
        return;
    }

    let output_style = if sudo {
        Style::new().fg(Color::Rgb(0xcc, 0x66, 0x66)).bg(BG)
    } else {
        Style::new().fg(Color::Rgb(0x88, 0xcc, 0x88)).bg(BG)
    };
    let alt_output_style = if sudo {
        Style::new().fg(Color::Rgb(0xaa, 0x55, 0x55)).bg(BG)
    } else {
        Style::new().fg(Color::Rgb(0x66, 0xaa, 0x66)).bg(BG)
    };

    let output_lines: Vec<Line> = app.output.lines()
        .enumerate()
        .map(|(i, l)| {
            let style = if i % 2 == 0 { output_style } else { alt_output_style };
            Line::from(vec![
                Span::styled("  ", dim_style(sudo)),
                Span::styled(l.to_string(), style),
            ])
        })
        .collect();
    let output_text = Text::from(output_lines);

    let para = Paragraph::new(output_text)
        .style(Style::new().bg(BG))
        .scroll((app.output_scroll as u16, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(para, inner);

    if line_count > 0 && inner.height >= 2 {
        let scroll_pos = if scroll_max > 0 {
            let pct = app.output_scroll as f64 / scroll_max as f64;
            let bar_h = (inner.height as f64 * 0.25).max(1.0) as u16;
            let thumb_pos = ((inner.height.saturating_sub(bar_h)) as f64 * pct) as u16;
            (thumb_pos, bar_h)
        } else {
            (0u16, inner.height)
        };

        let bar_x = inner.right().saturating_sub(2);
        let scroll_color = if sudo { DARK_RED } else { DARK_GREEN };
        let thumb_color = if sudo { RED } else { GREEN };

        for y in inner.top()..inner.bottom() {
            let rel_y = y - inner.top();
            let is_thumb = rel_y >= scroll_pos.0 && rel_y < scroll_pos.0 + scroll_pos.1;
            let c = if is_thumb { thumb_color } else { scroll_color };
            let ch = if is_thumb { "█" } else { "░" };
            f.render_widget(
                Paragraph::new(Text::from(Line::from(Span::styled(ch, Style::new().fg(c).bg(BG))))),
                Rect::new(bar_x, y, 1, 1),
            );
        }

        let line_info = format!(" {}:{} ", app.output_scroll + 1, line_count);
        if inner.width > line_info.len() as u16 + 4 {
            let info_x = inner.right().saturating_sub(line_info.len() as u16 + 2);
            f.render_widget(
                Paragraph::new(Text::from(Line::from(Span::styled(
                    &line_info,
                    Style::new().fg(thumb_color).bg(BG).add_modifier(Modifier::DIM),
                )))),
                Rect::new(info_x, inner.bottom().saturating_sub(1), line_info.len() as u16, 1),
            );
        }
    }
}

fn render_input(f: &mut Frame, area: Rect, app: &App, sudo: bool) {
    let bstyle = border_style(sudo);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(bstyle)
        .style(Style::new().bg(BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prompt = match app.mode {
        InputMode::Tool => "> ",
        InputMode::Run => "run> ",
        InputMode::Search => "/ ",
        InputMode::Tag => "tags> ",
        InputMode::Note => "note> ",
        InputMode::Password => "pw> ",
        InputMode::Add => "add> ",
    };

    let cursor_visible = matches!(app.mode, InputMode::Run | InputMode::Search | InputMode::Tag | InputMode::Note | InputMode::Password | InputMode::Add);

    let (prompt_style, input_style) = match app.mode {
        InputMode::Search => (fg(YELLOW), fg(YELLOW)),
        InputMode::Tag => (Style::new().fg(BRIGHT_GREEN).bg(BG), Style::new().fg(BRIGHT_GREEN).bg(BG)),
        InputMode::Note => (fg(YELLOW), fg(YELLOW)),
        InputMode::Password => (error_style(), error_style()),
        InputMode::Add => (Style::new().fg(BRIGHT_GREEN).bg(BG), Style::new().fg(BRIGHT_GREEN).bg(BG)),
        _ => (base_style(sudo), base_style(sudo)),
    };

    let display_value = if app.mode == InputMode::Password {
        "•".repeat(app.password.len())
    } else if app.mode == InputMode::Run && app.input.is_empty() {
        "type a command to run...".to_string()
    } else {
        app.input.clone()
    };

    let line = Line::from(vec![
        Span::styled(prompt, prompt_style),
        Span::styled(&display_value, input_style),
    ]);

    let para = Paragraph::new(line).style(Style::new().bg(BG));
    f.render_widget(para, inner);

    if cursor_visible {
        let x = inner.x + prompt.len() as u16 + app.input.len() as u16;
        let x = x.min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((x, inner.y));
    }
}

fn render_help(f: &mut Frame, area: Rect, app: &App, sudo: bool) {
    let help = match app.mode {
        InputMode::Tool => {
            let help_spans = vec![
                Span::styled(" [↑↓/j/k]Nav", dim_style(sudo)),
                Span::styled(" │ ", dim_style(sudo)),
                Span::styled("[Enter]Run", dim_style(sudo)),
                Span::styled(" │ ", dim_style(sudo)),
                Span::styled("[?]Help", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
                Span::styled(" │ ", dim_style(sudo)),
                Span::styled("[S]", if sudo { error_style() } else { fg(YELLOW) }),
                Span::styled("Sudo", dim_style(sudo)),
                Span::styled(" │ ", dim_style(sudo)),
                Span::styled("[q]Quit", error_style()),
            ];
            Line::from(help_spans)
        }
        InputMode::Run => Line::from(vec![
            Span::styled(" [Enter]Execute", base_style(sudo)),
            Span::styled(" │ ", dim_style(sudo)),
            Span::styled("[Esc]Cancel", dim_style(sudo)),
        ]),
        InputMode::Search => Line::from(vec![
            Span::styled(" [Enter]Search", fg(YELLOW)),
            Span::styled(" │ ", dim_style(sudo)),
            Span::styled("[Esc]Cancel", dim_style(sudo)),
        ]),
        InputMode::Tag => Line::from(vec![
            Span::styled(" [Enter]Save Tags", Style::new().fg(BRIGHT_GREEN).bg(BG)),
            Span::styled(" │ ", dim_style(sudo)),
            Span::styled("[Esc]Cancel", dim_style(sudo)),
        ]),
        InputMode::Note => Line::from(vec![
            Span::styled(" [Enter]Save Note", fg(YELLOW)),
            Span::styled(" │ ", dim_style(sudo)),
            Span::styled("[Esc]Cancel", dim_style(sudo)),
        ]),
        InputMode::Add => Line::from(vec![
            Span::styled(" [Enter]Capture to store", Style::new().fg(BRIGHT_GREEN).bg(BG)),
            Span::styled(" │ ", dim_style(sudo)),
            Span::styled("[Esc]Cancel", dim_style(sudo)),
        ]),
        InputMode::Password => Line::from(vec![
            Span::styled(" [Enter]Submit", error_style()),
            Span::styled(" │ ", dim_style(sudo)),
            Span::styled("[Esc]Cancel", dim_style(sudo)),
        ]),
    };
    f.render_widget(Paragraph::new(help).style(Style::new().bg(BG)), area);
}

fn render_password_popup(f: &mut Frame, area: Rect, app: &App) {
    let popup_width = area.width.min(50);
    let popup_height = 7;
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);

    f.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::new().fg(RED).bg(BG))
        .title(Line::from(Span::styled(
            " ⚡ SUDO PASSWORD ",
            Style::new().fg(RED).bg(BG).add_modifier(Modifier::BOLD),
        )))
        .style(Style::new().bg(BG));

    let inner = block.inner(popup_rect);
    f.render_widget(block, popup_rect);

    let masked: String = "•".repeat(app.password.len());
    let cursor = if (app.password.len() as u16) < inner.width.saturating_sub(2) {
        app.password.len() as u16
    } else {
        inner.width.saturating_sub(2)
    };

    let content = vec![
        Line::from(Span::styled(
            "  Enter sudo password:",
            Style::new().fg(YELLOW).bg(BG),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", Style::new().fg(BRIGHT_GREEN).bg(BG)),
            Span::styled(masked, Style::new().fg(GREEN).bg(BG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Enter] Submit  ", Style::new().fg(RED).bg(BG)),
            Span::styled("[Esc] Cancel", dim_style(false)),
        ]),
    ];

    let para = Paragraph::new(Text::from(content))
        .style(Style::new().bg(BG));
    f.render_widget(para, inner);

    f.set_cursor_position((inner.x + 2 + cursor, inner.y + 2));
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn short_id(id: &str) -> String {
    if id.len() > 8 { id[..8].to_string() } else { id.to_string() }
}

fn short_datetime(rfc3339: &str) -> String {
    if rfc3339.len() >= 19 {
        rfc3339[..19].replace('T', " ")
    } else {
        rfc3339.to_string()
    }
}

fn render_help_popup(f: &mut Frame, area: Rect) {
    let popup_width = area.width.min(56);
    let popup_height = 21;
    let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

    let popup_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
    f.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::new().fg(GREEN))
        .title(Line::from(Span::styled(
            " ⌨  KEYBINDINGS ",
            Style::new().fg(GREEN).bg(BG).add_modifier(Modifier::BOLD),
        )))
        .style(Style::new().bg(BG));

    let inner = block.inner(popup_rect);
    f.render_widget(block, popup_rect);

    let content = vec![
        Line::from(vec![
            Span::styled(" Navigation ", Style::new().fg(YELLOW).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled(" Output ", Style::new().fg(YELLOW).bg(BG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  ↑/↓ j/k", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("  Move    ", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("Ctrl+↑/↓", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled(" Scroll", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("  g", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("          Top     ", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("PgUp/Dn", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("  Jump 5", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("  G", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("          Bottom", dim_style(false)),
        ]),
        Line::from(vec![Span::styled("", dim_style(false))]),
        Line::from(vec![
            Span::styled(" Commands ", Style::new().fg(YELLOW).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled(" Modes ", Style::new().fg(YELLOW).bg(BG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Enter", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("      Run cmd    ", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("r", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("       Run", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("  S", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("          Sudo     ", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("w", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("       Add", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("  ? / F1", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("     Help     ", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("/ or s", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("  Search", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("                     ", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("t", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("       Tag", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("", dim_style(false)),
            Span::styled(" │ ", dim_style(false)),
            Span::styled("n / a", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("     Note", dim_style(false)),
        ]),
        Line::from(vec![Span::styled("", dim_style(false))]),
        Line::from(vec![
            Span::styled(" Manage ", Style::new().fg(YELLOW).bg(BG).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  b", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("          Bookmark", dim_style(false)),
        ]),
        Line::from(vec![
            Span::styled("  d", Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)),
            Span::styled("          Delete", dim_style(false)),
        ]),
        Line::from(vec![Span::styled("", dim_style(false))]),
        Line::from(vec![
            Span::styled("  [Esc] Close", Style::new().fg(RED).bg(BG).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let para = Paragraph::new(Text::from(content))
        .style(Style::new().bg(BG));
    f.render_widget(para, inner);
}
