use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
#[command(
    about = "Search, filter, and list recorded terminal commands",
    long_about = "Queries your database of command history. You can search by text queries (fuzzy match), \
                  by specific tag names, runtime failure states, bookmarks, or by duration. Out-of-the-box \
                  support for formatted tabular output or raw, beautiful JSON exports.",
    after_help = "💡 EXAMPLES & FILTERS:\n\n  \
       1. Search for a keyword anywhere in command strings:\n     \
          $ cmdstr search \"git commit\"\n\n  \
       2. Filter commands by tag name:\n     \
          $ cmdstr search --tag release\n\n  \
       3. Find failed commands from the last 24 hours:\n     \
          $ cmdstr search --failed --last 24\n\n  \
       4. View bookmarked commands only:\n     \
          $ cmdstr search --bookmarks\n\n  \
       5. List most frequently used commands across all logs:\n     \
          $ cmdstr search --freq\n\n  \
       6. Export query results in raw JSON format:\n     \
          $ cmdstr search \"npm\" --json --limit 5"
)]
pub struct QueryArgs {
    /// Search query term to match against command text
    #[arg(help = "The text pattern to find. Performs a wildcard match (%query%) on command history.")]
    pub query: Option<String>,

    /// Filter commands by specific tag name
    #[arg(short, long, help = "Only show commands tagged with this specific exact tag name")]
    pub tag: Option<String>,

    /// Filter by hourly timeframe
    #[arg(long, help = "Only list commands executed within the last N hours")]
    pub last: Option<u64>,

    /// Filter for failed commands
    #[arg(long, help = "Only show commands that exited with a non-zero code (exit_code != 0)")]
    pub failed: bool,

    /// Show command frequency analytics instead of log list
    #[arg(long, help = "Display unique commands sorted by execution counts")]
    pub freq: bool,

    /// Filter for bookmarked commands
    #[arg(long, help = "Only show commands that have been flagged as bookmarks")]
    pub bookmarks: bool,

    /// Limit the number of returned results
    #[arg(short, long, default_value = "30", help = "Maximum number of history logs to display")]
    pub limit: usize,

    /// Output results as JSON instead of formatted table
    #[arg(long, help = "Format and export query matches as structured JSON array")]
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
