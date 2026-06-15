use crate::session_store::{
    SessionSnapshotPruneReport, SessionSnapshotStoreStats, SessionSnapshotSummary,
};

#[tauri::command]
pub async fn get_session_store_stats() -> Result<SessionSnapshotStoreStats, String> {
    crate::session_store::stats()
}

#[tauri::command]
pub async fn search_session_store(query: String) -> Result<Vec<SessionSnapshotSummary>, String> {
    crate::session_store::search(&query)
}

#[tauri::command]
pub async fn export_session_store() -> Result<serde_json::Value, String> {
    crate::session_store::export()
}

#[tauri::command]
pub async fn prune_session_store(
    keep_recent: usize,
    older_than_ms: Option<u64>,
) -> Result<SessionSnapshotPruneReport, String> {
    crate::session_store::prune(keep_recent, older_than_ms)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn session_store_handlers_return_backend_payloads() {
        let stats = super::get_session_store_stats()
            .await
            .expect("stats payload");
        assert!(stats.total_snapshots >= stats.corrupted_snapshots);

        let search = super::search_session_store("unlikely-query-for-test".to_string())
            .await
            .expect("search payload");
        assert!(search.is_empty() || search.iter().all(|item| !item.session_id.is_empty()));

        let export = super::export_session_store().await.expect("export payload");
        assert_eq!(export["schema_version"].as_u64(), Some(1));

        let report = super::prune_session_store(usize::MAX, None)
            .await
            .expect("prune payload");
        assert!(report.deleted_session_ids.is_empty());
    }
}
