use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

static LOCK: Mutex<()> = Mutex::new(());

pub struct TestEnv {
    _temp_dir: TempDir,
    old_xdg: Option<String>,
}

impl TestEnv {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let old_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("XDG_DATA_HOME", temp_dir.path());
        std::fs::create_dir_all(temp_dir.path().join("cmdstr")).expect("failed to create cmdstr dir");
        TestEnv { _temp_dir: temp_dir, old_xdg }
    }

    pub fn db_path(&self) -> PathBuf {
        self._temp_dir.path().join("cmdstr").join("commands.db")
    }

    pub fn db_connection(&self) -> Connection {
        let conn = Connection::open(self.db_path()).expect("failed to open test db");
        cmd_store::db::schema::initialize(&conn).expect("schema init failed");
        conn
    }

    pub fn guard() -> std::sync::MutexGuard<'static, ()> {
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        match &self.old_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }
}

pub fn capture_test_command(_env: &TestEnv, command: &str) -> String {
    cmd_store::capture::capture_command(command, 0, 100, "/home/test", "test-session")
        .expect("capture failed")
}

pub fn count_rows(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .expect("count query failed")
}
