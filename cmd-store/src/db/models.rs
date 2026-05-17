use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    pub command: String,
    pub cwd: String,
    pub exit_code: i32,
    pub duration_ms: i64,
    pub session_id: String,
    pub hostname: String,
    pub captured_at: String,
    pub tags: Vec<String>,
    pub annotation: Option<String>,
    pub is_bookmark: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandFreq {
    pub command: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total: i64,
    pub unique: i64,
    pub bookmarked: i64,
    pub failure_rate: f64,
    pub top_tags: Vec<(String, i64)>,
    pub top_commands: Vec<(String, i64)>,
    pub commands_today: i64,
}
