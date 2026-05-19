use crate::config::paths;
use crate::db::schema;
use anyhow::{Context, Result};
use crossterm::cursor;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use rusqlite::Connection;
use std::io::{stdout, Write};
use std::time::Duration;

use super::ui;

#[derive(Clone)]
pub struct CommandInfo {
    pub id: String,
    pub command: String,
    pub exit_code: i32,
    pub duration_ms: i64,
    pub captured_at: String,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub is_bookmark: bool,
}

#[derive(Clone, PartialEq)]
pub enum InputMode {
    Tool,
    Run,
    Search,
    Tag,
    Note,
    Password,
    Add,
}

pub struct App {
    pub commands: Vec<CommandInfo>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub input: String,
    pub mode: InputMode,
    pub output: String,
    pub should_quit: bool,
    pub status_msg: String,
    pub list_scroll: usize,
    pub output_scroll: u16,
    pub sudo_mode: bool,
    pub password: String,
    pub show_password_popup: bool,
    pub mode_msg: String,
    pending_run: Option<PendingRun>,
    pub list_inner_height: usize,
    conn: Connection,
}

struct PendingRun {
    command: String,
    password: Option<String>,
}

impl App {
    pub fn new() -> Result<Self> {
        let db_path = paths::db_path()?;
        let conn = Connection::open(&db_path)?;
        schema::initialize(&conn)?;

        let mut app = App {
            commands: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            input: String::new(),
            mode: InputMode::Tool,
            output: String::new(),
            should_quit: false,
            status_msg: String::new(),
            list_scroll: 0,
            output_scroll: 0,
            sudo_mode: false,
            password: String::new(),
            show_password_popup: false,
            mode_msg: String::new(),
            pending_run: None,
            list_inner_height: 0,
            conn,
        };
        app.load_commands()?;
        app.status_msg = format!("{} commands loaded", app.commands.len());
        Ok(app)
    }

    fn load_commands(&mut self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.command, c.exit_code, c.duration_ms, c.captured_at,
                    a.note, COALESCE(a.is_bookmark, 0)
             FROM commands c
             LEFT JOIN annotations a ON a.command_id = c.id
             ORDER BY c.captured_at DESC
             LIMIT 500",
        )?;

        let mut commands = Vec::new();
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let command: String = row.get(1)?;
            let exit_code: i32 = row.get(2)?;
            let duration_ms: i64 = row.get(3)?;
            let captured_at: String = row.get(4)?;
            let note: Option<String> = row.get(5)?;
            let bm: i32 = row.get(6)?;
            Ok((id, command, exit_code, duration_ms, captured_at, note, bm != 0))
        })?;

        for row in rows {
            let (id, command, exit_code, duration_ms, captured_at, note, is_bookmark) = row?;
            let tags = crate::cli::query::get_tags_for_command(&self.conn, &id).unwrap_or_default();
            commands.push(CommandInfo {
                id,
                command,
                exit_code,
                duration_ms,
                captured_at,
                tags,
                note,
                is_bookmark,
            });
        }

        self.commands = commands;
        self.filtered = (0..self.commands.len()).collect();
        self.selected = 0;
        self.list_scroll = 0;
        Ok(())
    }

    pub fn selected_command(&self) -> Option<&CommandInfo> {
        self.filtered.get(self.selected).and_then(|&i| self.commands.get(i))
    }

    fn reload_and_select(&mut self, _idx: usize) -> Result<()> {
        self.load_commands()?;
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        Ok(())
    }

    fn capture_output(cmd: &str) -> Result<()> {
        let db_path = paths::db_path()?;
        let conn = Connection::open(&db_path)?;
        schema::initialize(&conn)?;
        crate::capture::capture_command(cmd, 0, 0, "", "tui")?;
        Ok(())
    }

    fn run_shell_suspended(
        cmd: &str,
        password: Option<&str>,
    ) -> Result<(String, i32)> {
        if let Some(pw) = password {
            let mut auth = std::process::Command::new("sudo")
                .arg("-S")
                .arg("-v")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .context("failed to spawn sudo -v")?;
            if let Some(mut stdin) = auth.stdin.take() {
                let _ = writeln!(stdin, "{}", pw);
            }
            auth.wait()?;
        }

        match Self::run_with_script(cmd) {
            Ok(result) => Ok(result),
            Err(_) => {
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .stdin(std::process::Stdio::inherit())
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status()
                    .context("failed to execute command")?;
                let code = status.code().unwrap_or(-1);
                Ok((format!("[exit code: {}]", code), code))
            }
        }
    }

    fn run_with_script(cmd: &str) -> Result<(String, i32)> {
        let out_dir = std::env::temp_dir();
        let out_path = out_dir.join(format!("cmdstr_out_{}", std::process::id()));
        let out_str = out_path.to_str().context("invalid temp path")?;

        let status = std::process::Command::new("script")
            .args(["-q", "-e", "-c", cmd, out_str])
            .stdin(std::process::Stdio::inherit())
            .status()
            .context("script command failed")?;

        let recorded = match std::fs::read_to_string(&out_path) {
            Ok(s) => {
                let _ = std::fs::remove_file(&out_path);
                s
            }
            Err(_) => String::new(),
        };

        let clean: String = recorded.chars().filter(|&c| c != '\r').collect();
        let output = Self::strip_script_header(&clean);

        let code = status.code().unwrap_or(-1);
        Ok((output, code))
    }

    fn strip_script_header(s: &str) -> String {
        let mut lines: Vec<&str> = s.lines().collect();
        if lines.first().is_some_and(|l| l.contains("Script started")) {
            lines.remove(0);
        }
        if lines.last().is_some_and(|l| l.contains("Script done")) {
            lines.pop();
        }
        lines.join("\n")
    }

    pub fn run_selected(&mut self) -> Result<()> {
        let cmd_str = self.selected_command().map(|c| c.command.clone());
        if let Some(cmd) = cmd_str {
            let effective_cmd = if self.sudo_mode && !cmd.starts_with("sudo ") {
                format!("sudo {}", cmd)
            } else {
                cmd.clone()
            };

            if effective_cmd.starts_with("sudo ") {
                self.mode = InputMode::Password;
                self.input = effective_cmd;
                self.password.clear();
                self.show_password_popup = true;
                self.mode_msg = "PASSWORD".to_string();
                return Ok(());
            }

            self.pending_run = Some(PendingRun { command: effective_cmd, password: None });
        }
        Ok(())
    }

    pub fn run_input_cmd(&mut self, cmd: &str) -> Result<()> {
        let effective_cmd = if self.sudo_mode && !cmd.starts_with("sudo ") {
            format!("sudo {}", cmd)
        } else {
            cmd.to_string()
        };

        if effective_cmd.starts_with("sudo ") {
            self.mode = InputMode::Password;
            self.input = effective_cmd;
            self.password.clear();
            self.show_password_popup = true;
            self.mode_msg = "PASSWORD".to_string();
            return Ok(());
        }

        self.pending_run = Some(PendingRun { command: effective_cmd, password: None });
        Ok(())
    }

    pub fn toggle_bookmark(&mut self) -> Result<()> {
        if let Some(cmd) = self.selected_command() {
            let new_bm = !cmd.is_bookmark;
            self.conn.execute(
                "INSERT INTO annotations (command_id, note, is_bookmark)
                 VALUES (?1, '', ?2)
                 ON CONFLICT(command_id) DO UPDATE SET is_bookmark = ?2",
                rusqlite::params![cmd.id, new_bm as i32],
            )?;
            if let Some(&idx) = self.filtered.get(self.selected) {
                if let Some(c) = self.commands.get_mut(idx) {
                    c.is_bookmark = new_bm;
                }
            }
            self.status_msg = if new_bm { "★ Bookmarked" } else { "Bookmark removed" }.to_string();
        }
        Ok(())
    }

    pub fn set_tags(&mut self, tags_str: &str) -> Result<()> {
        if let Some(cmd) = self.selected_command() {
            self.conn.execute("DELETE FROM command_tags WHERE command_id = ?1", rusqlite::params![cmd.id])?;

            for t in tags_str.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
                let tag = t.to_lowercase();
                self.conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", rusqlite::params![&tag])?;
                let tag_id: i64 = self.conn.query_row(
                    "SELECT id FROM tags WHERE name = ?1", rusqlite::params![&tag], |r| r.get(0),
                )?;
                self.conn.execute(
                    "INSERT OR IGNORE INTO command_tags (command_id, tag_id) VALUES (?1, ?2)",
                    rusqlite::params![cmd.id, tag_id],
                )?;
            }

            if let Some(&idx) = self.filtered.get(self.selected) {
                if let Some(c) = self.commands.get_mut(idx) {
                    c.tags = tags_str.split(',').map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect();
                }
            }
            self.status_msg = format!("Tagged: {tags_str}");
        }
        Ok(())
    }

    pub fn set_note(&mut self, note: &str) -> Result<()> {
        if let Some(cmd) = self.selected_command() {
            self.conn.execute(
                "INSERT INTO annotations (command_id, note, is_bookmark)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(command_id) DO UPDATE SET note = ?2",
                rusqlite::params![cmd.id, note, cmd.is_bookmark as i32],
            )?;
            if let Some(&idx) = self.filtered.get(self.selected) {
                if let Some(c) = self.commands.get_mut(idx) {
                    c.note = Some(note.to_string());
                }
            }
            self.status_msg = "Note saved ✓".to_string();
        }
        Ok(())
    }

    pub fn delete_command(&mut self) -> Result<()> {
        if let Some(cmd) = self.selected_command() {
            self.conn.execute("DELETE FROM commands WHERE id = ?1", rusqlite::params![cmd.id])?;
            self.reload_and_select(0)?;
            self.status_msg = "Command deleted".to_string();
        }
        Ok(())
    }

    pub fn search(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered = (0..self.commands.len()).collect();
        } else {
            let q = query.to_lowercase();
            self.filtered = self.commands.iter().enumerate()
                .filter(|(_, cmd)| {
                    cmd.command.to_lowercase().contains(&q)
                        || cmd.tags.iter().any(|t| t.contains(&q))
                        || cmd.note.as_deref().unwrap_or("").to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }
        self.selected = 0;
        self.list_scroll = 0;
        self.status_msg = format!("{} matches", self.filtered.len());
    }

    fn scroll_output(&mut self, delta: i16) {
        let max = self.output_line_count().saturating_sub(1);
        let new_scroll = (self.output_scroll as i16 + delta).max(0).min(max as i16);
        self.output_scroll = new_scroll as u16;
    }

    pub fn output_line_count(&self) -> u16 {
        if self.output.is_empty() {
            return 0;
        }
        self.output.lines().count() as u16
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let tick_rate = Duration::from_millis(50);

        while !self.should_quit {
                // Execute any pending command with TUI suspend/resume
            if let Some(run) = self.pending_run.take() {
                // Suspend the TUI
                let mut suspend = || -> Result<()> {
                    disable_raw_mode()?;
                    terminal.backend_mut().execute(LeaveAlternateScreen)?;
                    terminal.backend_mut().execute(cursor::Show)?;
                    terminal.backend_mut().flush()?;
                    Ok(())
                };
                if let Err(e) = suspend() {
                    self.status_msg = format!("Suspend error: {e}");
                    // Try to recover
                    let _ = enable_raw_mode();
                    let _ = terminal.backend_mut().execute(EnterAlternateScreen);
                }

                // Run the command with the real terminal
                let (output, exit_code) = Self::run_shell_suspended(
                    &run.command,
                    run.password.as_deref(),
                ).unwrap_or_else(|e| (format!("Error: {e}"), -1));

                // Capture to DB
                if let Err(e) = Self::capture_output(&run.command) {
                    eprintln!("Failed to capture: {e}");
                }

                // Reset terminal to a known good state
                let _ = std::process::Command::new("stty").arg("sane").status();

                // Re-enter the TUI
                let mut resume = || -> Result<()> {
                    enable_raw_mode()?;
                    terminal.backend_mut().execute(EnterAlternateScreen)?;
                    terminal.backend_mut().execute(cursor::Hide)?;
                    terminal.clear()?;
                    Ok(())
                };
                if let Err(e) = resume() {
                    self.status_msg = format!("Resume error: {e}");
                    self.should_quit = true;
                }

                // Update state
                self.output = output;
                self.status_msg = format!("Command exited: {}", exit_code);
                self.output_scroll = 0;
                if let Err(e) = self.reload_and_select(0) {
                    eprintln!("Failed to reload: {e}");
                }
            }

            let _ = terminal.draw(|f| ui::render(f, self));

            if event::poll(tick_rate)? {
                match self.mode.clone() {
                    InputMode::Tool => {
                        if let Err(e) = self.handle_tool_event(event::read()?) {
                            self.status_msg = format!("Error: {e}");
                        }
                    }
                    _ => {
                        if let Err(e) = self.handle_input_event(event::read()?) {
                            self.status_msg = format!("Error: {e}");
                        }
                    }
                }
            }
        }

        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;
        terminal.backend_mut().flush()?;
        Ok(())
    }

    fn handle_tool_event(&mut self, ev: Event) -> Result<()> {
        if let Event::Key(KeyEvent { code, kind, modifiers, .. }) = ev {
            if kind != KeyEventKind::Press && kind != KeyEventKind::Repeat {
                return Ok(());
            }

            match code {
                KeyCode::Char('q') if modifiers == KeyModifiers::NONE => self.should_quit = true,
                KeyCode::Char('c') if modifiers == KeyModifiers::CONTROL => self.should_quit = true,

                KeyCode::Char('j') | KeyCode::Down
                    if self.selected + 1 < self.filtered.len() => {
                        self.selected += 1;
                        self.ensure_visible();
                    }
                KeyCode::Char('k') | KeyCode::Up
                    if self.selected > 0 => {
                        self.selected -= 1;
                        self.ensure_visible();
                    }
                KeyCode::Char('g') => { self.selected = 0; self.list_scroll = 0; }
                KeyCode::Char('G') => {
                    self.selected = self.filtered.len().saturating_sub(1);
                    self.list_scroll = self.filtered.len().saturating_sub(1);
                }

                KeyCode::Char('r') => {
                    self.input.clear();
                    self.mode_msg = "RUN".to_string();
                    self.mode = InputMode::Run;
                }
                KeyCode::Char('w') => {
                    self.input.clear();
                    self.mode_msg = "ADD".to_string();
                    self.mode = InputMode::Add;
                }
                KeyCode::Char('/') | KeyCode::Char('s') => {
                    self.input.clear();
                    self.mode_msg = "SEARCH".to_string();
                    self.mode = InputMode::Search;
                }
                KeyCode::Char('t')
                    if self.selected_command().is_some() => {
                        let tags = self.selected_command().map(|c| c.tags.join(", ")).unwrap_or_default();
                        self.input = tags;
                        self.mode_msg = "TAG".to_string();
                        self.mode = InputMode::Tag;
                    }
                KeyCode::Char('n') | KeyCode::Char('a')
                    if self.selected_command().is_some() => {
                        let note = self.selected_command().and_then(|c| c.note.clone()).unwrap_or_default();
                        self.input = note;
                        self.mode_msg = "NOTE".to_string();
                        self.mode = InputMode::Note;
                    }
                KeyCode::Char('b') => self.toggle_bookmark()?,
                KeyCode::Char('d') => self.delete_command()?,

                KeyCode::Char('S') | KeyCode::Char('U') => {
                    self.sudo_mode = !self.sudo_mode;
                    self.status_msg = if self.sudo_mode {
                        "SUDO MODE ON — all commands run as root".to_string()
                    } else {
                        "SUDO MODE OFF".to_string()
                    };
                }

                KeyCode::Enter => self.run_selected()?,

                // Output scrolling
                KeyCode::Up if modifiers == KeyModifiers::CONTROL => self.scroll_output(-1),
                KeyCode::Down if modifiers == KeyModifiers::CONTROL => self.scroll_output(1),
                KeyCode::PageUp => self.scroll_output(-5),
                KeyCode::PageDown => self.scroll_output(5),

                _ => {}
            }
        }
        Ok(())
    }

    fn handle_input_event(&mut self, ev: Event) -> Result<()> {
        if let Event::Key(KeyEvent { code, kind, .. }) = ev {
            if kind != KeyEventKind::Press {
                return Ok(());
            }

            match code {
                KeyCode::Esc => {
                    self.mode = InputMode::Tool;
                    self.mode_msg.clear();
                    self.input.clear();
                    self.password.clear();
                    self.show_password_popup = false;
                }
                KeyCode::Enter => {
                    let input = self.input.trim().to_string();
                    let mode = self.mode.clone();
                    self.mode = InputMode::Tool;
                    self.mode_msg.clear();

                    match mode {
                        InputMode::Run
                            if !input.is_empty() => {
                                self.run_input_cmd(&input)?;
                            }
                        InputMode::Search => {
                            self.search(&input);
                        }
                        InputMode::Tag
                            if !input.is_empty() => {
                                self.set_tags(&input)?;
                            }
                        InputMode::Note
                            if !input.is_empty() => {
                                self.set_note(&input)?;
                            }
                        InputMode::Add
                            if !input.is_empty() => {
                                if let Err(e) = crate::capture::capture_command(&input, 0, 0, "", "tui") {
                                    self.status_msg = format!("Failed to add: {e}");
                                } else {
                                    self.status_msg = format!("Added: {}", &input);
                                    self.reload_and_select(0)?;
                                }
                            }
                        InputMode::Password => {
                            let pw = std::mem::take(&mut self.password);
                            self.show_password_popup = false;
                            if !pw.is_empty() && !input.is_empty() {
                                self.pending_run = Some(PendingRun {
                                    command: input,
                                    password: Some(pw),
                                });
                                self.status_msg = "Starting...".to_string();
                            }
                        }
                        _ => {}
                    }
                    self.input.clear();
                }
                KeyCode::Char(c) => {
                    if self.mode == InputMode::Password {
                        self.password.push(c);
                    } else {
                        self.input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if self.mode == InputMode::Password {
                        self.password.pop();
                    } else {
                        self.input.pop();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn ensure_visible(&mut self) {
        let list_height = self.list_inner_height.max(1);
        if self.selected < self.list_scroll {
            self.list_scroll = self.selected;
        } else if self.selected >= self.list_scroll + list_height {
            self.list_scroll = self.selected.saturating_sub(list_height).saturating_add(1);
        }
    }
}
