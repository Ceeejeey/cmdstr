use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
#[command(
    about = "Associate custom tags with recorded terminal commands",
    long_about = "Attaches tags to a specific command. To locate the target command, you can specify either \
                  a 26-character ULID (ID), an ID prefix (short ID), or a command text substring. If command \
                  text is provided, cmdstr intelligently scans history, resolves exact matches first, and defaults \
                  to the most recently run matching command.",
    after_help = "💡 EXAMPLES & UX TARGETS:\n\n  \
       1. Tag a command using its exact 26-character ULID:\n     \
          $ cmdstr tag 01H6W4A5G6C8D9E8F7A6B5C4D3 dev,work\n\n  \
       2. Tag a command using a short ID prefix (e.g. first 8 characters):\n     \
          $ cmdstr tag 01H6W4A5 quickstart\n\n  \
       3. Tag a command by providing the command text string:\n     \
          $ cmdstr tag \"git commit -m 'fix production bug'\" git,vcs\n\n  \
       4. Tag a command using a common substring or keyword:\n     \
          $ cmdstr tag \"ffmpeg\" video-processing\n     \
          (Binds 'video-processing' tag to the most recent command containing 'ffmpeg')"
)]
pub struct TagArgs {
    /// Command ID (ULID), short ID prefix, or command text substring
    #[arg(help = "The target command's ULID, ID prefix, or command text substring to search for")]
    pub command_id: String,

    /// Tag name(s) (comma-separated list)
    #[arg(help = "Comma-separated list of tags to apply to the resolved command")]
    pub tags: String,
}

pub fn execute(args: &TagArgs) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    let (resolved_id, display_label) = resolve_command_id(&conn, &args.command_id)?;

    let tags: Vec<&str> = args.tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
    apply_tags_to_conn(&conn, &resolved_id, tags.iter().copied())?;

    println!("Tagged {} with: {}", display_label, args.tags);
    Ok(())
}

/// Resolve a user-provided identifier to a command ID.
/// Accepts either a ULID/ID or a command text (searches most recent match).
fn resolve_command_id(conn: &Connection, input: &str) -> Result<(String, String)> {
    // First, try exact ID match
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM commands WHERE id = ?1",
            rusqlite::params![input],
            |r| r.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if exists {
        let short = if input.len() > 8 { &input[..8] } else { input };
        return Ok((input.to_string(), short.to_string()));
    }

    // Also try prefix match for short IDs (user might type first 6-8 chars)
    if input.len() >= 4 && input.chars().all(|c| c.is_alphanumeric()) {
        let prefix_match: Option<String> = conn
            .query_row(
                "SELECT id FROM commands WHERE id LIKE ?1 ORDER BY captured_at DESC LIMIT 1",
                rusqlite::params![format!("{}%", input)],
                |r| r.get(0),
            )
            .ok();

        if let Some(id) = prefix_match {
            let short = if id.len() > 8 { &id[..8] } else { &id };
            return Ok((id.to_string(), short.to_string()));
        }
    }

    // Otherwise, search by command text
    find_command_by_text(conn, input)
}

/// Find the most recent command whose text matches the input.
/// Tries exact match first, then substring match.
pub fn find_command_by_text(conn: &Connection, text: &str) -> Result<(String, String)> {
    // Try exact match first
    let exact: Option<String> = conn
        .query_row(
            "SELECT id FROM commands WHERE command = ?1 ORDER BY captured_at DESC LIMIT 1",
            rusqlite::params![text],
            |r| r.get(0),
        )
        .ok();

    if let Some(id) = exact {
        let display = truncate_display(text, 40);
        return Ok((id, format!("\"{}\"", display)));
    }

    // Then try substring/LIKE match
    let like: Option<(String, String)> = conn
        .query_row(
            "SELECT id, command FROM commands WHERE command LIKE ?1 ORDER BY captured_at DESC LIMIT 1",
            rusqlite::params![format!("%{}%", text)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();

    if let Some((id, cmd)) = like {
        let display = truncate_display(&cmd, 40);
        return Ok((id, format!("\"{}\"", display)));
    }

    anyhow::bail!(
        "No command found matching '{}'. Use 'cmdstr search' to find commands.",
        text
    )
}

fn truncate_display(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

pub fn apply_tags<'a, I>(command_id: &str, tags: I) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;
    apply_tags_to_conn(&conn, command_id, tags)
}

fn apply_tags_to_conn<'a, I>(conn: &Connection, command_id: &str, tags: I) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() {
            continue;
        }

        // Insert tag if new
        conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            rusqlite::params![&tag],
        )?;

        // Get tag id
        let tag_id: i64 = conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            rusqlite::params![&tag],
            |row| row.get(0),
        )?;

        // Link tag to command
        conn.execute(
            "INSERT OR IGNORE INTO command_tags (command_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![command_id, tag_id],
        )?;
    }
    Ok(())
}
