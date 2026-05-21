use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
#[command(
    about = "Annotate a command with descriptive explanation notes and bookmarks",
    long_about = "Attaches user notes or labels to a command, making it easy to explain what the command \
                  does for future recall. Also supports bookmarking key commands to filter on them easily.",
    after_help = "💡 EXAMPLES:\n\n  \
       1. Add an annotation note to a command by ID:\n     \
          $ cmdstr annotate 01H6W4A5 \"Starts nextjs server in development mode\"\n\n  \
       2. Mark a command as a bookmark without notes:\n     \
          $ cmdstr annotate 01H6W4A5 \"\" --bookmark\n\n  \
       3. Bookmark and add descriptive note in one go:\n     \
          $ cmdstr annotate 01H6W4A5 \"Production postgres dump script\" --bookmark"
)]
pub struct AnnotateArgs {
    /// Target Command ID to annotate
    #[arg(help = "The target command's ULID or ID prefix to attach the annotation note or bookmark to")]
    pub command_id: String,

    /// Explanatory note or description
    #[arg(help = "Detailed annotation notes describing command behavior")]
    pub note: String,

    /// Flag command as a key bookmark
    #[arg(long, help = "Flag the command as a primary bookmark for fast lookup")]
    pub bookmark: bool,
}

pub fn execute(args: &AnnotateArgs) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    set_note_to_conn(&conn, &args.command_id, &args.note, args.bookmark)?;
    println!("Annotated {} ✓", args.command_id);
    Ok(())
}

pub fn set_note(command_id: &str, note: &str, bookmark: bool) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;
    set_note_to_conn(&conn, command_id, note, bookmark)
}

fn set_note_to_conn(conn: &Connection, command_id: &str, note: &str, bookmark: bool) -> Result<()> {
    conn.execute(
        "INSERT INTO annotations (command_id, note, is_bookmark)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(command_id) DO UPDATE SET note = ?2, is_bookmark = ?3",
        rusqlite::params![command_id, note, bookmark as i32],
    )?;
    Ok(())
}
