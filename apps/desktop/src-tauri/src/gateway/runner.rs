use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::eval_headless::{EvalHeadlessRequest, EvalHeadlessTask};
use crate::gateway::webhook::{PendingTrigger, TriggerStore};
use crate::profile::ProfileStore;
use crate::workspace_safety::resolve_session_workspace_path;

const TRIGGER_POLL_INTERVAL_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerRunRecord {
    pub trigger_id: String,
    pub status: String,
    pub message: String,
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
    fallback_workspace: &Path,
    mut executor: F,
) -> Vec<TriggerRunRecord>
where
    F: FnMut(EvalHeadlessRequest) -> Fut,
    Fut: Future<Output = Result<serde_json::Value, String>>,
{
    let triggers = store.drain();
    let mut records = Vec::with_capacity(triggers.len());

    for trigger in triggers {
        let request = match build_headless_request_from_trigger(&trigger, fallback_workspace) {
            Ok(request) => request,
            Err(error) => {
                records.push(TriggerRunRecord {
                    trigger_id: trigger.id,
                    status: "error".to_string(),
                    message: error,
                });
                continue;
            }
        };

        match executor(request).await {
            Ok(payload) => records.push(TriggerRunRecord {
                trigger_id: trigger.id,
                status: trigger_status_from_payload(&payload).to_string(),
                message: trigger_message_from_payload(&payload),
            }),
            Err(error) => records.push(TriggerRunRecord {
                trigger_id: trigger.id,
                status: "error".to_string(),
                message: error,
            }),
        }
    }

    records
}

pub fn spawn_trigger_runner(store: Arc<TriggerStore>, fallback_workspace: PathBuf) {
    tokio::spawn(async move {
        loop {
            let records =
                run_pending_triggers_once(&store, &fallback_workspace, run_headless_request).await;
            for record in records {
                if record.status == "error" || record.status == "failed" {
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
        store.push(PendingTrigger {
            id: "trigger-2".into(),
            message: "run daily digest".into(),
            profile_id: None,
            provider: None,
            model: None,
            workspace_path: Some(workspace.path().to_string_lossy().to_string()),
            received_at_ms: 20,
        });

        let seen_prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts = seen_prompts.clone();
        let records = super::run_pending_triggers_once(&store, workspace.path(), move |request| {
            let prompts = prompts.clone();
            async move {
                prompts.lock().unwrap().push(request.prompt);
                Ok(serde_json::json!({"ok": true}))
            }
        })
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
}
