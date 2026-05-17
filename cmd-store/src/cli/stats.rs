use crate::config::paths;
use crate::db::schema;
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
pub struct StatsArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn execute(args: &StatsArgs) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    let total: i64 = conn.query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))?;
    let unique: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT command_hash) FROM command_freq",
        [],
        |r| r.get(0),
    )?;
    let bookmarked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM annotations WHERE is_bookmark = 1",
        [],
        |r| r.get(0),
    )?;
    let failed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM commands WHERE exit_code != 0",
        [],
        |r| r.get(0),
    )?;
    let today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM commands WHERE captured_at >= datetime('now', 'start of day')",
        [],
        |r| r.get(0),
    )?;

    let failure_rate = if total > 0 {
        (failed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Top tags
    let mut stmt = conn.prepare(
        "SELECT t.name, COUNT(ct.command_id) as cnt
         FROM tags t
         JOIN command_tags ct ON ct.tag_id = t.id
         GROUP BY t.id ORDER BY cnt DESC LIMIT 10",
    )?;
    let top_tags: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    // Most frequent commands
    let mut stmt = conn.prepare(
        "SELECT command, count FROM command_freq ORDER BY count DESC LIMIT 10",
    )?;
    let top_cmds: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    if args.json {
        let stats = serde_json::json!({
            "total": total,
            "unique": unique,
            "bookmarked": bookmarked,
            "failure_rate": format!("{:.1}%", failure_rate),
            "commands_today": today,
            "top_tags": top_tags,
            "top_commands": top_cmds,
        });
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        println!("📊 cmdstr stats\n");
        println!("  Total commands:    {total}");
        println!("  Unique commands:   {unique}");
        println!("  Bookmarked:        {bookmarked}");
        println!("  Failure rate:      {failure_rate:.1}%");
        println!("  Commands today:    {today}\n");

        if !top_tags.is_empty() {
            println!("  Top tags:");
            for (tag, count) in &top_tags {
                println!("    {tag:<20} {count}");
            }
            println!();
        }

        if !top_cmds.is_empty() {
            println!("  Most frequent:");
            for (cmd, count) in &top_cmds {
                println!("    {count:>4}x  {cmd}");
            }
        }
    }

    Ok(())
}
