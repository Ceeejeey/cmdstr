use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
pub struct TagArgs {
    /// Command ID to tag
    pub command_id: String,

    /// Tag name(s), comma-separated
    pub tags: String,
}

pub fn execute(args: &TagArgs) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    let tags: Vec<&str> = args.tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
    apply_tags_to_conn(&conn, &args.command_id, tags.iter().copied())?;

    println!("Tagged {} with: {}", args.command_id, args.tags);
    Ok(())
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
