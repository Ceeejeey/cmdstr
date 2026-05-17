use anyhow::Result;
use rusqlite::Connection;

pub fn initialize(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS commands (
            id          TEXT PRIMARY KEY,
            command     TEXT NOT NULL,
            cwd         TEXT NOT NULL DEFAULT '',
            exit_code   INTEGER NOT NULL DEFAULT 0,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            session_id  TEXT NOT NULL DEFAULT '',
            hostname    TEXT NOT NULL DEFAULT '',
            captured_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tags (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL
        );

        CREATE TABLE IF NOT EXISTS command_tags (
            command_id TEXT NOT NULL REFERENCES commands(id) ON DELETE CASCADE,
            tag_id     INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (command_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS annotations (
            command_id  TEXT PRIMARY KEY REFERENCES commands(id) ON DELETE CASCADE,
            note        TEXT NOT NULL DEFAULT '',
            is_bookmark INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS command_freq (
            command_hash TEXT PRIMARY KEY,
            command      TEXT NOT NULL,
            count        INTEGER NOT NULL DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_commands_captured_at ON commands(captured_at);
        CREATE INDEX IF NOT EXISTS idx_commands_exit_code  ON commands(exit_code);
        CREATE INDEX IF NOT EXISTS idx_commands_session_id ON commands(session_id);
        CREATE INDEX IF NOT EXISTS idx_command_tags_tag_id ON command_tags(tag_id);

        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        ",
    )?;
    Ok(())
}
