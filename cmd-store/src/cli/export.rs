use crate::cli::query::get_tags_for_command;
use crate::config::paths;
use crate::db::{models::Command, schema};
use anyhow::Result;
use clap::Args;
use rusqlite::Connection;

#[derive(Args)]
#[command(
    about = "Export commands from the history store database",
    long_about = "Reads all recorded commands along with metadata (tags, notes, exits, durations, hostnames, timestamps) \
                  and formats them into structured JSON or standard CSV. Output is printed to stdout by default, \
                  or redirected to a specified target file.",
    after_help = "💡 EXAMPLES:\n\n  \
       1. Export all history as pretty JSON to stdout:\n     \
          $ cmdstr export --format json\n\n  \
       2. Save all history as a structured CSV file:\n     \
          $ cmdstr export --format csv --output ~/history_backup.csv\n\n  \
       3. Export as JSON to a custom file:\n     \
          $ cmdstr export -f json -o ~/backup.json"
)]
pub struct ExportArgs {
    /// Target export format type
    #[arg(short, long, default_value = "json", help = "Output serialization format: json or csv")]
    pub format: String,

    /// Absolute target file output path
    #[arg(short, long, help = "Optional target file output path. Prints output to stdout if omitted")]
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

    let mut commands: Vec<Command> = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let note: Option<String> = row.get(8)?;
            let is_bookmark: Option<i32> = row.get(9)?;

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
                is_bookmark: is_bookmark.unwrap_or(0) != 0,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Populate tags for each command
    for cmd in &mut commands {
        cmd.tags = get_tags_for_command(&conn, &cmd.id).unwrap_or_default();
    }

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

pub(crate) fn commands_to_csv(commands: &[Command]) -> String {
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

pub(crate) fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::Command;

    #[test]
    fn test_escape_csv_plain() {
        assert_eq!(escape_csv("hello"), "hello");
    }

    #[test]
    fn test_escape_csv_with_commas() {
        assert_eq!(escape_csv("a,b,c"), "\"a,b,c\"");
    }

    #[test]
    fn test_escape_csv_with_quotes() {
        assert_eq!(escape_csv("say \"hello\""), "\"say \"\"hello\"\"\"");
    }

    #[test]
    fn test_escape_csv_with_newline() {
        assert_eq!(escape_csv("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_escape_csv_empty() {
        assert_eq!(escape_csv(""), "");
    }

    #[test]
    fn test_commands_to_csv_empty() {
        let csv = commands_to_csv(&[]);
        assert_eq!(csv, "id,command,cwd,exit_code,duration_ms,session_id,hostname,captured_at,note,bookmark\n");
    }

    #[test]
    fn test_commands_to_csv_single() {
        let cmd = Command {
            id: "test-id".into(),
            command: "echo hi".into(),
            cwd: "/home".into(),
            exit_code: 0,
            duration_ms: 100,
            session_id: "s1".into(),
            hostname: "box".into(),
            captured_at: "2026-01-01T00:00:00Z".into(),
            tags: vec![],
            annotation: None,
            is_bookmark: false,
        };
        let csv = commands_to_csv(&[cmd]);
        let expected = "id,command,cwd,exit_code,duration_ms,session_id,hostname,captured_at,note,bookmark\n\
                        test-id,echo hi,/home,0,100,s1,box,2026-01-01T00:00:00Z,,false\n";
        assert_eq!(csv, expected);
    }

    #[test]
    fn test_commands_to_csv_with_annotation() {
        let cmd = Command {
            id: "id1".into(),
            command: "docker ps".into(),
            cwd: "/".into(),
            exit_code: 0,
            duration_ms: 50,
            session_id: "s2".into(),
            hostname: "host".into(),
            captured_at: "2026-06-01T12:00:00Z".into(),
            tags: vec![],
            annotation: Some("list containers".into()),
            is_bookmark: true,
        };
        let csv = commands_to_csv(&[cmd]);
        assert!(csv.contains("docker ps"));
        assert!(csv.contains("list containers"));
        assert!(csv.contains("true"));
        assert!(csv.contains("id1"));
    }

    #[test]
    fn test_commands_to_csv_quotes_special_chars() {
        let cmd = Command {
            id: "id2".into(),
            command: "cmd,with,commas".into(),
            cwd: "/tmp".into(),
            exit_code: 0,
            duration_ms: 0,
            session_id: "s".into(),
            hostname: "h".into(),
            captured_at: "t".into(),
            tags: vec![],
            annotation: None,
            is_bookmark: false,
        };
        let csv = commands_to_csv(&[cmd]);
        assert!(csv.contains("\"cmd,with,commas\""));
    }
}
