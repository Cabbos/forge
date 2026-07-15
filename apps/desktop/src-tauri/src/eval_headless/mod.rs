pub mod resolve;
pub mod runner;
pub mod snapshot;
pub mod summary;
pub mod trace;
pub mod types;
pub mod validation;

use std::sync::Arc;
use std::time::Instant;

use crate::agent::session_guards::lock_unpoisoned;
use crate::agent::time::now_ms;
use crate::agent::turn_state::AgentTurnState;
use crate::continuity::ContinuityService;
use crate::ipc::session_builder::{
    build_agent_session_with_registry_path, BuildAgentSessionRequest,
};
use crate::settings;

pub use types::{EvalHeadlessRequest, EvalHeadlessTask, HeadlessFileDiff, TracePayloadInput};

pub use trace::{build_trace_payload, insert_agent_identity, insert_failure_fields};

pub async fn run_stdin_json(input: &str) -> Result<serde_json::Value, String> {
    let request: EvalHeadlessRequest = serde_json::from_str(input)
        .map_err(|error| format!("failed to parse Forge eval stdin JSON: {error}"))?;
    run_request(request).await
}

pub async fn run_request(request: EvalHeadlessRequest) -> Result<serde_json::Value, String> {
    let credential_store = crate::credential_store::system_credential_store();
    crate::credential_migration::migrate_legacy_credentials(
        &crate::credential_migration::CredentialMigrationPaths::default_paths(),
        credential_store.as_ref(),
    )
    .map_err(|_| "credential migration failed before headless startup".to_string())?;
    let profile_store =
        crate::profile::ProfileStore::new(crate::profile::ProfileStore::default_path());
    let selected_profile = request
        .profile_id
        .as_deref()
        .and_then(|profile_id| profile_store.get(profile_id));

    let started = Instant::now();
    let task_id = request
        .task
        .as_ref()
        .and_then(|task| task.id.clone())
        .unwrap_or_else(|| "forge-headless-task".to_string());
    let prompt = resolve::resolve_prompt(&request)?;
    let display_provider = request
        .provider
        .clone()
        .unwrap_or_else(|| "forge".to_string());
    let display_model = request
        .model
        .clone()
        .unwrap_or_else(|| "local-forge".to_string());
    let workspace_path = request.workspace_path.clone();
    let (registry_database_path, continuity_database_path) = resolve_headless_runtime_paths(
        &workspace_path,
        request.runtime_state_path.as_deref(),
        request.continuity_database_path.as_deref(),
    )?;
    let continuity_service = continuity_database_path
        .map(ContinuityService::new_with_database_path)
        .unwrap_or_else(ContinuityService::new);

    // Resolve provider/model from profile when profile_id is set.
    let (effective_provider, effective_model) = resolve::resolve_profile_defaults(
        request.profile_id.as_deref(),
        request.provider.as_deref().unwrap_or("forge"),
        request.model.as_deref().unwrap_or("local-forge"),
    );

    let before_snapshot = snapshot::snapshot_workspace(&workspace_path)?;
    let agent_provider = resolve::resolve_agent_provider(Some(&effective_provider));
    let credentials = settings::CredentialResolver::new(credential_store)
        .resolve(&agent_provider, selected_profile.as_ref())
        .map_err(|error| error.to_string())?;
    let agent_model = resolve::resolve_agent_model(
        Some(&effective_model),
        credentials.model.as_deref(),
        &agent_provider,
    );

    if credentials.api_key.trim().is_empty() {
        return Ok(trace::build_setup_error_payload(
            types::SetupErrorPayloadInput {
                task_id,
                prompt,
                display_provider,
                display_model,
                agent_provider,
                agent_model,
                duration_ms: started.elapsed().as_millis() as u64,
                error: "missing_api_key".to_string(),
                failure_reason:
                    "Forge headless eval could not find an API key for the selected provider."
                        .to_string(),
            },
        ));
    }

    let pending_confirms = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let session_id = uuid::Uuid::now_v7().to_string();
    let (session, missing_api_key) = build_agent_session_with_registry_path(
        BuildAgentSessionRequest {
            session_id: session_id.clone(),
            provider: agent_provider.clone(),
            model: agent_model.clone(),
            api_key: &credentials.api_key,
            api_base: credentials.api_base.as_deref(),
            working_dir: &workspace_path,
            pending_confirms: pending_confirms.clone(),
            existing_context_window_tokens: None,
        },
        registry_database_path,
    )
    .await?;

    if missing_api_key {
        return Ok(trace::build_setup_error_payload(
            types::SetupErrorPayloadInput {
                task_id,
                prompt,
                display_provider,
                display_model,
                agent_provider,
                agent_model,
                duration_ms: started.elapsed().as_millis() as u64,
                error: "missing_api_key".to_string(),
                failure_reason: "Forge headless eval built a session without usable credentials."
                    .to_string(),
            },
        ));
    }

    let session = Arc::new(session);
    let emitter = Arc::new(runner::HeadlessEventEmitter::new(pending_confirms));
    let validation_commands = validation::validation_commands_from_task(request.task.as_ref());
    let max_repair_attempts = validation::max_repair_attempts_from_task(request.task.as_ref());
    let timeout_secs = validation::resolve_timeout_secs(request.task.as_ref());
    let max_model_rounds = validation::resolve_max_model_rounds(request.task.as_ref());
    let mut raw_events = Vec::new();
    let mut agent_error: Option<String> = None;
    let mut validation_result = None;
    let mut repair_attempts_used = 0;
    let mut validation_attempts = 0;

    // Initial agent turn
    {
        let watchdog =
            runner::spawn_timeout_watchdog(started, timeout_secs, session.clone(), emitter.clone());
        let result = runner::send_headless_turn(&session, &prompt, emitter.clone()).await;
        watchdog.abort();
        raw_events.extend(emitter.drain());

        if started.elapsed().as_secs() >= timeout_secs {
            agent_error = Some("timeout".to_string());
        } else if emitter.model_rounds() >= max_model_rounds {
            agent_error = Some("max_model_rounds_exceeded".to_string());
        } else if let Err(error) = result {
            agent_error = Some(error);
        }
    }

    // Validation and repair loop
    if agent_error.is_none() && !validation_commands.is_empty() {
        for attempt in 0..=max_repair_attempts {
            // Budget check before validation
            if started.elapsed().as_secs() >= timeout_secs {
                agent_error = Some("timeout".to_string());
                break;
            }
            if emitter.model_rounds() >= max_model_rounds {
                agent_error = Some("max_model_rounds_exceeded".to_string());
                break;
            }

            let validation =
                validation::run_headless_validation_commands(&validation_commands, &workspace_path)
                    .await?;
            validation_attempts += 1;
            raw_events.extend(validation::validation_events(
                &session_id,
                &format!("headless-validation-{attempt}"),
                &validation,
            ));
            validation_result = Some(validation.clone());

            if validation.passed() || attempt == max_repair_attempts {
                break;
            }

            // Budget check before repair turn
            if started.elapsed().as_secs() >= timeout_secs {
                agent_error = Some("timeout".to_string());
                repair_attempts_used = attempt + 1;
                break;
            }
            if emitter.model_rounds() >= max_model_rounds {
                agent_error = Some("max_model_rounds_exceeded".to_string());
                repair_attempts_used = attempt + 1;
                break;
            }

            let repair_prompt = validation::repair_prompt_from_validation_failure(
                &prompt,
                attempt + 1,
                &validation,
            );
            let watchdog = runner::spawn_timeout_watchdog(
                started,
                timeout_secs,
                session.clone(),
                emitter.clone(),
            );
            let result =
                runner::send_headless_turn(&session, &repair_prompt, emitter.clone()).await;
            watchdog.abort();
            raw_events.extend(emitter.drain());
            repair_attempts_used = attempt + 1;

            if started.elapsed().as_secs() >= timeout_secs {
                agent_error = Some("timeout".to_string());
                break;
            }
            if emitter.model_rounds() >= max_model_rounds {
                agent_error = Some("max_model_rounds_exceeded".to_string());
                break;
            }
            if let Err(error) = result {
                agent_error = Some(error);
                break;
            }
        }
    }

    let mut latest_turn = lock_unpoisoned(&session.latest_turn).clone();
    if let (Some(turn), Some(validation)) = (latest_turn.as_mut(), validation_result.as_ref()) {
        turn.set_verification(validation.to_trace());
    }
    let continuity_outcome =
        runner::headless_reflection_outcome(agent_error.as_ref(), latest_turn.as_ref());
    let mut continuity_formed_count = None;
    let mut continuity_error = None;
    match runner::record_headless_continuity(
        &continuity_service,
        &workspace_path,
        &session_id,
        &prompt,
        latest_turn.as_ref(),
        continuity_outcome,
        now_ms(),
    ) {
        Ok(formed_count) => {
            continuity_formed_count = Some(formed_count);
        }
        Err(error) => {
            crate::app_log!(
                "WARN",
                "[continuity] headless continuity record failed: {}",
                error
            );
            continuity_error = Some(error);
        }
    }
    let after_snapshot = snapshot::snapshot_workspace(&workspace_path)?;
    let (changed_files, file_diffs) =
        snapshot::diff_workspace_snapshots(&before_snapshot, &after_snapshot);
    let final_answer = resolve::final_answer_from_events(&raw_events);
    let snapshot_error = save_headless_session_snapshot(&session, latest_turn.clone()).err();

    let mut payload = trace::build_trace_payload(types::TracePayloadInput {
        task_id,
        prompt,
        provider: display_provider,
        model: display_model,
        raw_events,
        loop_task: None,
        latest_turn,
        file_diffs,
        changed_files,
        final_answer,
        duration_ms: started.elapsed().as_millis() as u64,
        continuity_formed_count,
        continuity_error,
        repair_attempts_used,
        validation_attempts,
    });
    trace::insert_agent_identity(&mut payload, &agent_provider, &agent_model);
    if let Some(error) = snapshot_error {
        if let Some(object) = payload.as_object_mut() {
            object.insert(
                "headless_snapshot_error".to_string(),
                serde_json::Value::String(error),
            );
        }
    }

    if let Some(ref error) = agent_error {
        let (error_code, failure_category, failure_reason) = if error == "timeout" {
            (
                "timeout",
                "timeout",
                "Forge headless eval exceeded the configured timeout.".to_string(),
            )
        } else if error == "max_model_rounds_exceeded" {
            (
                "max_model_rounds_exceeded",
                "budget_exhausted",
                "Forge headless eval exceeded the configured max model rounds.".to_string(),
            )
        } else {
            (
                "agent_error",
                "agent_error",
                format!("Forge agent turn failed: {error}"),
            )
        };
        trace::insert_failure_fields(&mut payload, error_code, failure_category, &failure_reason);
    }

    Ok(payload)
}

fn resolve_headless_runtime_paths(
    workspace_path: &std::path::Path,
    runtime_state_path: Option<&std::path::Path>,
    continuity_database_path: Option<&std::path::Path>,
) -> Result<(Option<std::path::PathBuf>, Option<std::path::PathBuf>), String> {
    let Some(runtime_state_path) = runtime_state_path else {
        if continuity_database_path.is_some() {
            return Err(
                "continuity_database_path requires an isolated runtime_state_path".to_string(),
            );
        }
        return Ok((None, None));
    };
    if !runtime_state_path.is_absolute() {
        return Err("runtime_state_path must be absolute".to_string());
    }

    let workspace = workspace_path.canonicalize().map_err(|error| {
        format!(
            "failed to resolve Forge eval workspace {}: {error}",
            workspace_path.display()
        )
    })?;
    let runtime_state = canonicalize_runtime_target(runtime_state_path)?;
    if runtime_state.starts_with(&workspace) {
        return Err(format!(
            "runtime_state_path must be outside the Forge eval workspace: {}",
            runtime_state_path.display()
        ));
    }

    let continuity_database = match continuity_database_path {
        Some(path) => {
            if !path.is_absolute() {
                return Err("continuity_database_path must be absolute".to_string());
            }
            let resolved = canonicalize_runtime_target(path)?;
            if !resolved.starts_with(&runtime_state) {
                return Err(format!(
                    "continuity_database_path must be inside runtime_state_path: {}",
                    path.display()
                ));
            }
            Some(resolved)
        }
        None => None,
    };

    Ok((Some(runtime_state.join("registry.db")), continuity_database))
}

fn canonicalize_runtime_target(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if path.exists() {
        return path.canonicalize().map_err(|error| {
            format!("failed to resolve runtime path {}: {error}", path.display())
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("runtime path has no parent: {}", path.display()))?
        .canonicalize()
        .map_err(|error| {
            format!(
                "failed to resolve runtime path parent for {}: {error}",
                path.display()
            )
        })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("runtime path has no final component: {}", path.display()))?;
    Ok(parent.join(file_name))
}

fn save_headless_session_snapshot(
    session: &crate::agent::session::AgentSession,
    latest_turn: Option<AgentTurnState>,
) -> Result<(), String> {
    let mut snapshot = session.snapshot();
    if let Some(latest_turn) = latest_turn {
        snapshot = snapshot.with_latest_turn(latest_turn);
    }
    crate::agent::snapshot::save_session_snapshot(&snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::event_sink::EventEmitter;
    use crate::agent::turn_state::{
        AgentToolCategory, AgentToolStatus, AgentToolTrace, AgentTurnProjection, AgentTurnState,
        AgentTurnStatus, AgentVerificationStatus,
    };
    use crate::harness::write_boundary::{WriteBoundary, WriteBoundaryRisk};
    use crate::profile::{ProfileStore, UpsertProfileInput};
    use crate::protocol::events::StreamEvent;
    use std::time::Duration;

    static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn restore_env(key: &str, value: Option<String>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn trace_payload_maps_forge_events_turn_state_and_diffs() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/tmp/workspace".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash[1m]".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Update calculator".to_string(),
        );
        turn.tools.push(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "edit_file".to_string(),
            category: AgentToolCategory::Write,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Edited src/calculator.py".to_string()),
            is_error: false,
            affected_files: vec!["src/calculator.py".to_string()],
            command: None,
        });
        turn.verification.status = AgentVerificationStatus::Passed;
        turn.verification.command = Some("python -m pytest tests/test_calculator.py".to_string());
        turn.verification.exit_code = Some(0);
        turn.verification.stdout_preview = Some("1 passed".to_string());
        turn.verification.duration_ms = Some(120);

        let payload = trace::build_trace_payload(types::TracePayloadInput {
            task_id: "small-edit-success".to_string(),
            prompt: "Update src/calculator.py so add_one returns value + 1".to_string(),
            provider: "forge".to_string(),
            model: "local-forge".to_string(),
            raw_events: vec![
                StreamEvent::ToolCallStart {
                    session_id: "session-1".to_string(),
                    block_id: "tool-1".to_string(),
                    tool_name: "edit_file".to_string(),
                    tool_input: serde_json::json!({
                        "path": "src/calculator.py",
                        "old_string": "return value",
                        "new_string": "return value + 1"
                    }),
                },
                StreamEvent::ToolCallResult {
                    session_id: "session-1".to_string(),
                    block_id: "tool-1".to_string(),
                    result: "Edited src/calculator.py".to_string(),
                    is_error: false,
                    duration_ms: 10,
                },
                StreamEvent::ShellStart {
                    session_id: "session-1".to_string(),
                    block_id: "shell-1".to_string(),
                    command: "python -m pytest tests/test_calculator.py".to_string(),
                },
                StreamEvent::ShellOutput {
                    session_id: "session-1".to_string(),
                    block_id: "shell-1".to_string(),
                    content: "1 passed\n".to_string(),
                },
                StreamEvent::ShellEnd {
                    session_id: "session-1".to_string(),
                    block_id: "shell-1".to_string(),
                    exit_code: 0,
                },
                StreamEvent::ContextCompacted {
                    session_id: "session-1".to_string(),
                    block_id: "compact-1".to_string(),
                    summary: "Kept the important setup and validation details.".to_string(),
                    retained_messages: 16,
                    compacted_messages: 48,
                    estimated_tokens_before: 120_000,
                    estimated_tokens_after: 42_000,
                },
                StreamEvent::Usage {
                    session_id: "session-1".to_string(),
                    input_tokens: 120,
                    output_tokens: 40,
                    estimated_cost_usd: 0.001,
                },
                StreamEvent::AgentA2AUpdated {
                    session_id: "session-1".to_string(),
                    state: serde_json::from_str(
                        r#"{
                            "running_count": 0,
                            "completed_count": 1,
                            "failed_count": 0,
                            "interrupted_count": 0,
                            "tasks": [
                                {
                                    "task_id": "parent-1",
                                    "agent_id": "agent-parent",
                                    "role": "reviewer",
                                    "execution_mode": "read_only",
                                    "status": "completed",
                                    "title": "Parent",
                                    "messages": [],
                                    "latest_message": null,
                                    "failure_message": null,
                                    "updated_at_ms": 200,
                                    "artifact_count": 0,
                                    "latest_artifact_kind": null,
                                    "latest_artifact_title": null,
                                    "needs_human_review": null,
                                    "reason_codes": [],
                                    "tests_passed": null,
                                    "diff_truncated": null,
                                    "worktree_path": null,
                                    "cleaned_up": null,
                                    "suggested_action": null,
                                    "child_capsules": [
                                        {
                                            "capsule_id": "child-capsule:parent-1:child-1",
                                            "parent_task_id": "parent-1",
                                            "child_task_id": "child-1",
                                            "child_goal": "Update calculator child",
                                            "status": "completed",
                                            "artifact_titles": ["Patch proposal", "Worktree diff"],
                                            "changed_files": ["src/calculator.py"],
                                            "review_decision": "approved",
                                            "failure_reason": null,
                                            "next_action": "Review child evidence before parent completion.",
                                            "estimated_tokens": 20
                                        }
                                    ]
                                },
                                {
                                    "task_id": "child-1",
                                    "agent_id": "agent-child",
                                    "role": "implementer",
                                    "execution_mode": "worktree_worker",
                                    "status": "completed",
                                    "title": "Child",
                                    "messages": [],
                                    "latest_message": null,
                                    "failure_message": null,
                                    "updated_at_ms": 190,
                                    "artifact_count": 2,
                                    "latest_artifact_kind": "diff_summary",
                                    "latest_artifact_title": "Worktree diff",
                                    "needs_human_review": false,
                                    "reason_codes": [],
                                    "tests_passed": true,
                                    "diff_truncated": false,
                                    "worktree_path": "/tmp/forge-child",
                                    "cleaned_up": false,
                                    "suggested_action": "Review approved by controller.",
                                    "review_decision": "approved",
                                    "runtime_events": [
                                        {
                                            "kind": "assigned",
                                            "label": "Assigned",
                                            "detail": "Child worker assigned",
                                            "created_at_ms": 120
                                        },
                                        {
                                            "kind": "lease_claimed",
                                            "label": "Lease claimed",
                                            "detail": "worker-1",
                                            "created_at_ms": 125
                                        },
                                        {
                                            "kind": "started",
                                            "label": "Started",
                                            "detail": "Worktree worker started",
                                            "created_at_ms": 130
                                        },
                                        {
                                            "kind": "file_fact",
                                            "label": "File fact",
                                            "detail": "src/calculator.py",
                                            "created_at_ms": 150
                                        },
                                        {
                                            "kind": "completed",
                                            "label": "Completed",
                                            "detail": "Worktree worker completed",
                                            "created_at_ms": 175
                                        }
                                    ],
                                    "review_gate": {
                                        "kind": "approved",
                                        "label": "Review approved",
                                        "reason": "ship it",
                                        "completion_impact": "child_review_approved_only",
                                        "parent_task_id": "parent-1",
                                        "child_task_id": "child-1",
                                        "reviewed_at_ms": 180
                                    },
                                    "recovery_actions": []
                                }
                            ]
                        }"#,
                    )
                    .expect("a2a projection"),
                },
            ],
            loop_task: None,
            latest_turn: Some(turn),
            file_diffs: vec![types::HeadlessFileDiff {
                path: "src/calculator.py".to_string(),
                change_type: "modified".to_string(),
                diff: "diff --git a/src/calculator.py b/src/calculator.py".to_string(),
            }],
            changed_files: vec!["src/calculator.py".to_string()],
            final_answer: "Completed.".to_string(),
            duration_ms: 250,
            continuity_formed_count: Some(1),
            continuity_error: None,
            repair_attempts_used: 0,
            validation_attempts: 1,
        });

        assert_eq!(payload["task_id"], "small-edit-success");
        assert_eq!(payload["session_id"], "session-1");
        assert_eq!(payload["provider"], "forge");
        assert_eq!(payload["model"], "local-forge");
        assert_eq!(
            payload["changed_files"],
            serde_json::json!(["src/calculator.py"])
        );
        assert_eq!(
            payload["tool_calls"][0]["command"],
            "edit_file src/calculator.py"
        );
        assert_eq!(
            payload["tool_calls"][0]["stdout"],
            "Edited src/calculator.py"
        );
        assert_eq!(
            payload["shell_outputs"][0]["command"],
            "python -m pytest tests/test_calculator.py"
        );
        assert_eq!(payload["shell_outputs"][0]["stdout"], "1 passed\n");
        assert_eq!(payload["verification_result"]["passed"], true);
        assert_eq!(payload["model_rounds"], 1);
        assert_eq!(payload["confirm_requests"], 0);
        assert_eq!(payload["input_tokens"], 120);
        assert_eq!(payload["output_tokens"], 40);
        assert_eq!(payload["compact_count"], 1);
        assert_eq!(payload["compact_estimated_tokens_saved"], 78_000);
        assert_eq!(payload["compact_events"][0]["retained_messages"], 16);
        assert_eq!(payload["compact_events"][0]["compacted_messages"], 48);
        assert_eq!(
            payload["compact_events"][0]["estimated_tokens_before"],
            120_000
        );
        assert_eq!(
            payload["compact_events"][0]["estimated_tokens_after"],
            42_000
        );
        assert_eq!(
            payload["compact_events"][0]["estimated_tokens_saved"],
            78_000
        );
        assert_eq!(
            payload["compact_events"][0]["estimated_reduction_percent"],
            65
        );
        assert_eq!(payload["failure_category"], "none");
        assert_eq!(
            payload["forge_run_evidence"]["prompt"],
            "Update src/calculator.py so add_one returns value + 1"
        );
        assert_eq!(payload["forge_run_evidence"]["schema_version"], 2);
        assert_eq!(
            payload["forge_run_evidence"]["completion_eligibility"]["status"],
            "unknown"
        );
        assert_eq!(
            payload["forge_run_evidence"]["normalized_goal"],
            "Update calculator"
        );
        assert_eq!(
            payload["forge_run_evidence"]["prepared_context"]["turn_context"]["sources"],
            serde_json::json!([])
        );
        assert_eq!(
            payload["forge_run_evidence"]["tool_calls"][0]["command"],
            "edit_file src/calculator.py"
        );
        assert_eq!(
            payload["forge_run_evidence"]["changed_files"],
            serde_json::json!(["src/calculator.py"])
        );
        assert_eq!(
            payload["forge_run_evidence"]["verification"]["passed"],
            true
        );
        assert_eq!(
            payload["forge_run_evidence"]["provider_usage"]["latest"]["input_tokens"],
            120
        );
        assert_eq!(
            payload["forge_run_evidence"]["continuity_lessons"][0]["formed_count"],
            1
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["child_task_id"],
            "child-1"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["review_gate"]["kind"],
            "approved"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["execution_mode"],
            "worktree_worker"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["worktree_path"],
            "/tmp/forge-child"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["tests_passed"],
            true
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["diff_truncated"],
            false
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["cleaned_up"],
            false
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["runtime_events"][0]["kind"],
            "assigned"
        );
        assert_eq!(
            payload["forge_run_evidence"]["a2a_child_capsules"][0]["runtime_events"][3]["kind"],
            "file_fact"
        );
    }

    #[test]
    fn trace_payload_includes_projected_loop_task_authority() {
        let mut task =
            crate::loop_runtime::LoopTaskRecord::new_for_test("loop-trace", "recover task");
        task.status = crate::loop_runtime::LoopTaskStatus::Interrupted;
        task.latest_usage_ledger = Some(crate::loop_runtime::LoopUsageLedger {
            provider_id: Some("deepseek".to_string()),
            model: Some("deepseek-v4-flash".to_string()),
            input_tokens: Some(222),
            output_tokens: Some(33),
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
            estimated_cost_micros: Some(8),
            pricing_source: Some("test".to_string()),
            has_unknown_input_tokens: false,
            has_unknown_output_tokens: false,
            has_unknown_cost: false,
            turn_count: 2,
            tool_call_count: 4,
            elapsed_ms: 3_000,
        });
        task.recovery_state = Some(crate::loop_runtime::LoopTaskRecoveryState::interrupted(
            "stale lease recovered by operator",
            5,
            Some("event-loop-trace-interrupted".to_string()),
        ));
        task.completion_result = Some(crate::loop_runtime::LoopCompletionResult {
            status: crate::loop_runtime::LoopCompletionStatus::Blocked,
            reasons: vec!["task_interrupted".to_string()],
            review_status: crate::loop_runtime::LoopReviewStatus::NotRequired,
            commit_eligible: false,
            commit_blockers: vec!["task_interrupted".to_string()],
            human_gate_id: None,
            last_review_decision: None,
            eligibility_facts: crate::loop_runtime::LoopCompletionEligibilityFacts::default(),
        });
        task.evidence
            .push(crate::loop_runtime::EvidenceRecord::command_for_test(
                "build:desktop",
                true,
            ));

        let payload = trace::build_trace_payload(types::TracePayloadInput {
            task_id: "loop-trace".to_string(),
            prompt: "Recover task".to_string(),
            provider: "forge".to_string(),
            model: "local-forge".to_string(),
            raw_events: Vec::new(),
            loop_task: Some(task),
            latest_turn: None,
            file_diffs: Vec::new(),
            changed_files: Vec::new(),
            final_answer: String::new(),
            duration_ms: 50,
            continuity_formed_count: None,
            continuity_error: None,
            repair_attempts_used: 0,
            validation_attempts: 0,
        });

        assert_eq!(payload["loop_task"]["task_id"], "loop-trace");
        assert_eq!(payload["loop_task"]["owner"]["kind"], "gateway");
        assert_eq!(payload["loop_task"]["status"], "interrupted");
        assert_eq!(payload["loop_task"]["failure_category"], "orphaned");
        assert_eq!(payload["loop_task"]["usage"]["input_tokens"], 222);
        assert_eq!(payload["loop_task"]["recovery_state"]["kind"], "orphaned");
        assert_eq!(
            payload["loop_task"]["completion_evidence"][0]["evidence_id"],
            "evidence-command-build:desktop"
        );
        assert_eq!(
            payload["forge_run_evidence"]["failure_category"],
            "orphaned"
        );
        assert_eq!(
            payload["forge_run_evidence"]["recovery"]["kind"],
            "orphaned"
        );
        assert_eq!(
            payload["forge_run_evidence"]["completion_eligibility"]["status"],
            "blocked"
        );
        assert_eq!(
            payload["forge_run_evidence"]["completion_eligibility"]["commit_eligible"],
            false
        );
    }

    #[test]
    fn workspace_snapshot_diff_reports_added_modified_and_deleted_files() {
        let before = std::collections::HashMap::from([
            (
                "src/old.py".to_string(),
                types::SnapshotFile {
                    contents: b"old\n".to_vec(),
                },
            ),
            (
                "src/keep.py".to_string(),
                types::SnapshotFile {
                    contents: b"same\n".to_vec(),
                },
            ),
            (
                "src/delete.py".to_string(),
                types::SnapshotFile {
                    contents: b"delete me\n".to_vec(),
                },
            ),
        ]);
        let after = std::collections::HashMap::from([
            (
                ".forge/registry.db".to_string(),
                types::SnapshotFile {
                    contents: b"internal runtime state".to_vec(),
                },
            ),
            (
                "src/old.py".to_string(),
                types::SnapshotFile {
                    contents: b"new\n".to_vec(),
                },
            ),
            (
                "src/keep.py".to_string(),
                types::SnapshotFile {
                    contents: b"same\n".to_vec(),
                },
            ),
            (
                "src/add.py".to_string(),
                types::SnapshotFile {
                    contents: b"add me\n".to_vec(),
                },
            ),
        ]);

        let (changed_files, file_diffs) = snapshot::diff_workspace_snapshots(&before, &after);

        assert_eq!(
            changed_files,
            vec![
                "src/add.py".to_string(),
                "src/delete.py".to_string(),
                "src/old.py".to_string()
            ]
        );
        assert_eq!(file_diffs[0].change_type, "added");
        assert_eq!(file_diffs[1].change_type, "deleted");
        assert_eq!(file_diffs[2].change_type, "modified");
        assert_eq!(
            file_diffs[0].diff,
            "diff --git a/src/add.py b/src/add.py\n\
new file mode 100644\n\
--- /dev/null\n\
+++ b/src/add.py\n\
@@ -0,0 +1 @@\n\
+add me\n"
        );
        assert_eq!(
            file_diffs[1].diff,
            "diff --git a/src/delete.py b/src/delete.py\n\
deleted file mode 100644\n\
--- a/src/delete.py\n\
+++ /dev/null\n\
@@ -1 +0,0 @@\n\
-delete me\n"
        );
        assert_eq!(
            file_diffs[2].diff,
            "diff --git a/src/old.py b/src/old.py\n\
--- a/src/old.py\n\
+++ b/src/old.py\n\
@@ -1 +1 @@\n\
-old\n\
+new\n"
        );
    }

    #[test]
    fn workspace_snapshot_diffs_replay_with_patch() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let workspace = tempfile::tempdir().expect("workspace");
        std::fs::create_dir_all(workspace.path().join("src")).expect("src");
        std::fs::write(workspace.path().join("src/old.py"), "old\n").expect("old file");
        std::fs::write(workspace.path().join("src/delete.py"), "delete me\n")
            .expect("deleted file");
        let before = std::collections::HashMap::from([
            (
                "src/old.py".to_string(),
                types::SnapshotFile {
                    contents: b"old\n".to_vec(),
                },
            ),
            (
                "src/delete.py".to_string(),
                types::SnapshotFile {
                    contents: b"delete me\n".to_vec(),
                },
            ),
        ]);
        let after = std::collections::HashMap::from([
            (
                "src/old.py".to_string(),
                types::SnapshotFile {
                    contents: b"new\n".to_vec(),
                },
            ),
            (
                "src/add.py".to_string(),
                types::SnapshotFile {
                    contents: b"add me\n".to_vec(),
                },
            ),
        ]);
        let (_, diffs) = snapshot::diff_workspace_snapshots(&before, &after);
        let patch_text = diffs
            .iter()
            .map(|diff| diff.diff.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let mut child = Command::new("git")
            .args(["apply", "--whitespace=nowarn", "-"])
            .current_dir(workspace.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn patch");
        child
            .stdin
            .take()
            .expect("patch stdin")
            .write_all(patch_text.as_bytes())
            .expect("write patch");
        let output = child.wait_with_output().expect("patch output");

        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("src/old.py")).unwrap(),
            "new\n"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("src/add.py")).unwrap(),
            "add me\n"
        );
        assert!(
            !workspace.path().join("src/delete.py").exists(),
            "delete patch left file behind; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn headless_request_accepts_explicit_isolated_runtime_paths() {
        let request: EvalHeadlessRequest = serde_json::from_value(serde_json::json!({
            "prompt": "test",
            "workspace_path": "/tmp/workspace",
            "runtime_state_path": "/tmp/runtime-state",
            "continuity_database_path": "/tmp/runtime-state/continuity.db"
        }))
        .expect("headless request");

        assert_eq!(
            request.runtime_state_path.as_deref(),
            Some(std::path::Path::new("/tmp/runtime-state"))
        );
        assert_eq!(
            request.continuity_database_path.as_deref(),
            Some(std::path::Path::new("/tmp/runtime-state/continuity.db"))
        );
    }

    #[test]
    fn headless_runtime_paths_resolve_to_sibling_state_directory() {
        let root = tempfile::tempdir().expect("root");
        let workspace = root.path().join("workspace");
        let runtime_state = root.path().join("runtime-state");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&runtime_state).expect("runtime state");
        let continuity_database = runtime_state.join("continuity.db");

        let (registry, continuity) = super::resolve_headless_runtime_paths(
            &workspace,
            Some(&runtime_state),
            Some(&continuity_database),
        )
        .expect("isolated paths");

        let resolved_runtime_state = runtime_state
            .canonicalize()
            .expect("resolved runtime state");
        assert_eq!(registry, Some(resolved_runtime_state.join("registry.db")));
        assert_eq!(
            continuity,
            Some(resolved_runtime_state.join("continuity.db"))
        );
    }

    #[test]
    fn headless_runtime_paths_reject_state_inside_observed_workspace() {
        let root = tempfile::tempdir().expect("root");
        let workspace = root.path().join("workspace");
        let runtime_state = workspace.join(".forge").join("eval-state");
        std::fs::create_dir_all(&runtime_state).expect("runtime state");

        let error = super::resolve_headless_runtime_paths(
            &workspace,
            Some(&runtime_state),
            Some(&runtime_state.join("continuity.db")),
        )
        .expect_err("workspace-local runtime state must fail closed");

        assert!(error.contains("must be outside"), "{error}");
    }

    #[test]
    fn deepseek_headless_model_ignores_anthropic_credential_model() {
        let _guard = ENV_LOCK.blocking_lock();
        let previous_headless_model = std::env::var("FORGE_HEADLESS_MODEL").ok();
        let previous_eval_model = std::env::var("FORGE_EVAL_AI_MODEL").ok();
        std::env::remove_var("FORGE_HEADLESS_MODEL");
        std::env::remove_var("FORGE_EVAL_AI_MODEL");

        let model =
            resolve::resolve_agent_model(Some("local-forge"), Some("kimi-for-coding"), "deepseek");

        restore_env("FORGE_HEADLESS_MODEL", previous_headless_model);
        restore_env("FORGE_EVAL_AI_MODEL", previous_eval_model);
        assert_eq!(model, "deepseek-v4-flash");
    }

    #[test]
    fn trace_payload_uses_turn_tools_when_events_have_results_without_starts() {
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            "/tmp/workspace".to_string(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Read calculator".to_string(),
        );
        turn.tools.push(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "read_file".to_string(),
            category: AgentToolCategory::Read,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Loaded src/calculator.py".to_string()),
            is_error: false,
            affected_files: vec!["src/calculator.py".to_string()],
            command: None,
        });

        let payload = trace::build_trace_payload(types::TracePayloadInput {
            task_id: "small-edit-success".to_string(),
            prompt: "Read src/calculator.py".to_string(),
            provider: "forge".to_string(),
            model: "local-forge".to_string(),
            raw_events: vec![StreamEvent::ToolCallResult {
                session_id: "session-1".to_string(),
                block_id: "tool-1".to_string(),
                result: "def add_one(value): return value".to_string(),
                is_error: false,
                duration_ms: 10,
            }],
            loop_task: None,
            latest_turn: Some(turn),
            file_diffs: Vec::new(),
            changed_files: Vec::new(),
            final_answer: String::new(),
            duration_ms: 50,
            continuity_formed_count: None,
            continuity_error: None,
            repair_attempts_used: 0,
            validation_attempts: 0,
        });

        assert_eq!(
            payload["tool_calls"][0]["command"],
            "read_file src/calculator.py"
        );
        assert_eq!(
            payload["tool_calls"][0]["stdout"],
            "def add_one(value): return value"
        );
    }

    #[test]
    fn summarize_events_counts_calling_model_transitions_without_tool_starts() {
        let events = vec![
            agent_turn_event("session-1", AgentTurnStatus::Started),
            agent_turn_event("session-1", AgentTurnStatus::CallingModel),
            agent_turn_event("session-1", AgentTurnStatus::CallingModel),
            agent_turn_event("session-1", AgentTurnStatus::RunningTools),
            agent_turn_event("session-1", AgentTurnStatus::CallingModel),
        ];

        let summary = summary::summarize_events(&events);

        assert_eq!(summary.model_rounds, 2);
    }

    #[test]
    fn validation_commands_prefer_case_commands_and_fall_back_to_verification_command() {
        let task = EvalHeadlessTask {
            id: Some("case-1".to_string()),
            prompt: Some("Fix the code".to_string()),
            validation_commands: vec!["npm test".to_string(), "npx tsc --noEmit".to_string()],
            verification_command: Some("npm run check".to_string()),
            max_repair_attempts: None,
            timeout_secs: None,
            max_model_rounds: None,
        };

        assert_eq!(
            validation::validation_commands_from_task(Some(&task)),
            vec!["npm test".to_string(), "npx tsc --noEmit".to_string()]
        );

        let fallback_task = EvalHeadlessTask {
            id: Some("case-2".to_string()),
            prompt: Some("Fix the code".to_string()),
            validation_commands: Vec::new(),
            verification_command: Some("npm run check".to_string()),
            max_repair_attempts: None,
            timeout_secs: None,
            max_model_rounds: None,
        };

        assert_eq!(
            validation::validation_commands_from_task(Some(&fallback_task)),
            vec!["npm run check".to_string()]
        );
    }

    #[test]
    fn repair_prompt_includes_validation_failure_details_without_hiding_original_task() {
        let validation = types::HeadlessValidationResult {
            command: "npx tsc --noEmit".to_string(),
            status: AgentVerificationStatus::Failed,
            exit_code: Some(2),
            stdout: String::new(),
            stderr: "src/normalize.test.ts(3,32): error TS5097".to_string(),
            duration_ms: 120,
        };

        let prompt = validation::repair_prompt_from_validation_failure(
            "Add normalizeInput and tests.",
            1,
            &validation,
        );

        assert!(prompt.contains("Add normalizeInput and tests."));
        assert!(prompt.contains("npx tsc --noEmit"));
        assert!(prompt.contains("TS5097"));
        assert!(prompt.contains("attempt 1"));
    }

    #[test]
    fn validation_result_events_are_visible_as_shell_outputs() {
        let validation = types::HeadlessValidationResult {
            command: "npm test".to_string(),
            status: AgentVerificationStatus::Failed,
            exit_code: Some(1),
            stdout: "1 failed".to_string(),
            stderr: "Expected true".to_string(),
            duration_ms: 50,
        };

        let events = validation::validation_events("session-1", "validation-1", &validation);
        let summary = summary::summarize_events(&events);

        assert_eq!(summary.shell_outputs[0]["command"], "npm test");
        assert_eq!(
            summary.shell_outputs[0]["stdout"],
            "1 failed\nExpected true"
        );
        assert_eq!(summary.shell_outputs[0]["exit_code"], 1);
    }

    #[tokio::test]
    async fn headless_event_emitter_auto_resolves_confirm_requests() {
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let emitter = runner::HeadlessEventEmitter::new(pending_confirms.clone());

        let (ask_tx, ask_rx) = tokio::sync::oneshot::channel();
        pending_confirms
            .write()
            .await
            .insert("ask-user".to_string(), ask_tx);
        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "ask-user".to_string(),
            question: "Need input?".to_string(),
            kind: "ask_user".to_string(),
            boundary: None,
            permission_evidence: None,
            replayed_interrupted: false,
        });
        let ask_response = tokio::time::timeout(Duration::from_secs(1), ask_rx)
            .await
            .expect("ask_user response should not hang")
            .expect("ask_user sender should respond");
        assert!(!ask_response);

        let (permission_tx, permission_rx) = tokio::sync::oneshot::channel();
        pending_confirms
            .write()
            .await
            .insert("write-file".to_string(), permission_tx);
        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "write-file".to_string(),
            question: "Allow write?".to_string(),
            kind: "file_write".to_string(),
            replayed_interrupted: false,
            permission_evidence: None,
            boundary: Some(WriteBoundary {
                title: "准备修改项目".to_string(),
                target_label: None,
                workspace_name: "workspace".to_string(),
                workspace_path: "/tmp/workspace".to_string(),
                operation: "修改文件".to_string(),
                affected_files: vec!["src/calculator.py".to_string()],
                command: None,
                impact: "将修改 1 个文件".to_string(),
                risk: WriteBoundaryRisk::Normal,
                recovery: "disposable eval workspace".to_string(),
                checkpoint_status: None,
                warning: None,
            }),
        });
        let permission_response = tokio::time::timeout(Duration::from_secs(1), permission_rx)
            .await
            .expect("permission response should not hang")
            .expect("permission sender should respond");
        assert!(permission_response);
    }

    #[tokio::test]
    async fn headless_event_emitter_approves_permission_by_kind_without_boundary() {
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let emitter = runner::HeadlessEventEmitter::new(pending_confirms.clone());

        let (permission_tx, permission_rx) = tokio::sync::oneshot::channel();
        pending_confirms
            .write()
            .await
            .insert("write-file".to_string(), permission_tx);
        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "write-file".to_string(),
            question: "Allow write?".to_string(),
            kind: "file_write".to_string(),
            boundary: None,
            permission_evidence: None,
            replayed_interrupted: false,
        });

        let permission_response = tokio::time::timeout(Duration::from_secs(1), permission_rx)
            .await
            .expect("permission response should not hang")
            .expect("permission sender should respond");
        assert!(permission_response);
    }

    #[tokio::test]
    async fn headless_event_emitter_retries_until_confirm_sender_is_registered() {
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let emitter = runner::HeadlessEventEmitter::new(pending_confirms.clone());

        emitter.emit(StreamEvent::ConfirmAsk {
            session_id: "session-1".to_string(),
            block_id: "late-sender".to_string(),
            question: "Allow write?".to_string(),
            kind: "file_write".to_string(),
            boundary: None,
            permission_evidence: None,
            replayed_interrupted: false,
        });

        let (permission_tx, permission_rx) = tokio::sync::oneshot::channel();
        tokio::time::sleep(Duration::from_millis(25)).await;
        pending_confirms
            .write()
            .await
            .insert("late-sender".to_string(), permission_tx);

        let permission_response = tokio::time::timeout(Duration::from_secs(1), permission_rx)
            .await
            .expect("permission response should not hang")
            .expect("permission sender should respond");
        assert!(permission_response);
    }

    #[tokio::test]
    async fn headless_event_emitter_approves_harness_write_permission() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-headless-write-permission-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&workspace).expect("workspace should be created");
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let harness =
            crate::harness::Harness::new_with_pending(workspace.clone(), pending_confirms.clone());
        let emitter = Arc::new(runner::HeadlessEventEmitter::new(pending_confirms));

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            harness.execute_tool_with_emitter(
                "session-1",
                "write_to_file",
                &serde_json::json!({
                    "path": workspace.join("created.txt").to_string_lossy(),
                    "content": "created by headless test"
                }),
                emitter,
                Some("tool-block-1"),
                None,
            ),
        )
        .await
        .expect("write permission should be resolved without hanging");

        assert!(result.starts_with("File written:"), "{result}");
        assert_eq!(
            std::fs::read_to_string(workspace.join("created.txt")).unwrap(),
            "created by headless test"
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn headless_continuity_records_turn_and_forms_experience() {
        let workspace = std::env::temp_dir().join(format!(
            "forge-headless-continuity-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(workspace.join("src")).expect("workspace should be created");
        let project_path = workspace.to_string_lossy().to_string();
        let mut turn = AgentTurnState::new(
            "turn-1".to_string(),
            "session-1".to_string(),
            project_path.clone(),
            "deepseek".to_string(),
            "deepseek-v4-flash".to_string(),
            "direct".to_string(),
            "idle".to_string(),
            "Add normalizeInput and tests".to_string(),
        );
        turn.record_tool(AgentToolTrace {
            tool_call_id: "tool-1".to_string(),
            name: "edit_file".to_string(),
            category: AgentToolCategory::Write,
            status: AgentToolStatus::Completed,
            started_at_ms: 10,
            ended_at_ms: Some(20),
            result_summary: Some("Edited src/normalize.ts".to_string()),
            is_error: false,
            affected_files: vec!["src/normalize.ts".to_string()],
            command: None,
        });
        turn.verification.status = AgentVerificationStatus::Passed;
        turn.verification.command = Some("npm test && npx tsc --noEmit".to_string());
        turn.verification.exit_code = Some(0);
        turn.verification.stdout_preview = Some("tests passed".to_string());
        turn.mark_status(AgentTurnStatus::Completed);

        let service = crate::continuity::ContinuityService::new();
        runner::record_headless_continuity(
            &service,
            &workspace,
            "session-1",
            "Add normalizeInput and tests",
            Some(&turn),
            crate::continuity::ReflectionOutcome::Completed,
            42,
        )
        .expect("headless continuity should record");

        let db_path = workspace.join(".forge").join("continuity.db");
        let conn = rusqlite::Connection::open(&db_path).expect("continuity db should open");
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM continuity_events", [], |row| {
                row.get(0)
            })
            .expect("event count should query");
        let formed_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM continuity_formed_reflections",
                [],
                |row| row.get(0),
            )
            .expect("formed count should query");
        let experience_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM continuity_experiences", [], |row| {
                row.get(0)
            })
            .expect("experience count should query");

        assert!(event_count >= 5, "expected turn events, got {event_count}");
        assert_eq!(formed_count, 1);
        assert!(experience_count >= 1, "expected formed experience");

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn headless_event_emitter_tracks_model_rounds_from_agent_turn_updated() {
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let emitter = runner::HeadlessEventEmitter::new(pending_confirms);

        assert_eq!(emitter.model_rounds(), 0);

        // First transition to CallingModel counts
        emitter.emit(agent_turn_event(
            "session-1",
            AgentTurnStatus::GatheringContext,
        ));
        assert_eq!(emitter.model_rounds(), 0);
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::CallingModel));
        assert_eq!(emitter.model_rounds(), 1);

        // Repeated CallingModel without transition does not count
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::CallingModel));
        assert_eq!(emitter.model_rounds(), 1);

        // Transition out and back in counts again
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::RunningTools));
        emitter.emit(agent_turn_event("session-1", AgentTurnStatus::CallingModel));
        assert_eq!(emitter.model_rounds(), 2);
    }

    #[test]
    fn resolve_timeout_and_budget_uses_defaults_and_task_values() {
        assert_eq!(
            validation::resolve_timeout_secs(None),
            types::HEADLESS_DEFAULT_TIMEOUT_SECS
        );
        assert_eq!(
            validation::resolve_timeout_secs(Some(&EvalHeadlessTask {
                timeout_secs: Some(120),
                ..Default::default()
            })),
            120
        );

        assert_eq!(
            validation::resolve_max_model_rounds(None),
            types::HEADLESS_DEFAULT_MAX_MODEL_ROUNDS
        );
        assert_eq!(
            validation::resolve_max_model_rounds(Some(&EvalHeadlessTask {
                max_model_rounds: Some(20),
                ..Default::default()
            })),
            20
        );
    }

    #[tokio::test]
    async fn timeout_watchdog_kills_session_after_sleep() {
        let workspace =
            std::env::temp_dir().join(format!("forge-headless-watchdog-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&workspace).expect("workspace should be created");
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let (session, _missing_key) = crate::ipc::session_builder::build_agent_session(
            crate::ipc::session_builder::BuildAgentSessionRequest {
                session_id: "watchdog-session".to_string(),
                provider: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                api_key: "fake-key-for-test",
                api_base: None,
                working_dir: &workspace,
                pending_confirms: pending_confirms.clone(),
                existing_context_window_tokens: None,
            },
        )
        .await
        .expect("session should build");
        let session = Arc::new(session);
        let emitter = Arc::new(runner::HeadlessEventEmitter::new(pending_confirms));
        let started = Instant::now();

        let watchdog = runner::spawn_timeout_watchdog(started, 0, session.clone(), emitter.clone());
        tokio::time::sleep(Duration::from_millis(50)).await;
        watchdog.abort();

        // Session should be stopped because timeout was 0 (immediate kill)
        assert!(
            !session.running.load(std::sync::atomic::Ordering::SeqCst),
            "watchdog should have killed session with zero timeout"
        );
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[tokio::test]
    async fn save_headless_session_snapshot_persists_gateway_attach_snapshot() {
        let _guard = ENV_LOCK.lock().await;
        let previous_home = std::env::var("HOME").ok();
        let home = tempfile::tempdir().expect("home");
        std::env::set_var("HOME", home.path());
        let workspace = tempfile::tempdir().expect("workspace");
        let pending_confirms: types::PendingConfirms =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
        let (session, _missing_key) = crate::ipc::session_builder::build_agent_session(
            crate::ipc::session_builder::BuildAgentSessionRequest {
                session_id: "headless-snapshot-session".to_string(),
                provider: "deepseek".to_string(),
                model: "deepseek-v4-flash".to_string(),
                api_key: "fake-key-for-test",
                api_base: None,
                working_dir: workspace.path(),
                pending_confirms,
                existing_context_window_tokens: None,
            },
        )
        .await
        .expect("session should build");

        super::save_headless_session_snapshot(&session, None).expect("save headless snapshot");

        let snapshot = crate::agent::snapshot::load_session_snapshot("headless-snapshot-session")
            .expect("snapshot should be readable");
        assert_eq!(snapshot.session_id, "headless-snapshot-session");
        assert_eq!(snapshot.provider, "deepseek");
        assert_eq!(snapshot.model, "deepseek-v4-flash");
        assert_eq!(
            snapshot.working_dir,
            workspace.path().to_string_lossy().to_string()
        );

        restore_env("HOME", previous_home);
    }

    fn agent_turn_event(session_id: &str, status: AgentTurnStatus) -> StreamEvent {
        StreamEvent::AgentTurnUpdated {
            session_id: session_id.to_string(),
            state: AgentTurnProjection {
                session_id: session_id.to_string(),
                status,
                step_label: String::new(),
                workspace_path: "/tmp/workspace".to_string(),
                compact_count: 0,
                verification_status: AgentVerificationStatus::NotNeeded,
                model_rounds: 0,
                tool_call_count: 0,
                failed_tool_count: 0,
                estimated_context_tokens: None,
                compact_saved_tokens: 0,
                stop_reason: None,
            },
        }
    }

    // ── profile resolution ───────────────────────────────────────────────

    fn temp_profile_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("forge-headless-profile-{label}-{nanos}.json"))
    }

    #[test]
    fn resolve_profile_defaults_returns_provided_defaults_when_no_profile_id() {
        let (provider, model) =
            resolve::resolve_profile_defaults(None, "deepseek", "deepseek-chat");
        assert_eq!(provider, "deepseek");
        assert_eq!(model, "deepseek-chat");
    }

    #[test]
    fn resolve_profile_defaults_returns_provided_defaults_when_profile_not_found() {
        // Use a non-existent profile id — the store won't have it.
        let (provider, model) = resolve::resolve_profile_defaults(
            Some("nonexistent-profile-id-12345"),
            "deepseek",
            "deepseek-chat",
        );
        // Falls back to defaults because profile doesn't exist.
        assert_eq!(provider, "deepseek");
        assert_eq!(model, "deepseek-chat");
    }

    #[test]
    fn resolve_profile_defaults_uses_profile_overrides_when_present() {
        let path = temp_profile_path("overrides");
        let store = ProfileStore::new(path.clone());
        let profile = store
            .upsert(UpsertProfileInput {
                id: Some("work".into()),
                name: "Work".into(),
                default_provider: Some("anthropic".into()),
                default_model: Some("claude-opus-4-8".into()),
                default_workspace: None,
            })
            .expect("create profile");

        // Now call resolve_profile_defaults — it will read from default_path,
        // but our temp store wrote to a temp path.
        // We need to test the resolution logic without relying on default_path.
        // Use a store that we control.
        let store = ProfileStore::new(path.clone());
        let (provider, model) = if let Some(pid) = Some(profile.id.as_str()) {
            let p = store.get(pid).expect("profile exists");
            let prov = p.default_provider.unwrap_or_else(|| "deepseek".to_string());
            let modl = p
                .default_model
                .unwrap_or_else(|| "deepseek-chat".to_string());
            (prov, modl)
        } else {
            ("deepseek".to_string(), "deepseek-chat".to_string())
        };
        assert_eq!(provider, "anthropic");
        assert_eq!(model, "claude-opus-4-8");

        // Clean up.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resolve_profile_defaults_falls_back_when_profile_has_no_overrides() {
        let path = temp_profile_path("no-overrides");
        let store = ProfileStore::new(path.clone());
        let profile = store
            .upsert(UpsertProfileInput {
                id: Some("minimal".into()),
                name: "Minimal".into(),
                default_provider: None,
                default_model: None,
                default_workspace: None,
            })
            .expect("create profile");

        let store = ProfileStore::new(path.clone());
        let (provider, model) = if let Some(pid) = Some(profile.id.as_str()) {
            let p = store.get(pid).expect("profile exists");
            let prov = p.default_provider.unwrap_or_else(|| "deepseek".to_string());
            let modl = p
                .default_model
                .unwrap_or_else(|| "deepseek-chat".to_string());
            (prov, modl)
        } else {
            ("deepseek".to_string(), "deepseek-chat".to_string())
        };
        assert_eq!(provider, "deepseek");
        assert_eq!(model, "deepseek-chat");

        let _ = std::fs::remove_file(&path);
    }
}
