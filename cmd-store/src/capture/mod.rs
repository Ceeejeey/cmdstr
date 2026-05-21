use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use chrono::Utc;
use rusqlite::Connection;
use ulid::Ulid;

pub fn capture_command(
    command: &str,
    exit_code: i32,
    duration_ms: i64,
    cwd: &str,
    session_id: &str,
) -> Result<String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Command cannot be empty or whitespace-only");
    }

    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    let id = Ulid::new().to_string();
    let captured_at = Utc::now().to_rfc3339();
    let hostname = whoami::fallible::hostname().unwrap_or_default();

    conn.execute(
        "INSERT INTO commands (id, command, cwd, exit_code, duration_ms, session_id, hostname, captured_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, command, cwd, exit_code, duration_ms, session_id, hostname, captured_at],
    )?;

    // Update frequency table
    let norm = command.trim().to_lowercase();
    conn.execute(
        "INSERT INTO command_freq (command_hash, command, count)
         VALUES (?1, ?2, 1)
         ON CONFLICT(command_hash) DO UPDATE SET count = count + 1",
        rusqlite::params![simple_hash(&norm), &norm],
    )?;

    Ok(id)
}

pub(crate) fn simple_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_hash_deterministic() {
        assert_eq!(simple_hash("hello"), simple_hash("hello"));
    }

    #[test]
    fn test_simple_hash_differs_for_diff_inputs() {
        assert_ne!(simple_hash("hello"), simple_hash("world"));
    }

    #[test]
    fn test_simple_hash_empty_string() {
        let h = simple_hash("");
        assert!(!h.is_empty());
    }

    #[test]
    fn test_simple_hash_different_casing_differs() {
        assert_ne!(simple_hash("Docker"), simple_hash("docker"));
    }

    #[test]
    fn test_simple_hash_repeatable() {
        assert_eq!(
            simple_hash("echo hello"),
            simple_hash("echo hello")
        );
    }
}
