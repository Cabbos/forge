pub use crate::agent::snapshot::{SessionSnapshotPruneReport, SessionSnapshotStoreStats};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSnapshotSummary {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub working_dir: String,
    pub summary: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
}

pub fn stats() -> Result<SessionSnapshotStoreStats, String> {
    crate::agent::snapshot::session_snapshot_store_stats()
}

pub fn search(query: &str) -> Result<Vec<SessionSnapshotSummary>, String> {
    crate::agent::snapshot::search_session_snapshots(query)
        .map(|snapshots| snapshots.into_iter().map(summary_from_snapshot).collect())
}

pub fn get_summary(session_id: &str) -> Result<Option<SessionSnapshotSummary>, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Ok(None);
    }

    let snapshots = crate::agent::snapshot::list_session_snapshots()?;
    Ok(summary_from_snapshots(snapshots, session_id))
}

pub fn get_snapshot(session_id: &str) -> Result<Option<serde_json::Value>, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Ok(None);
    }
    if get_summary(session_id)?.is_none() {
        return Ok(None);
    }

    serde_json::to_value(crate::agent::snapshot::load_session_snapshot(session_id)?)
        .map(Some)
        .map_err(|error| format!("Failed to serialize session snapshot: {error}"))
}

pub fn export() -> Result<serde_json::Value, String> {
    serde_json::to_value(crate::agent::snapshot::export_session_snapshots()?)
        .map_err(|error| format!("Failed to serialize session export: {error}"))
}

pub fn prune(
    keep_recent: usize,
    older_than_ms: Option<u64>,
) -> Result<SessionSnapshotPruneReport, String> {
    crate::agent::snapshot::prune_session_snapshots(keep_recent, older_than_ms)
}

fn summary_from_snapshot(
    snapshot: crate::agent::snapshot::AgentSessionSnapshot,
) -> SessionSnapshotSummary {
    SessionSnapshotSummary {
        session_id: snapshot.session_id,
        provider: snapshot.provider,
        model: snapshot.model,
        working_dir: snapshot.working_dir,
        summary: snapshot.summary,
        created_at_ms: snapshot.created_at_ms,
        updated_at_ms: snapshot.updated_at_ms,
        message_count: snapshot.messages.len(),
    }
}

fn summary_from_snapshots(
    snapshots: Vec<crate::agent::snapshot::AgentSessionSnapshot>,
    session_id: &str,
) -> Option<SessionSnapshotSummary> {
    snapshots
        .into_iter()
        .find(|snapshot| snapshot.session_id == session_id)
        .map(summary_from_snapshot)
}

#[cfg(test)]
mod tests {
    use crate::adapters::base::ChatMessage;
    use crate::agent::snapshot::AgentSessionSnapshot;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn summary_from_snapshots_matches_exact_session_id() {
        let first = AgentSessionSnapshot::new(
            "session-1".to_string(),
            "openai".to_string(),
            "gpt-5".to_string(),
            "/workspace/one".to_string(),
            vec![ChatMessage::user("hello")],
            Some("first summary".to_string()),
            Some(128_000),
        );
        let second = AgentSessionSnapshot::new(
            "session-10".to_string(),
            "claude".to_string(),
            "sonnet".to_string(),
            "/workspace/two".to_string(),
            vec![
                ChatMessage::user("hello"),
                ChatMessage::assistant("world".into()),
            ],
            None,
            Some(200_000),
        );

        let summary =
            super::summary_from_snapshots(vec![first, second], "session-1").expect("summary");

        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.provider, "openai");
        assert_eq!(summary.model, "gpt-5");
        assert_eq!(summary.working_dir, "/workspace/one");
        assert_eq!(summary.summary.as_deref(), Some("first summary"));
        assert_eq!(summary.message_count, 1);
        assert!(super::summary_from_snapshots(Vec::new(), "session-1").is_none());
    }

    #[test]
    fn get_snapshot_returns_full_snapshot_json() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous_home = std::env::var("HOME").ok();
        let home = tempfile::tempdir().expect("home");
        std::env::set_var("HOME", home.path());
        let snapshot = AgentSessionSnapshot::new(
            "session-detail".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "/repo/detail".to_string(),
            vec![ChatMessage::user("show me".into())],
            Some("detail summary".to_string()),
            Some(128_000),
        );
        crate::agent::snapshot::save_session_snapshot(&snapshot).expect("save snapshot");

        let detail = super::get_snapshot(" session-detail ")
            .expect("get snapshot")
            .expect("snapshot");

        assert_eq!(detail["session_id"], "session-detail");
        assert_eq!(detail["provider"], "deepseek");
        assert_eq!(detail["messages"][0]["content"], "show me");

        if let Some(value) = previous_home {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
