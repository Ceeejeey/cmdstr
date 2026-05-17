use crate::config::paths;
use crate::db::{models::Command, schema};
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
pub struct ExportArgs {
    /// Output format: json or csv
    #[arg(short, long, default_value = "json")]
    pub format: String,

    /// Output file (stdout if not specified)
    #[arg(short, long)]
    pub output: Option<String>,
}

pub fn execute(args: &ExportArgs) -> Result<()> {
    let db_path = paths::db_path()?;
    let conn = Connection::open(&db_path)?;
    schema::initialize(&conn)?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.command, c.cwd, c.exit_code, c.duration_ms,
                c.session_id, c.hostname, c.captured_at,
                a.note, a.is_bookmark
         FROM commands c
         LEFT JOIN annotations a ON a.command_id = c.id
         ORDER BY c.captured_at",
    )?;

    let commands: Vec<Command> = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let note: Option<String> = row.get(8)?;
            let is_bookmark: bool = row.get(9)?;

            Ok(Command {
                id,
                command: row.get(1)?,
                cwd: row.get(2)?,
                exit_code: row.get(3)?,
                duration_ms: row.get(4)?,
                session_id: row.get(5)?,
                hostname: row.get(6)?,
                captured_at: row.get(7)?,
                tags: Vec::new(),
                annotation: note,
                is_bookmark,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let output: String = match args.format.as_str() {
        "csv" => commands_to_csv(&commands),
        _ => serde_json::to_string_pretty(&commands)?,
    };

    if let Some(path) = &args.output {
        std::fs::write(path, &output)?;
        println!("Exported {} commands to {path}", commands.len());
    } else {
        println!("{output}");
    }

    Ok(())
}

fn commands_to_csv(commands: &[Command]) -> String {
    let mut csv = String::from("id,command,cwd,exit_code,duration_ms,session_id,hostname,captured_at,note,bookmark\n");
    for cmd in commands {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            cmd.id,
            escape_csv(&cmd.command),
            escape_csv(&cmd.cwd),
            cmd.exit_code,
            cmd.duration_ms,
            cmd.session_id,
            cmd.hostname,
            cmd.captured_at,
            escape_csv(cmd.annotation.as_deref().unwrap_or("")),
            cmd.is_bookmark,
        ));
    }
    csv
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
