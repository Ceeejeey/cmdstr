use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
pub struct QueryArgs {
    /// Search term (fuzzy matched against command text)
    pub query: Option<String>,

    /// Filter by tag
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Only commands from the last N hours
    #[arg(long)]
    pub last: Option<u64>,

    /// Only failed commands (exit code != 0)
    #[arg(long)]
    pub failed: bool,

    /// Show most frequent commands
    #[arg(long)]
    pub freq: bool,

    /// Bookmarked commands only
    #[arg(long)]
    pub bookmarks: bool,

    /// Limit results
    #[arg(short, long, default_value = "30")]
    pub limit: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn execute(args: &QueryArgs) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    if args.freq {
        return list_frequent(&conn, args.limit, args.json);
    }

    let mut sql = String::from(
        "SELECT DISTINCT c.id, c.command, c.cwd, c.exit_code, c.duration_ms,
                c.captured_at, c.hostname, a.note, a.is_bookmark
         FROM commands c
         LEFT JOIN annotations a ON a.command_id = c.id
         LEFT JOIN command_tags ct ON ct.command_id = c.id
         LEFT JOIN tags t ON t.id = ct.tag_id
         WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = &args.query {
        params.push(Box::new(format!("%{}%", q)));
        sql.push_str(&format!(" AND c.command LIKE ?{}", params.len()));
    }

    if let Some(tag) = &args.tag {
        params.push(Box::new(tag.clone()));
        sql.push_str(&format!(" AND t.name = ?{}", params.len()));
    }

    if let Some(hours) = args.last {
        sql.push_str(&format!(
            " AND c.captured_at >= datetime('now', '-{} hours')",
            hours
        ));
    }

    if args.failed {
        sql.push_str(" AND c.exit_code != 0");
    }

    if args.bookmarks {
        sql.push_str(" AND a.is_bookmark = 1");
    }

    sql.push_str(" ORDER BY c.captured_at DESC");
    sql.push_str(&format!(" LIMIT {}", args.limit));

    let mut stmt = conn.prepare(&sql)?;

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    #[allow(unused_mut)]
    let mut rows = stmt.query(param_refs.as_slice())?;
    let mut results = Vec::new();

    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let cmd: String = row.get(1)?;
        let cwd: String = row.get(2)?;
        let exit_code: i32 = row.get(3)?;
        let duration_ms: i64 = row.get(4)?;
        let captured_at: String = row.get(5)?;
        let hostname: String = row.get(6)?;
        let note: Option<String> = row.get(7)?;
        let is_bookmark_i32: Option<i32> = row.get(8)?;
        let is_bookmark = is_bookmark_i32.unwrap_or(0) != 0;

        let tags = get_tags_for_command(&conn, &id)?;

        results.push(serde_json::json!({
            "id": id,
            "command": cmd,
            "cwd": cwd,
            "exit_code": exit_code,
            "duration_ms": duration_ms,
            "captured_at": captured_at,
            "hostname": hostname,
            "note": note,
            "bookmark": is_bookmark,
            "tags": tags,
        }));
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            println!("No commands found.");
            return Ok(());
        }
        for entry in &results {
            let emoji = if entry["exit_code"].as_i64() != Some(0) {
                "✗"
            } else {
                "✓"
            };
            let tags = entry["tags"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();

            let tag_str = if tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", tags)
            };

            let note_str = entry["note"]
                .as_str()
                .filter(|n| !n.is_empty())
                .map(|n| format!("  # {}", n))
                .unwrap_or_default();

            println!(
                "  {:>6} │ {} {} │ {}{}{}",
                entry["id"]
                    .as_str()
                    .map(|s| &s[..s.len().min(6)])
                    .unwrap_or("?"),
                emoji,
                entry["captured_at"]
                    .as_str()
                    .unwrap_or("?"),
                entry["command"].as_str().unwrap_or("?"),
                tag_str,
                note_str,
            );
        }
    }

    Ok(())
}

pub fn get_tags_for_command(conn: &Connection, command_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t
         JOIN command_tags ct ON ct.tag_id = t.id
         WHERE ct.command_id = ?1",
    )?;
    let tags: Vec<String> = stmt
        .query_map(rusqlite::params![command_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tags)
}

fn list_frequent(conn: &Connection, limit: usize, json: bool) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT command, count FROM command_freq ORDER BY count DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
        let cmd: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((cmd, count))
    })?;

    let mut results = Vec::new();
    for row in rows {
        let (cmd, count) = row?;
        results.push(serde_json::json!({"command": cmd, "count": count}));
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        println!("{:<6} │ Command", "Count");
        println!("{}", "─".repeat(50));
        for entry in &results {
            println!(
                "  {:>4} │ {}",
                entry["count"].as_i64().unwrap_or(0),
                entry["command"].as_str().unwrap_or("")
            );
        }
    }
    Ok(())
}
