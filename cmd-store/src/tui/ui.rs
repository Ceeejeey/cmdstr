use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, InputMode};

const GREEN: Color = Color::Rgb(0x00, 0xff, 0x00);
const DARK_GREEN: Color = Color::Rgb(0x00, 0x55, 0x00);
const BRIGHT_GREEN: Color = Color::Rgb(0x55, 0xff, 0x55);
const BG: Color = Color::Rgb(0x00, 0x00, 0x00);
const RED: Color = Color::Rgb(0xff, 0x33, 0x33);
const YELLOW: Color = Color::Rgb(0xff, 0xcc, 0x00);

fn border_style() -> Style {
    Style::new().fg(DARK_GREEN).bg(BG)
}

fn base_style() -> Style {
    Style::new().fg(GREEN).bg(BG)
}

fn highlight_style() -> Style {
    Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)
}

fn dim_style() -> Style {
    Style::new().fg(DARK_GREEN).bg(BG)
}

fn error_style() -> Style {
    Style::new().fg(RED).bg(BG).add_modifier(Modifier::BOLD)
}

fn title_style() -> Style {
    Style::new().fg(BRIGHT_GREEN).bg(BG).add_modifier(Modifier::BOLD)
}

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

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
    render_command_list(f, chunks[1], app);
    render_detail(f, chunks[2], app);
    render_input(f, chunks[3], app);
    render_help(f, chunks[4], app);
}

fn render_title(f: &mut Frame, area: Rect, _app: &App) {
    let title = Line::from(Span::styled(
        " ⚡ cmdstr TUI ",
        Style::new().fg(GREEN).bg(BG).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(Paragraph::new(title).style(base_style()), area);
}

fn render_command_list(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style())
        .title(Line::from(Span::styled(" COMMANDS ", title_style())))
        .style(Style::new().bg(BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.filtered.is_empty() {
        let empty = Paragraph::new(Text::styled("  No commands found.", dim_style()))
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

            let bm = if cmd.is_bookmark { " ★" } else { "   " };

            let tag_str = if cmd.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", cmd.tags.join(","))
            };

            let cmd_display = truncate(&cmd.command, (inner.width as usize).saturating_sub(20));

            let line = Line::from(vec![
                Span::styled(status, Style::new().fg(status_color).bg(BG)),
                Span::styled(" ", base_style()),
                Span::styled(bm, Style::new().fg(YELLOW).bg(BG)),
                Span::styled(" ", base_style()),
                if is_selected {
                    Span::styled(cmd_display, highlight_style())
                } else {
                    Span::styled(cmd_display, base_style())
                },
                Span::styled(tag_str, dim_style()),
            ]);

            if is_selected {
                ListItem::new(line).style(Style::new().bg(Color::Rgb(0x00, 0x22, 0x00)))
            } else {
                ListItem::new(line).style(Style::new().bg(BG))
            }
        })
        .collect();

    let list = List::new(items).style(Style::new().bg(BG));
    f.render_widget(list, inner);
}

fn render_detail(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style())
        .title(Line::from(Span::styled(" DETAILS / OUTPUT ", title_style())))
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

    render_command_details(f, left, app);
    render_output(f, right, app);
}

fn render_command_details(f: &mut Frame, area: Rect, app: &App) {
    let mut text = Vec::new();

    if let Some(cmd) = app.selected_command() {
        text.push(Line::from(vec![
            Span::styled("ID:      ", dim_style()),
            Span::styled(short_id(&cmd.id), base_style()),
        ]));
        let status_text = if cmd.exit_code == 0 {
            "OK".to_string()
        } else {
            format!("FAIL ({})", cmd.exit_code)
        };
        text.push(Line::from(vec![
            Span::styled("Status:  ", dim_style()),
            Span::styled(
                status_text,
                if cmd.exit_code == 0 { Style::new().fg(GREEN).bg(BG) } else { error_style() },
            ),
        ]));
        text.push(Line::from(vec![
            Span::styled("Time:    ", dim_style()),
            Span::styled(format!("{} ms", cmd.duration_ms), base_style()),
        ]));
        text.push(Line::from(vec![
            Span::styled("Date:    ", dim_style()),
            Span::styled(short_datetime(&cmd.captured_at), base_style()),
        ]));
        text.push(Line::from(vec![
            Span::styled("Tags:    ", dim_style()),
            if cmd.tags.is_empty() {
                Span::styled("(none)", dim_style())
            } else {
                Span::styled(cmd.tags.join(", "), base_style())
            },
        ]));
        text.push(Line::from(vec![
            Span::styled("Bookmark:", dim_style()),
            if cmd.is_bookmark {
                Span::styled(" ★", YELLOW)
            } else {
                Span::styled(" no", dim_style())
            },
        ]));
        text.push(Line::from(Span::styled("─".repeat(area.width as usize), dim_style())));
        if let Some(note) = &cmd.note {
            text.push(Line::from(vec![
                Span::styled("Note:    ", dim_style()),
                Span::styled(note.clone(), Style::new().fg(YELLOW).bg(BG)),
            ]));
        }
        text.push(Line::from(Span::styled("─".repeat(area.width as usize), dim_style())));
        text.push(Line::from(Span::styled(
            truncate(&cmd.command, area.width as usize),
            Style::new().fg(GREEN).bg(BG).add_modifier(Modifier::DIM),
        )));
    } else {
        text.push(Line::from(Span::styled("  Select a command from the list", dim_style())));
    }

    let para = Paragraph::new(Text::from(text))
        .style(Style::new().bg(BG))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_output(f: &mut Frame, area: Rect, app: &App) {
    let output = if app.output.is_empty() {
        Text::styled("  Press Enter to run or 'r' to type a command", dim_style())
    } else {
        Text::from(Line::from(Span::styled(&app.output, base_style())))
    };

    let para = Paragraph::new(output)
        .style(Style::new().bg(BG))
        .scroll((app.output_scroll, 0))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style())
        .style(Style::new().bg(BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let prompt = match app.mode {
        InputMode::Normal => "> ",
        InputMode::Command => "> ",
        InputMode::Search => "/ ",
        InputMode::Tag => "tags> ",
        InputMode::Note => "note> ",
    };

    let cursor_visible = matches!(app.mode, InputMode::Command | InputMode::Search | InputMode::Tag | InputMode::Note);

    let (prompt_style, input_style) = match app.mode {
        InputMode::Search => (Style::new().fg(YELLOW).bg(BG), Style::new().fg(YELLOW).bg(BG)),
        InputMode::Tag => (Style::new().fg(BRIGHT_GREEN).bg(BG), Style::new().fg(BRIGHT_GREEN).bg(BG)),
        InputMode::Note => (Style::new().fg(YELLOW).bg(BG), Style::new().fg(YELLOW).bg(BG)),
        _ => (Style::new().fg(GREEN).bg(BG), Style::new().fg(GREEN).bg(BG)),
    };

    let input_display = if app.mode == InputMode::Command && app.input.is_empty() {
        "type a command to run...".to_string()
    } else {
        app.input.clone()
    };

    let line = Line::from(vec![
        Span::styled(prompt, prompt_style),
        Span::styled(&input_display, input_style),
    ]);

    let para = Paragraph::new(line).style(Style::new().bg(BG));
    f.render_widget(para, inner);

    if cursor_visible {
        let x = inner.x + prompt.len() as u16 + app.input.len() as u16;
        let x = x.min(inner.x + inner.width.saturating_sub(1));
        f.set_cursor_position((x, inner.y));
    }
}

fn render_help(f: &mut Frame, area: Rect, _app: &App) {
    let help = Line::from(vec![
        Span::styled(" [↑↓/j/k]Nav", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[Enter]Run", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[r]Type", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[/]Search", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[t]Tag", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[n]Note", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[b]Bookmark", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[d]Del", dim_style()),
        Span::styled(" │ ", DARK_GREEN),
        Span::styled("[q]Quit", error_style()),
    ]);
    f.render_widget(Paragraph::new(help).style(Style::new().bg(BG)), area);
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
