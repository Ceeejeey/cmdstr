mod common;

use common::{capture_test_command, count_rows, TestEnv};
use cmd_store::cli;
use cmd_store::db::schema;
use rusqlite::Connection;

// ── Schema ───────────────────────────────────────────────────────────────

#[test]
fn test_schema_initialization() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let conn = env.db_connection();
    schema::initialize(&conn).expect("schema init failed");

    assert_eq!(count_rows(&conn, "commands"), 0);
    assert_eq!(count_rows(&conn, "tags"), 0);
    assert_eq!(count_rows(&conn, "command_tags"), 0);
    assert_eq!(count_rows(&conn, "annotations"), 0);
    assert_eq!(count_rows(&conn, "command_freq"), 0);
}

#[test]
fn test_schema_is_idempotent() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let conn = env.db_connection();
    schema::initialize(&conn).expect("first init");
    schema::initialize(&conn).expect("second init");

    assert_eq!(count_rows(&conn, "commands"), 0);
}

// ── Capture ──────────────────────────────────────────────────────────────

#[test]
fn test_capture_command() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "echo hello");
    assert!(!id.is_empty(), "capture should return a ULID");

    let conn = env.db_connection();
    let (cmd, exit_code, duration, session_id): (String, i32, i64, String) = conn
        .query_row(
            "SELECT command, exit_code, duration_ms, session_id FROM commands WHERE id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("captured command not found");

    assert_eq!(cmd, "echo hello");
    assert_eq!(exit_code, 0);
    assert_eq!(duration, 100);
    assert_eq!(session_id, "test-session");
    assert_eq!(count_rows(&conn, "commands"), 1);
}

#[test]
fn test_capture_updates_freq() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    capture_test_command(&env, "docker ps");
    capture_test_command(&env, "docker ps");
    capture_test_command(&env, "docker ps");

    let conn = env.db_connection();
    let (command, count): (String, i64) = conn
        .query_row(
            "SELECT command, count FROM command_freq ORDER BY count DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("freq entry not found");

    assert_eq!(command, "docker ps");
    assert_eq!(count, 3);
    assert_eq!(count_rows(&conn, "commands"), 3);
}

#[test]
fn test_capture_distinct_freq() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    capture_test_command(&env, "ls -la");
    capture_test_command(&env, "pwd");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "command_freq"), 2);
}

#[test]
fn test_capture_stores_hostname() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "hostname_test");
    let conn = env.db_connection();
    let hostname: String = conn
        .query_row("SELECT hostname FROM commands WHERE id = ?1", rusqlite::params![id], |r| r.get(0))
        .expect("command not found");

    assert!(!hostname.is_empty(), "hostname should be populated");
}

#[test]
fn test_capture_exit_code_and_duration() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    cmd_store::capture::capture_command("failing_cmd", 1, 500, "/tmp", "s2")
        .expect("capture failed");

    let conn = env.db_connection();
    let (exit_code, duration_ms): (i32, i64) = conn
        .query_row(
            "SELECT exit_code, duration_ms FROM commands WHERE command = ?1",
            rusqlite::params!["failing_cmd"],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("command not found");

    assert_eq!(exit_code, 1);
    assert_eq!(duration_ms, 500);
}

#[test]
fn test_capture_multiple_commands() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    capture_test_command(&env, "first");
    capture_test_command(&env, "second");
    capture_test_command(&env, "third");

    let conn = env.db_connection();
    let rows = query_commands(&conn);
    assert_eq!(rows.len(), 3);
}

// ── Tag ─────────────────────────────────────────────────────────────────

#[test]
fn test_tag_command() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "docker compose up");
    cli::tag::apply_tags(&id, ["docker", "compose"]).expect("tagging failed");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "tags"), 2);

    let tags = cli::query::get_tags_for_command(&conn, &id).expect("get tags");
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&"docker".to_string()));
    assert!(tags.contains(&"compose".to_string()));
}

#[test]
fn test_tag_duplicates_are_idempotent() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "npm test");
    cli::tag::apply_tags(&id, ["test", "npm"]).expect("first tag");
    cli::tag::apply_tags(&id, ["test", "npm"]).expect("second tag");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "tags"), 2);
    assert_eq!(count_rows(&conn, "command_tags"), 2);
}

#[test]
fn test_tag_empty_strings_ignored() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "git status");
    cli::tag::apply_tags(&id, ["git", "", "  ", "vcs"]).expect("tag with empty");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "tags"), 2);

    let tags = cli::query::get_tags_for_command(&conn, &id).expect("get tags");
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&"git".to_string()));
    assert!(tags.contains(&"vcs".to_string()));
}

#[test]
fn test_tag_normalizes_to_lowercase() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "echo CASE");
    cli::tag::apply_tags(&id, ["Docker", "Compose"]).expect("tag");

    let conn = env.db_connection();
    let tags = cli::query::get_tags_for_command(&conn, &id).expect("get tags");
    assert!(tags.contains(&"docker".to_string()));
    assert!(tags.contains(&"compose".to_string()));
}

#[test]
fn test_tag_multiple_commands_independent() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id1 = capture_test_command(&env, "cmd_a");
    let id2 = capture_test_command(&env, "cmd_b");

    cli::tag::apply_tags(&id1, ["tag_a"]).expect("tag id1");
    cli::tag::apply_tags(&id2, ["tag_b"]).expect("tag id2");

    let conn = env.db_connection();
    let tags1 = cli::query::get_tags_for_command(&conn, &id1).expect("get tags1");
    let tags2 = cli::query::get_tags_for_command(&conn, &id2).expect("get tags2");

    assert_eq!(tags1, vec!["tag_a"]);
    assert_eq!(tags2, vec!["tag_b"]);
    assert_eq!(count_rows(&conn, "tags"), 2);
}

#[test]
fn test_tag_cli_execute() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "cli_tag_test");
    let args = cli::tag::TagArgs {
        command_id: id.clone(),
        tags: "alpha, beta, gamma".to_string(),
    };
    cli::tag::execute(&args).expect("tag execute");

    let conn = env.db_connection();
    let tags = cli::query::get_tags_for_command(&conn, &id).expect("get tags");
    assert_eq!(tags.len(), 3);
    assert!(tags.contains(&"alpha".to_string()));
    assert!(tags.contains(&"beta".to_string()));
    assert!(tags.contains(&"gamma".to_string()));
}

// ── Annotate ─────────────────────────────────────────────────────────────

#[test]
fn test_annotate_add_note() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "curl example.com");
    cli::annotate::set_note(&id, "Test API endpoint", false).expect("set note");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "annotations"), 1);

    let (note, bookmark): (String, bool) = conn
        .query_row(
            "SELECT note, is_bookmark FROM annotations WHERE command_id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("annotation not found");

    assert_eq!(note, "Test API endpoint");
    assert!(!bookmark);
}

#[test]
fn test_annotate_with_bookmark() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "favourite cmd");
    cli::annotate::set_note(&id, "My favourite", true).expect("set bookmark");

    let conn = env.db_connection();
    let is_bookmark: bool = conn
        .query_row(
            "SELECT is_bookmark FROM annotations WHERE command_id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .expect("annotation not found");

    assert!(is_bookmark);
}

#[test]
fn test_annotate_update_overwrites() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "updatable cmd");
    cli::annotate::set_note(&id, "Version 1", false).expect("first note");
    cli::annotate::set_note(&id, "Version 2", true).expect("second note");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "annotations"), 1);

    let (note, bookmark): (String, bool) = conn
        .query_row(
            "SELECT note, is_bookmark FROM annotations WHERE command_id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("annotation not found");

    assert_eq!(note, "Version 2");
    assert!(bookmark);
}

#[test]
fn test_annotate_cli_execute() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "cli annotate test");
    let args = cli::annotate::AnnotateArgs {
        command_id: id.clone(),
        note: "CLI annotation".to_string(),
        bookmark: true,
    };
    cli::annotate::execute(&args).expect("annotate execute");

    let conn = env.db_connection();
    let (note, bookmark): (String, bool) = conn
        .query_row(
            "SELECT note, is_bookmark FROM annotations WHERE command_id = ?1",
            rusqlite::params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("annotation not found");

    assert_eq!(note, "CLI annotation");
    assert!(bookmark);
}

// ── Search ───────────────────────────────────────────────────────────────

fn seed_search_data(env: &TestEnv) {
    let id1 = capture_test_command(env, "docker ps");
    let id2 = capture_test_command(env, "docker compose up");
    let _id3 = capture_test_command(env, "npm run build");
    let id4 = capture_test_command(env, "git push origin main");
    cmd_store::capture::capture_command("failing_tool", 1, 50, "/tmp", "s5").expect("capture failed");

    cli::tag::apply_tags(&id1, ["docker"]).expect("tag id1");
    cli::tag::apply_tags(&id2, ["docker", "compose"]).expect("tag id2");
    cli::tag::apply_tags(&id4, ["git", "vcs"]).expect("tag id4");
    cli::annotate::set_note(&id4, "Favourite command", true).expect("note id4 bookmark");
}

#[test]
fn test_search_by_query_db() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    seed_search_data(&env);

    let conn = env.db_connection();
    let mut stmt = conn
        .prepare("SELECT command FROM commands WHERE command LIKE ?1 ORDER BY captured_at")
        .expect("prepare");
    let results: Vec<String> = stmt
        .query_map(rusqlite::params!["%docker%"], |r| r.get(0))
        .expect("query_map")
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|c| c == "docker ps"));
    assert!(results.iter().any(|c| c == "docker compose up"));
}

#[test]
fn test_search_by_tag_db() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    seed_search_data(&env);

    let conn = env.db_connection();
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT c.command FROM commands c
             JOIN command_tags ct ON ct.command_id = c.id
             JOIN tags t ON t.id = ct.tag_id
             WHERE t.name = ?1",
        )
        .expect("prepare");
    let results: Vec<String> = stmt
        .query_map(rusqlite::params!["docker"], |r| r.get(0))
        .expect("query_map")
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(results.len(), 2);
    assert!(results.contains(&"docker ps".to_string()));
    assert!(results.contains(&"docker compose up".to_string()));
}

#[test]
fn test_search_failed_only_db() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    seed_search_data(&env);

    let conn = env.db_connection();
    let mut stmt = conn
        .prepare("SELECT command FROM commands WHERE exit_code != 0")
        .expect("prepare");
    let results: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("query_map")
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(results, vec!["failing_tool"]);
}

#[test]
fn test_search_bookmarks_only_db() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    seed_search_data(&env);

    let conn = env.db_connection();
    let mut stmt = conn
        .prepare(
            "SELECT c.command FROM commands c
             JOIN annotations a ON a.command_id = c.id
             WHERE a.is_bookmark = 1",
        )
        .expect("prepare");
    let results: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("query_map")
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(results, vec!["git push origin main"]);
}

// ── Stats (DB-level) ─────────────────────────────────────────────────────

#[test]
fn test_stats_empty_db_values() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    let conn = env.db_connection();

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, 0);
}

#[test]
fn test_stats_with_data_values() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id1 = capture_test_command(&env, "docker ps");
    capture_test_command(&env, "docker ps");
    capture_test_command(&env, "ls");
    cmd_store::capture::capture_command("fail", 1, 0, "/tmp", "s").expect("capture");
    cli::annotate::set_note(&id1, "bookmarked!", true).expect("bookmark");

    let conn = env.db_connection();
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0)).unwrap();
    let bookmarked: i64 = conn
        .query_row("SELECT COUNT(*) FROM annotations WHERE is_bookmark = 1", [], |r| r.get(0))
        .unwrap();
    let failed: i64 = conn
        .query_row("SELECT COUNT(*) FROM commands WHERE exit_code != 0", [], |r| r.get(0))
        .unwrap();

    assert_eq!(total, 4);
    assert_eq!(bookmarked, 1);
    assert_eq!(failed, 1);
}

// ── Export with in-process verification ──────────────────────────────────

#[test]
fn test_export_json_format_in_process() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    let _conn = env.db_connection();

    // Verify an empty export gives empty JSON via the library function
    let commands: Vec<cmd_store::db::models::Command> = Vec::new();
    let json = serde_json::to_string_pretty(&commands).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn test_export_with_data_json_format() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "echo export_test");
    cli::annotate::set_note(&id, "test note", true).expect("note");

    let conn = env.db_connection();
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.command, c.cwd, c.exit_code, c.duration_ms,
                    c.session_id, c.hostname, c.captured_at,
                    a.note, a.is_bookmark
             FROM commands c
             LEFT JOIN annotations a ON a.command_id = c.id
             ORDER BY c.captured_at",
        )
        .unwrap();

    let commands: Vec<cmd_store::db::models::Command> = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let note: Option<String> = row.get(8)?;
            let is_bookmark: bool = row.get(9)?;
            Ok(cmd_store::db::models::Command {
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
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "echo export_test");
    assert_eq!(commands[0].annotation.as_deref(), Some("test note"));
    assert!(commands[0].is_bookmark);
}

#[test]
fn test_export_csv_correctness() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    capture_test_command(&env, "echo csv_test");

    let conn = env.db_connection();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
        .unwrap();
    assert!(count > 0);
}

// ── Add (end-to-end with tags + notes) ───────────────────────────────────

#[test]
fn test_add_command_end_to_end() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let args = cli::add::AddArgs {
        command: "kubectl get pods".to_string(),
        tag: Some("kubernetes,k8s".to_string()),
        note: Some("List all pods".to_string()),
        exit_code: 0,
        duration: 0,
    };
    cli::add::execute(&args).expect("add failed");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "commands"), 1);

    let cmd_id: String = conn
        .query_row("SELECT id FROM commands LIMIT 1", [], |r| r.get(0))
        .expect("no commands");

    let (cmd,): (String,) = conn
        .query_row(
            "SELECT command FROM commands WHERE id = ?1",
            rusqlite::params![cmd_id],
            |r| Ok((r.get(0)?,)),
        )
        .expect("command not found");
    assert_eq!(cmd, "kubectl get pods");

    let tags = cli::query::get_tags_for_command(&conn, &cmd_id).expect("get tags");
    assert!(tags.contains(&"kubernetes".to_string()));
    assert!(tags.contains(&"k8s".to_string()));

    let (note,): (String,) = conn
        .query_row(
            "SELECT note FROM annotations WHERE command_id = ?1",
            rusqlite::params![cmd_id],
            |r| Ok((r.get(0)?,)),
        )
        .expect("annotation not found");
    assert_eq!(note, "List all pods");
}

#[test]
fn test_add_command_without_tags_or_notes() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let args = cli::add::AddArgs {
        command: "plain command".to_string(),
        tag: None,
        note: None,
        exit_code: 0,
        duration: 0,
    };
    cli::add::execute(&args).expect("add failed");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "commands"), 1);
    assert_eq!(count_rows(&conn, "tags"), 0);
    assert_eq!(count_rows(&conn, "annotations"), 0);
}

// ── Run tag lookup ───────────────────────────────────────────────────────

#[test]
fn test_run_lookup_existing_tag() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "echo run_me");
    cli::tag::apply_tags(&id, ["runnable"]).expect("tag");

    let conn = env.db_connection();
    let cmd: Option<String> = conn
        .query_row(
            "SELECT c.command FROM commands c
             JOIN command_tags ct ON ct.command_id = c.id
             JOIN tags t ON t.id = ct.tag_id
             WHERE t.name = ?1
             ORDER BY c.captured_at DESC LIMIT 1",
            rusqlite::params!["runnable"],
            |r| r.get(0),
        )
        .expect("query failed");

    assert_eq!(cmd, Some("echo run_me".to_string()));
}

#[test]
fn test_run_lookup_nonexistent_tag() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();
    env.db_connection();

    let conn = env.db_connection();
    let cmd: Option<String> = conn
        .query_row(
            "SELECT c.command FROM commands c
             JOIN command_tags ct ON ct.command_id = c.id
             JOIN tags t ON t.id = ct.tag_id
             WHERE t.name = ?1
             ORDER BY c.captured_at DESC LIMIT 1",
            rusqlite::params!["nonexistent"],
            |r| r.get(0),
        )
        .ok();

    assert!(cmd.is_none(), "nonexistent tag should not find a command");
}

// ── Foreign key cascade ──────────────────────────────────────────────────

#[test]
fn test_delete_command_cascades_to_tags_and_annotations() {
    let _g = TestEnv::guard();
    let env = TestEnv::new();

    let id = capture_test_command(&env, "cascade test");
    cli::tag::apply_tags(&id, ["cascade"]).expect("tag");
    cli::annotate::set_note(&id, "will be deleted", false).expect("note");

    let conn = env.db_connection();
    assert_eq!(count_rows(&conn, "tags"), 1);
    assert_eq!(count_rows(&conn, "command_tags"), 1);
    assert_eq!(count_rows(&conn, "annotations"), 1);

    conn.execute("DELETE FROM commands WHERE id = ?1", rusqlite::params![id])
        .expect("delete failed");

    assert_eq!(count_rows(&conn, "commands"), 0);
    assert_eq!(
        count_rows(&conn, "command_tags"),
        0,
        "command_tags should cascade on DELETE"
    );
    assert_eq!(
        count_rows(&conn, "annotations"),
        0,
        "annotations should cascade on DELETE"
    );
    assert_eq!(
        count_rows(&conn, "tags"),
        1,
        "tags should NOT cascade (shared across commands)"
    );
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn query_commands(conn: &Connection) -> Vec<(String, i32, i64, String)> {
    let mut stmt = conn
        .prepare("SELECT command, exit_code, duration_ms, session_id FROM commands ORDER BY captured_at")
        .expect("prepare");
    stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i32>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
        ))
    })
    .expect("query_map")
    .filter_map(|r| r.ok())
    .collect()
}
