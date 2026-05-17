use crate::config::paths;
use crate::db::schema;
use anyhow::{Context, Result};
use rusqlite::Connection;

pub fn execute(tag: &str) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    let mut stmt = conn.prepare(
        "SELECT DISTINCT c.command
         FROM commands c
         JOIN command_tags ct ON ct.command_id = c.id
         JOIN tags t ON t.id = ct.tag_id
         WHERE t.name = ?1
         ORDER BY c.captured_at DESC
         LIMIT 1",
    )?;

    let command: Option<String> = stmt
        .query_map(rusqlite::params![tag], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .next();

    match command {
        Some(cmd) => {
            println!("Running: {}", cmd);
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .status()
                .context("failed to execute command")?;
            if !status.success() {
                eprintln!("Command exited with status: {}", status);
            }
            Ok(())
        }
        None => anyhow::bail!("No command found with tag '{}'", tag),
    }
}
