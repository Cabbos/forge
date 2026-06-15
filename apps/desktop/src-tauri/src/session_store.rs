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
