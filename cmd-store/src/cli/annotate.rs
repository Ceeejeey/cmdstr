use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
pub struct AnnotateArgs {
    /// Command ID
    pub command_id: String,

    /// Annotation note
    pub note: String,

    /// Mark as bookmark
    #[arg(long)]
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
