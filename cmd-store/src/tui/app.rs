use crate::config::paths;
use crate::db::schema;
use anyhow::{Context, Result};
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
    Normal,
    Command,
    Search,
    Tag,
    Note,
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
    conn: Connection,
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
            mode: InputMode::Normal,
            output: String::new(),
            should_quit: false,
            status_msg: String::new(),
            list_scroll: 0,
            output_scroll: 0,
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

    fn run_shell(cmd: &str) -> Result<String> {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .context("failed to execute command")?;

        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            if !result.is_empty() { result.push('\n'); }
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        if !output.status.success() {
            result.push_str(&format!("\n[exit code: {}]", output.status.code().unwrap_or(-1)));
        }
        Ok(result)
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

    fn run_command(cmd: &str) -> Result<String> {
        let output = Self::run_shell(cmd)?;
        if let Err(e) = Self::capture_output(cmd) {
            eprintln!("Failed to capture: {e}");
        }
        Ok(output)
    }

    pub fn run_selected(&mut self) -> Result<()> {
        let cmd_str = self.selected_command().map(|c| c.command.clone());
        if let Some(cmd) = cmd_str {
            self.output = Self::run_shell(&cmd)?;
            self.status_msg = format!("Ran: {}", &cmd);
            self.output_scroll = 0;
        }
        Ok(())
    }

    pub fn run_input_cmd(&mut self, cmd: &str) -> Result<()> {
        let output = Self::run_command(cmd)?;
        self.output = output;
        self.status_msg = format!("Ran: {cmd}");
        self.output_scroll = 0;
        self.reload_and_select(0)?;
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

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let tick_rate = Duration::from_millis(50);

        while !self.should_quit {
            terminal.draw(|f| ui::render(f, &self))?;

            if event::poll(tick_rate)? {
                match self.mode.clone() {
                    InputMode::Normal => self.handle_normal_event(event::read()?)?,
                    _ => self.handle_input_event(event::read()?)?,
                }
            }
        }

        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;
        terminal.backend_mut().flush()?;
        Ok(())
    }

    fn handle_normal_event(&mut self, ev: Event) -> Result<()> {
        if let Event::Key(KeyEvent { code, kind, modifiers, .. }) = ev {
            if kind != KeyEventKind::Press && kind != KeyEventKind::Repeat {
                return Ok(());
            }

            match code {
                KeyCode::Char('q') if modifiers == KeyModifiers::NONE => self.should_quit = true,
                KeyCode::Char('c') if modifiers == KeyModifiers::CONTROL => self.should_quit = true,
                KeyCode::Esc => self.should_quit = true,

                KeyCode::Char('j') | KeyCode::Down => {
                    if self.selected + 1 < self.filtered.len() {
                        self.selected += 1;
                        self.ensure_visible();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if self.selected > 0 {
                        self.selected -= 1;
                        self.ensure_visible();
                    }
                }
                KeyCode::Char('g') => { self.selected = 0; self.list_scroll = 0; }
                KeyCode::Char('G') => {
                    self.selected = self.filtered.len().saturating_sub(1);
                    self.list_scroll = self.filtered.len().saturating_sub(1);
                }

                KeyCode::Enter => self.run_selected()?,

                KeyCode::Char('r') => {
                    self.input.clear();
                    self.mode = InputMode::Command;
                }
                KeyCode::Char('/') => {
                    self.input.clear();
                    self.mode = InputMode::Search;
                }
                KeyCode::Char('t') => {
                    if self.selected_command().is_some() {
                        let tags = self.selected_command().map(|c| c.tags.join(", ")).unwrap_or_default();
                        self.input = tags;
                        self.mode = InputMode::Tag;
                    }
                }
                KeyCode::Char('n') => {
                    if self.selected_command().is_some() {
                        let note = self.selected_command().and_then(|c| c.note.clone()).unwrap_or_default();
                        self.input = note;
                        self.mode = InputMode::Note;
                    }
                }
                KeyCode::Char('b') => self.toggle_bookmark()?,
                KeyCode::Char('d') => self.delete_command()?,

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
                    self.mode = InputMode::Normal;
                    self.input.clear();
                }
                KeyCode::Enter => {
                    let input = self.input.trim().to_string();
                    let mode = self.mode.clone();
                    self.mode = InputMode::Normal;

                    match mode {
                        InputMode::Command => {
                            if !input.is_empty() {
                                self.run_input_cmd(&input)?;
                            }
                        }
                        InputMode::Search => {
                            self.search(&input);
                        }
                        InputMode::Tag => {
                            if !input.is_empty() {
                                self.set_tags(&input)?;
                            }
                        }
                        InputMode::Note => {
                            if !input.is_empty() {
                                self.set_note(&input)?;
                            }
                        }
                        _ => {}
                    }
                    self.input.clear();
                }
                KeyCode::Char(c) => {
                    self.input.push(c);
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn ensure_visible(&mut self) {
        let list_height = 10usize;
        if self.selected < self.list_scroll {
            self.list_scroll = self.selected;
        } else if self.selected >= self.list_scroll + list_height {
            self.list_scroll = self.selected.saturating_sub(list_height).saturating_add(1);
        }
    }
}
