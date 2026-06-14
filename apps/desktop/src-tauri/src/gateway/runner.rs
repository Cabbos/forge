use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::eval_headless::{EvalHeadlessRequest, EvalHeadlessTask};
use crate::gateway::webhook::{PendingTrigger, TriggerStore};
use crate::profile::ProfileStore;
use crate::workspace_safety::resolve_session_workspace_path;
use serde::{Deserialize, Serialize};

const TRIGGER_POLL_INTERVAL_SECS: u64 = 5;
const TRIGGER_LEASE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
const MAX_TRIGGER_ATTEMPTS: u32 = 3;
const MAX_TRIGGER_RUN_RECORDS: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TriggerRunRecord {
    pub id: String,
    pub trigger_id: String,
    pub attempt: u32,
    pub status: String,
    pub message: String,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
}

#[derive(Debug, Default)]
pub struct TriggerRunStore {
    records: Mutex<Vec<TriggerRunRecord>>,
    path: Option<PathBuf>,
}

impl TriggerRunStore {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            path: None,
        }
    }

    pub fn persistent_default() -> Self {
        Self::persistent_at(default_trigger_run_store_path())
    }

    pub fn persistent_at(path: PathBuf) -> Self {
        Self {
            records: Mutex::new(load_trigger_runs(&path)),
            path: Some(path),
        }
    }

    pub fn list(&self) -> Vec<TriggerRunRecord> {
        self.records
            .lock()
            .map(|mut records| {
                self.refresh_locked(&mut records);
                records.clone()
            })
            .unwrap_or_default()
    }

    pub fn push(&self, record: TriggerRunRecord) {
        if let Ok(mut records) = self.records.lock() {
            self.refresh_locked(&mut records);
            records.insert(0, record);
            if records.len() > MAX_TRIGGER_RUN_RECORDS {
                records.truncate(MAX_TRIGGER_RUN_RECORDS);
            }
            self.save_locked(&records);
        }
    }

    fn refresh_locked(&self, records: &mut Vec<TriggerRunRecord>) {
        let Some(path) = &self.path else {
            return;
        };
        merge_trigger_runs(records, load_trigger_runs(path));
    }

    fn save_locked(&self, records: &[TriggerRunRecord]) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(error) = save_trigger_runs(path, records) {
            log::warn!("failed to persist gateway trigger runs: {error}");
        }
    }
}

fn merge_trigger_runs(target: &mut Vec<TriggerRunRecord>, incoming: Vec<TriggerRunRecord>) {
    for record in incoming {
        if !target.iter().any(|existing| existing.id == record.id) {
            target.push(record);
        }
    }
    target.sort_by(|a, b| b.started_at_ms.cmp(&a.started_at_ms));
    if target.len() > MAX_TRIGGER_RUN_RECORDS {
        target.truncate(MAX_TRIGGER_RUN_RECORDS);
    }
}

fn default_trigger_run_store_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".forge").join("trigger-runs.json")
}

fn load_trigger_runs(path: &Path) -> Vec<TriggerRunRecord> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<TriggerRunRecord>>(&raw) {
        Ok(records) => records,
        Err(error) => {
            log::warn!("failed to load gateway trigger runs from disk: {error}");
            Vec::new()
        }
    }
}

fn save_trigger_runs(path: &Path, records: &[TriggerRunRecord]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create trigger run dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(records)
        .map_err(|e| format!("serialize trigger runs: {e}"))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, json.as_bytes()).map_err(|e| format!("write trigger run tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("replace trigger run store: {e}"))?;
    Ok(())
}

pub fn build_headless_request_from_trigger(
    trigger: &PendingTrigger,
    fallback_workspace: &Path,
) -> Result<EvalHeadlessRequest, String> {
    let workspace_hint = trigger
        .workspace_path
        .as_deref()
        .filter(|workspace| !workspace.trim().is_empty())
        .map(str::to_string)
        .or_else(|| profile_default_workspace(trigger.profile_id.as_deref()))
        .unwrap_or_else(|| fallback_workspace.to_string_lossy().to_string());
    let workspace_path = resolve_session_workspace_path(&workspace_hint)?;

    Ok(EvalHeadlessRequest {
        task: Some(EvalHeadlessTask {
            id: Some(trigger.id.clone()),
            prompt: Some(trigger.message.clone()),
            ..EvalHeadlessTask::default()
        }),
        prompt: trigger.message.clone(),
        provider: trigger.provider.clone(),
        model: trigger.model.clone(),
        profile_id: trigger.profile_id.clone(),
        workspace_path,
    })
}

pub async fn run_pending_triggers_once<F, Fut>(
    store: &TriggerStore,
    run_store: &TriggerRunStore,
    fallback_workspace: &Path,
    mut executor: F,
) -> Vec<TriggerRunRecord>
where
    F: FnMut(EvalHeadlessRequest) -> Fut,
    Fut: Future<Output = Result<serde_json::Value, String>>,
{
    let triggers = store.claim_available(now_millis(), TRIGGER_LEASE_TIMEOUT_MS);
    let mut records = Vec::with_capacity(triggers.len());

    for trigger in triggers {
        let started_at_ms = now_millis();
        let request = match build_headless_request_from_trigger(&trigger, fallback_workspace) {
            Ok(request) => request,
            Err(error) => {
                records.push(record_trigger_failure(
                    store,
                    run_store,
                    trigger,
                    error,
                    started_at_ms,
                ));
                continue;
            }
        };

        match executor(request).await {
            Ok(payload) => {
                let status = trigger_status_from_payload(&payload);
                let message = trigger_message_from_payload(&payload);
                if status == "failed" {
                    records.push(record_trigger_failure(
                        store,
                        run_store,
                        trigger,
                        message,
                        started_at_ms,
                    ));
                } else {
                    records.push(record_trigger_success(
                        store,
                        run_store,
                        trigger,
                        status,
                        message,
                        started_at_ms,
                    ));
                }
            }
            Err(error) => records.push(record_trigger_failure(
                store,
                run_store,
                trigger,
                error,
                started_at_ms,
            )),
        }
    }

    records
}

fn record_trigger_success(
    store: &TriggerStore,
    run_store: &TriggerRunStore,
    trigger: PendingTrigger,
    status: &str,
    message: String,
    started_at_ms: u64,
) -> TriggerRunRecord {
    let trigger_id = trigger.id.clone();
    let record = TriggerRunRecord {
        id: new_trigger_run_id(),
        trigger_id: trigger.id,
        attempt: trigger.attempt_count.saturating_add(1),
        status: status.to_string(),
        message,
        started_at_ms,
        ended_at_ms: now_millis(),
    };
    run_store.push(record.clone());
    store.complete(&trigger_id);
    record
}

fn record_trigger_failure(
    store: &TriggerStore,
    run_store: &TriggerRunStore,
    mut trigger: PendingTrigger,
    message: String,
    started_at_ms: u64,
) -> TriggerRunRecord {
    let next_attempt = trigger.attempt_count.saturating_add(1);
    trigger.attempt_count = next_attempt;
    let status = if next_attempt < MAX_TRIGGER_ATTEMPTS {
        store.release(trigger.clone());
        "retrying"
    } else {
        store.complete(&trigger.id);
        "dead_letter"
    };

    let record = TriggerRunRecord {
        id: new_trigger_run_id(),
        trigger_id: trigger.id,
        attempt: next_attempt,
        status: status.to_string(),
        message,
        started_at_ms,
        ended_at_ms: now_millis(),
    };
    run_store.push(record.clone());
    record
}

pub fn spawn_trigger_runner(
    store: Arc<TriggerStore>,
    run_store: Arc<TriggerRunStore>,
    fallback_workspace: PathBuf,
) {
    tokio::spawn(async move {
        loop {
            let records = run_pending_triggers_once(
                &store,
                &run_store,
                &fallback_workspace,
                run_headless_request,
            )
            .await;
            for record in records {
                if record.status == "error"
                    || record.status == "failed"
                    || record.status == "retrying"
                    || record.status == "dead_letter"
                {
                    log::warn!(
                        "gateway trigger {} finished with {}: {}",
                        record.trigger_id,
                        record.status,
                        record.message
                    );
                } else {
                    log::info!(
                        "gateway trigger {} finished with {}",
                        record.trigger_id,
                        record.status
                    );
                }
            }
            tokio::time::sleep(Duration::from_secs(TRIGGER_POLL_INTERVAL_SECS)).await;
        }
    });
}

async fn run_headless_request(request: EvalHeadlessRequest) -> Result<serde_json::Value, String> {
    crate::eval_headless::run_request(request).await
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn new_trigger_run_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn profile_default_workspace(profile_id: Option<&str>) -> Option<String> {
    let profile_id = profile_id?;
    let store = ProfileStore::new(ProfileStore::default_path());
    store
        .get(profile_id)
        .and_then(|profile| profile.default_workspace)
        .map(|workspace| workspace.trim().to_string())
        .filter(|workspace| !workspace.is_empty())
}

fn trigger_status_from_payload(payload: &serde_json::Value) -> &'static str {
    if payload.get("error").is_some() || payload.get("failure_reason").is_some() {
        "failed"
    } else {
        "completed"
    }
}

fn trigger_message_from_payload(payload: &serde_json::Value) -> String {
    payload
        .get("failure_reason")
        .or_else(|| payload.get("final_answer"))
        .and_then(|value| value.as_str())
        .filter(|message| !message.trim().is_empty())
        .unwrap_or("Headless trigger run finished.")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::gateway::webhook::{PendingTrigger, TriggerStore};

    #[test]
    fn builds_headless_request_from_trigger_metadata() {
        let workspace = tempfile::tempdir().expect("workspace");
        let trigger = PendingTrigger {
            id: "trigger-1".into(),
            message: "summarize the queue".into(),
            profile_id: Some("ops".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5".into()),
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            attempt_count: 0,
            claimed_at_ms: None,
            received_at_ms: 10,
        };

        let request = super::build_headless_request_from_trigger(&trigger, workspace.path())
            .expect("headless request");

        assert_eq!(request.prompt, "summarize the queue");
        assert_eq!(request.profile_id.as_deref(), Some("ops"));
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-5"));
        assert_eq!(
            request.workspace_path,
            workspace.path().canonicalize().unwrap()
        );
        assert_eq!(
            request.task.as_ref().and_then(|task| task.id.as_deref()),
            Some("trigger-1")
        );
    }

    #[tokio::test]
    async fn run_pending_triggers_once_drains_and_invokes_executor() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = TriggerStore::new();
        let run_store = super::TriggerRunStore::new();
        store.push(PendingTrigger {
            id: "trigger-2".into(),
            message: "run daily digest".into(),
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            attempt_count: 0,
            claimed_at_ms: None,
            received_at_ms: 20,
        });

        let seen_prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts = seen_prompts.clone();
        let records = super::run_pending_triggers_once(
            &store,
            &run_store,
            workspace.path(),
            move |request| {
                let prompts = prompts.clone();
                async move {
                    prompts.lock().unwrap().push(request.prompt);
                    Ok(serde_json::json!({"ok": true}))
                }
            },
        )
        .await;

        assert!(store.list().is_empty());
        assert_eq!(
            seen_prompts.lock().unwrap().as_slice(),
            ["run daily digest"]
        );
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].trigger_id, "trigger-2");
        assert_eq!(records[0].status, "completed");
    }

    #[tokio::test]
    async fn run_pending_triggers_once_requeues_failures_until_dead_letter() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = TriggerStore::new();
        let run_store = super::TriggerRunStore::new();
        store.push(PendingTrigger {
            id: "trigger-3".into(),
            message: "run flaky digest".into(),
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            attempt_count: 0,
            claimed_at_ms: None,
            received_at_ms: 30,
        });

        let first = super::run_pending_triggers_once(
            &store,
            &run_store,
            workspace.path(),
            |_request| async { Err("provider offline".to_string()) },
        )
        .await;
        assert_eq!(first[0].status, "retrying");
        assert_eq!(store.list().len(), 1);

        let second = super::run_pending_triggers_once(
            &store,
            &run_store,
            workspace.path(),
            |_request| async { Err("provider offline".to_string()) },
        )
        .await;
        assert_eq!(second[0].status, "retrying");
        assert_eq!(store.list().len(), 1);

        let third = super::run_pending_triggers_once(
            &store,
            &run_store,
            workspace.path(),
            |_request| async { Err("provider offline".to_string()) },
        )
        .await;
        assert_eq!(third[0].status, "dead_letter");
        assert_eq!(third[0].message, "provider offline");
        assert!(store.list().is_empty());
    }

    #[tokio::test]
    async fn run_pending_triggers_once_persists_attempt_records() {
        let workspace = tempfile::tempdir().expect("workspace");
        let run_path = workspace.path().join("trigger-runs.json");
        let run_store = super::TriggerRunStore::persistent_at(run_path.clone());
        let store = TriggerStore::new();
        store.push(PendingTrigger {
            id: "trigger-4".into(),
            message: "run durable ledger".into(),
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            attempt_count: 0,
            claimed_at_ms: None,
            received_at_ms: 40,
        });

        let records = super::run_pending_triggers_once(
            &store,
            &run_store,
            workspace.path(),
            |_request| async { Ok(serde_json::json!({"final_answer": "ledger ok"})) },
        )
        .await;

        assert_eq!(records.len(), 1);
        let restored = super::TriggerRunStore::persistent_at(run_path);
        let persisted = restored.list();
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].trigger_id, "trigger-4");
        assert_eq!(persisted[0].status, "completed");
        assert_eq!(persisted[0].attempt, 1);
        assert_eq!(persisted[0].message, "ledger ok");
        assert!(persisted[0].started_at_ms <= persisted[0].ended_at_ms);
    }

    #[tokio::test]
    async fn run_pending_triggers_once_leases_trigger_during_execution() {
        let workspace = tempfile::tempdir().expect("workspace");
        let trigger_path = workspace.path().join("pending-triggers.json");
        let store = TriggerStore::persistent_at(trigger_path.clone());
        let run_store = super::TriggerRunStore::new();
        store.push(PendingTrigger {
            id: "trigger-5".into(),
            message: "run leased work".into(),
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            attempt_count: 0,
            claimed_at_ms: None,
            received_at_ms: 50,
        });

        let observed_claim = Arc::new(Mutex::new(None));
        let observed_claim_for_executor = observed_claim.clone();
        let trigger_path_for_executor = trigger_path.clone();
        let records = super::run_pending_triggers_once(
            &store,
            &run_store,
            workspace.path(),
            move |_request| {
                let observed_claim = observed_claim_for_executor.clone();
                let trigger_path = trigger_path_for_executor.clone();
                async move {
                    let persisted = TriggerStore::persistent_at(trigger_path).list();
                    *observed_claim.lock().unwrap() =
                        persisted.first().and_then(|trigger| trigger.claimed_at_ms);
                    Ok(serde_json::json!({"final_answer": "lease ok"}))
                }
            },
        )
        .await;

        assert_eq!(records[0].status, "completed");
        assert!(observed_claim.lock().unwrap().is_some());
        assert!(TriggerStore::persistent_at(trigger_path).list().is_empty());
    }
}
