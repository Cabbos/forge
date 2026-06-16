//! Local scheduled task store — CRUD for declarative cron-like tasks.
//!
//! Persisted as JSON at `~/.forge/scheduler.json`.  The store owns the task
//! model and history; IPC can pair it with the gateway trigger queue so run-now
//! and due tasks are picked up by the background runtime.
//!
//! ## Current limitations
//!
//! - Scheduler firing queues gateway triggers; the gateway runner owns
//!   headless execution and records attempt results in the gateway run ledger.
//! - The gateway daemon owns the background tick; the frontend can still drive
//!   `run_scheduled_task_now` and poll `list_scheduled_tasks` for display.

use crate::gateway::webhook::{PendingTrigger, TriggerStore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Schema ───────────────────────────────────────────────────────────────────

const CURRENT_SCHEMA_VERSION: u32 = 1;
const MANUAL_NEXT_RUN_AT_MS: u64 = 253_402_300_799_000; // 9999-12-31T23:59:59Z, JS-safe.

/// A scheduled task — declarative cron-like item persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledTask {
    pub id: String,
    pub title: String,
    /// Prompt / command text the task would execute when triggered.
    pub text: String,
    pub enabled: bool,
    /// Interval in seconds between scheduled runs (0 = manual only).
    pub interval_seconds: u64,
    /// Unix-epoch milliseconds for the next scheduled run.
    pub next_run_at_ms: u64,
    /// Unix-epoch milliseconds of the last run, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<u64>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    /// Last error message, if the most recent run failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// A single run-history entry recorded after a task fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunHistoryEntry {
    pub id: String,
    pub task_id: String,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    /// "queued" | "completed" | "skipped" | "error"
    pub status: String,
    /// Short message / log excerpt (≤ 200 chars in practice).
    pub message: String,
}

/// On-disk representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchedulerFile {
    schema_version: u32,
    tasks: Vec<ScheduledTask>,
    history: Vec<RunHistoryEntry>,
}

// ── Input / output helpers ───────────────────────────────────────────────────

/// Input for creating or updating a scheduled task via IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertScheduledTaskInput {
    /// When present the store updates the existing task; otherwise creates.
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub interval_seconds: u64,
    #[serde(default)]
    pub profile_id: Option<String>,
}

/// Payload returned by list_scheduled_tasks IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerListPayload {
    pub tasks: Vec<ScheduledTask>,
    /// Most recent history entries across all tasks, newest first (max 50).
    pub recent_history: Vec<RunHistoryEntry>,
    /// Human-readable message if the JSON file was corrupted on load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_error: Option<String>,
}

// ── Store ────────────────────────────────────────────────────────────────────

const MAX_HISTORY: usize = 500;
const SCHEDULER_TICK_INTERVAL_SECS: u64 = 60;

pub struct SchedulerStore {
    path: PathBuf,
    tasks: Mutex<Vec<ScheduledTask>>,
    history: Mutex<Vec<RunHistoryEntry>>,
    load_error: Mutex<Option<String>>,
}

impl SchedulerStore {
    // -- construction ----------------------------------------------------------

    /// Returns the default path `~/.forge/scheduler.json`.
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".forge").join("scheduler.json")
    }

    /// Creates a store, loading from `path` if it exists.
    pub fn new(path: PathBuf) -> Self {
        let (tasks, history, load_error) = load_scheduler(&path);
        Self {
            path,
            tasks: Mutex::new(tasks),
            history: Mutex::new(history),
            load_error: Mutex::new(load_error),
        }
    }

    /// Returns the last load error (if any) so diagnostics / UI can surface it.
    pub fn load_error(&self) -> Option<String> {
        self.load_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // -- queries ---------------------------------------------------------------

    /// List all tasks and recent history.
    pub fn list_payload(&self) -> SchedulerListPayload {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());

        let recent = if history.len() <= 50 {
            history.clone()
        } else {
            history[..50].to_vec()
        };

        SchedulerListPayload {
            tasks: tasks.clone(),
            recent_history: recent,
            load_error: self.load_error(),
        }
    }

    /// Look up a single task by id.
    pub fn get(&self, id: &str) -> Option<ScheduledTask> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.iter().find(|t| t.id == id).cloned()
    }

    /// Get history entries for a specific task, newest first.
    pub fn history_for_task(&self, task_id: &str) -> Vec<RunHistoryEntry> {
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        history
            .iter()
            .filter(|h| h.task_id == task_id)
            .cloned()
            .collect()
    }

    // -- mutations -------------------------------------------------------------

    /// Create or update a task.
    ///
    /// - When `input.id` is `Some` and the id exists the existing task is
    ///   updated.
    /// - When `input.id` is `Some` but the id does not exist a new task is
    ///   created with that id.
    /// - Otherwise a new task is created with a fresh UUIDv7 id.
    ///
    /// Title is trimmed; empty title is rejected.  `created_at_ms` is preserved
    /// on update; `updated_at_ms` is always set to now.
    pub fn upsert(&self, input: UpsertScheduledTaskInput) -> Result<ScheduledTask, String> {
        let title = input.title.trim().to_string();
        if title.is_empty() {
            return Err("Task title must not be empty.".to_string());
        }

        let now_ms = now_millis();
        let tags = normalize_tags(&input.tags);

        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(ref id) = input.id {
            if let Some(existing) = tasks.iter_mut().find(|t| t.id == *id) {
                let old_interval = existing.interval_seconds;
                existing.title = title;
                existing.text = input.text.trim().to_string();
                existing.tags = tags;
                existing.interval_seconds = input.interval_seconds;
                existing.profile_id = input
                    .profile_id
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                existing.updated_at_ms = now_ms;

                // Recompute next_run_at_ms if interval changed or task was
                // previously run and next_run was in the past.
                if existing.interval_seconds != old_interval
                    || existing.last_run_at_ms.is_some_and(|t| {
                        existing.next_run_at_ms <= t || existing.next_run_at_ms <= now_ms
                    })
                {
                    existing.next_run_at_ms = compute_next_run(
                        now_ms,
                        existing.interval_seconds,
                        existing.last_run_at_ms,
                    );
                }

                let task = existing.clone();
                drop(tasks);
                self.save()?;
                return Ok(task);
            }
        }

        // Create
        let id = input
            .id
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(new_task_id);

        let next_run_at_ms = compute_next_run(now_ms, input.interval_seconds, None);

        let task = ScheduledTask {
            id,
            title,
            text: input.text.trim().to_string(),
            enabled: true,
            interval_seconds: input.interval_seconds,
            next_run_at_ms,
            last_run_at_ms: None,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            tags,
            profile_id: input
                .profile_id
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            last_error: None,
        };
        tasks.push(task.clone());
        drop(tasks);
        self.save()?;
        Ok(task)
    }

    /// Delete a task by id.  Returns `true` if it existed and was removed.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let len_before = tasks.len();
        tasks.retain(|t| t.id != id);
        let removed = tasks.len() < len_before;
        drop(tasks);
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Set a task's enabled flag.  Returns `true` if the task was found.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<bool, String> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let mut found = false;
        let now_ms = now_millis();

        if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
            found = true;
            task.enabled = enabled;
            task.updated_at_ms = now_ms;

            // When re-enabled, re-compute next_run if it's in the past.
            if enabled
                && (task
                    .last_run_at_ms
                    .is_some_and(|t| task.next_run_at_ms <= t)
                    || task.next_run_at_ms <= now_ms)
            {
                task.next_run_at_ms =
                    compute_next_run(now_ms, task.interval_seconds, task.last_run_at_ms);
            }
        }
        drop(tasks);
        if found {
            self.save()?;
        }
        Ok(found)
    }

    /// Run a task now and record history.
    ///
    /// For MVP, this writes a deterministic `completed` history entry with a
    /// message explaining the task was recorded by the local scheduler.
    /// It does NOT create an agent session or invoke any gateway.
    ///
    /// Returns the task with updated `last_run_at_ms` and `next_run_at_ms`.
    /// Returns an error if the task is not found or is disabled.
    pub fn run_task_now(&self, id: &str) -> Result<ScheduledTask, String> {
        let started_ms = now_millis();

        // Read task state under lock.
        let (mut task, enabled) = {
            let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            let t = tasks
                .iter()
                .find(|t| t.id == id)
                .ok_or_else(|| format!("Task '{id}' not found."))?
                .clone();
            (t.clone(), t.enabled)
        };

        if !enabled {
            let ended_ms = now_millis();
            let entry = RunHistoryEntry {
                id: new_history_id(),
                task_id: id.to_string(),
                started_at_ms: started_ms,
                ended_at_ms: ended_ms,
                status: "skipped".to_string(),
                message: "[Forge Scheduler MVP] Task is disabled — skipped. Run-now via IPC was requested but the task is currently disabled.".to_string(),
            };
            self.push_history(entry);
            self.save()?;
            // Still update the in-memory task (not persisted for skipped).
            task.last_run_at_ms = Some(ended_ms);
            task.next_run_at_ms = compute_next_run(ended_ms, task.interval_seconds, Some(ended_ms));
            task.last_error = Some("Task is disabled — cannot run.".to_string());
            return Ok(task);
        }

        // MVP: deterministic "completed" history — no agent session creation.
        let ended_ms = now_millis();
        let entry = RunHistoryEntry {
            id: new_history_id(),
            task_id: id.to_string(),
            started_at_ms: started_ms,
            ended_at_ms: ended_ms,
            status: "completed".to_string(),
            message: format!(
                "[Forge Scheduler MVP] Task \"{}\" would execute prompt: \"{}\". Agent session execution is deferred to a future phase.",
                task.title,
                truncate_str(&task.text, 120)
            ),
        };
        self.push_history(entry);

        // Update task timing under lock and persist.
        {
            let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                t.last_run_at_ms = Some(ended_ms);
                t.next_run_at_ms = compute_next_run(ended_ms, t.interval_seconds, Some(ended_ms));
                t.last_error = None;
                t.updated_at_ms = ended_ms;
                task = t.clone();
            }
        }
        self.save()?;
        Ok(task)
    }

    /// Queue a task for execution through the gateway trigger store and record
    /// a run-history entry.
    pub fn run_task_now_with_trigger_store(
        &self,
        id: &str,
        trigger_store: &TriggerStore,
    ) -> Result<ScheduledTask, String> {
        self.run_task_now_with_trigger_store_at_workspace(id, trigger_store, None)
    }

    /// Queue a task for execution through the gateway trigger store with an
    /// optional workspace hint that the gateway runner can use for headless
    /// execution.
    pub fn run_task_now_with_trigger_store_at_workspace(
        &self,
        id: &str,
        trigger_store: &TriggerStore,
        workspace_path: Option<&Path>,
    ) -> Result<ScheduledTask, String> {
        let started_ms = now_millis();

        let (mut task, enabled) = {
            let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            let t = tasks
                .iter()
                .find(|t| t.id == id)
                .ok_or_else(|| format!("Task '{id}' not found."))?
                .clone();
            (t.clone(), t.enabled)
        };

        if !enabled {
            let ended_ms = now_millis();
            let entry = RunHistoryEntry {
                id: new_history_id(),
                task_id: id.to_string(),
                started_at_ms: started_ms,
                ended_at_ms: ended_ms,
                status: "skipped".to_string(),
                message: "Task is disabled — Gateway trigger was not queued.".to_string(),
            };
            self.push_history(entry);
            self.save()?;
            task.last_run_at_ms = Some(ended_ms);
            task.next_run_at_ms = compute_next_run(ended_ms, task.interval_seconds, Some(ended_ms));
            task.last_error = Some("Task is disabled — cannot run.".to_string());
            return Ok(task);
        }

        let ended_ms = now_millis();
        let trigger = PendingTrigger {
            id: new_history_id(),
            message: task.text.clone(),
            profile_id: task.profile_id.clone(),
            provider: None,
            model: None,
            workspace_path: workspace_path.map(|path| path.to_string_lossy().to_string()),
            attempt_count: 0,
            claimed_at_ms: None,
            received_at_ms: ended_ms,
        };
        trigger_store.push(trigger);

        let entry = RunHistoryEntry {
            id: new_history_id(),
            task_id: id.to_string(),
            started_at_ms: started_ms,
            ended_at_ms: ended_ms,
            status: "queued".to_string(),
            message: format!(
                "Queued Gateway trigger for task \"{}\": \"{}\".",
                task.title,
                truncate_str(&task.text, 120)
            ),
        };
        self.push_history(entry);

        {
            let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                t.last_run_at_ms = Some(ended_ms);
                t.next_run_at_ms = compute_next_run(ended_ms, t.interval_seconds, Some(ended_ms));
                t.last_error = None;
                t.updated_at_ms = ended_ms;
                task = t.clone();
            }
        }
        self.save()?;
        Ok(task)
    }

    // -- run due helper (tick) -------------------------------------------------

    /// Run all enabled tasks whose `next_run_at_ms` ≤ now.
    ///
    /// Returns the ids of tasks that were triggered.  For MVP, each due task
    /// records a deterministic history entry like `run_task_now` — no agent
    /// sessions are created.
    pub fn run_due_tasks(&self) -> Result<Vec<String>, String> {
        let now_ms = now_millis();
        let due_ids: Vec<String> = {
            let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            tasks
                .iter()
                .filter(|t| t.enabled && t.next_run_at_ms <= now_ms)
                .map(|t| t.id.clone())
                .collect()
        };

        for id in &due_ids {
            // Best-effort: if a task was deleted between listing and running,
            // just skip it.
            let _ = self.run_task_now(id);
        }

        Ok(due_ids)
    }

    /// Queue all enabled tasks whose `next_run_at_ms` ≤ now into the gateway
    /// trigger store.
    pub fn run_due_tasks_with_trigger_store(
        &self,
        trigger_store: &TriggerStore,
    ) -> Result<Vec<String>, String> {
        self.run_due_tasks_with_trigger_store_at_workspace(trigger_store, None)
    }

    /// Queue all due tasks into the gateway trigger store with an optional
    /// workspace hint for the downstream runner.
    pub fn run_due_tasks_with_trigger_store_at_workspace(
        &self,
        trigger_store: &TriggerStore,
        workspace_path: Option<&Path>,
    ) -> Result<Vec<String>, String> {
        let now_ms = now_millis();
        let due_ids: Vec<String> = {
            let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            tasks
                .iter()
                .filter(|t| t.enabled && t.next_run_at_ms <= now_ms)
                .map(|t| t.id.clone())
                .collect()
        };

        for id in &due_ids {
            let _ = self.run_task_now_with_trigger_store_at_workspace(
                id,
                trigger_store,
                workspace_path,
            );
        }

        Ok(due_ids)
    }

    // -- persistence -----------------------------------------------------------

    fn push_history(&self, entry: RunHistoryEntry) {
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        history.insert(0, entry);
        // Prune old entries.
        if history.len() > MAX_HISTORY {
            history.truncate(MAX_HISTORY);
        }
    }

    fn save(&self) -> Result<(), String> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());

        let file = SchedulerFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            tasks: tasks.clone(),
            history: history.clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| format!("serialize: {e}"))?;

        // Atomic-ish: write to temp then rename.
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, json.as_bytes()).map_err(|e| format!("write temp: {e}"))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("rename: {e}"))?;

        // Clear any stale load error on successful save.
        if let Ok(mut err) = self.load_error.lock() {
            *err = None;
        }

        Ok(())
    }
}

pub fn run_scheduler_tick_once(
    store: &SchedulerStore,
    trigger_store: &TriggerStore,
    workspace_path: Option<&Path>,
) -> Result<Vec<String>, String> {
    store.run_due_tasks_with_trigger_store_at_workspace(trigger_store, workspace_path)
}

pub fn spawn_scheduler_tick(
    store: Arc<SchedulerStore>,
    trigger_store: Arc<TriggerStore>,
    workspace_path: PathBuf,
) {
    tokio::spawn(async move {
        loop {
            match run_scheduler_tick_once(&store, &trigger_store, Some(workspace_path.as_path())) {
                Ok(triggered) if !triggered.is_empty() => {
                    log::info!(
                        "scheduler tick queued {} gateway trigger(s)",
                        triggered.len()
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    log::warn!("scheduler tick failed: {error}");
                }
            }

            tokio::time::sleep(Duration::from_secs(SCHEDULER_TICK_INTERVAL_SECS)).await;
        }
    });
}

// ── File I/O ─────────────────────────────────────────────────────────────────

fn load_scheduler(path: &PathBuf) -> (Vec<ScheduledTask>, Vec<RunHistoryEntry>, Option<String>) {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return (Vec::new(), Vec::new(), None);
        }
        Err(e) => return (Vec::new(), Vec::new(), Some(format!("read error: {e}"))),
    };

    let file: SchedulerFile = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            return (Vec::new(), Vec::new(), Some(format!("corrupt JSON: {e}")));
        }
    };

    (file.tasks, file.history, None)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn new_task_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn new_history_id() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn normalize_tags(raw: &[String]) -> Vec<String> {
    let mut tags: Vec<String> = raw
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

/// Compute the next run timestamp in ms.
///
/// - If `interval_seconds` is 0 (manual-only), next_run is set to a JS-safe
///   far-future timestamp so it never appears as "due".
/// - Otherwise, returns `max(now_ms, last_run + interval)`.
fn compute_next_run(now_ms: u64, interval_seconds: u64, last_run_at_ms: Option<u64>) -> u64 {
    if interval_seconds == 0 {
        return MANUAL_NEXT_RUN_AT_MS;
    }
    let interval_ms = interval_seconds as u128 * 1_000;
    let next = match last_run_at_ms {
        Some(last) => std::cmp::max(now_ms, last.saturating_add(interval_ms as u64)),
        None => now_ms.saturating_add(interval_ms as u64),
    };
    next.min(MANUAL_NEXT_RUN_AT_MS)
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}…")
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("forge-scheduler-{name}-{nanos}.json"))
    }

    fn sample_input(title: &str) -> UpsertScheduledTaskInput {
        UpsertScheduledTaskInput {
            id: None,
            title: title.to_string(),
            text: "echo hello".to_string(),
            tags: vec![],
            interval_seconds: 3600,
            profile_id: None,
        }
    }

    fn assert_cleanup(path: &PathBuf) {
        let _ = fs::remove_file(path);
        let tmp = path.with_extension("tmp");
        let _ = fs::remove_file(&tmp);
    }

    // ── Empty store ──────────────────────────────────────────────────────

    #[test]
    fn empty_store_lists_nothing() {
        let path = temp_path("empty");
        let store = SchedulerStore::new(path.clone());
        let payload = store.list_payload();
        assert!(payload.tasks.is_empty());
        assert!(payload.recent_history.is_empty());
        assert_eq!(payload.load_error, None);
        assert_cleanup(&path);
    }

    #[test]
    fn empty_store_has_no_load_error() {
        let path = temp_path("no-error");
        let store = SchedulerStore::new(path.clone());
        assert_eq!(store.load_error(), None);
        assert_cleanup(&path);
    }

    // ── Create / list ────────────────────────────────────────────────────

    #[test]
    fn create_and_list_task() {
        let path = temp_path("create-list");
        let store = SchedulerStore::new(path.clone());

        let task = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "Daily sync".to_string(),
                text: "sync data".to_string(),
                tags: vec!["daily".into(), "sync".into()],
                interval_seconds: 86400,
                profile_id: Some("default".into()),
            })
            .expect("upsert");

        assert_eq!(task.title, "Daily sync");
        assert_eq!(task.text, "sync data");
        assert_eq!(task.tags, vec!["daily", "sync"]);
        assert_eq!(task.interval_seconds, 86400);
        assert_eq!(task.profile_id.as_deref(), Some("default"));
        assert!(task.enabled);
        assert!(task.created_at_ms > 0);
        assert_eq!(task.created_at_ms, task.updated_at_ms);
        // next_run should be in the future for interval > 0
        assert!(task.next_run_at_ms > task.created_at_ms);
        assert_eq!(task.last_run_at_ms, None);

        let payload = store.list_payload();
        assert_eq!(payload.tasks.len(), 1);
        assert_eq!(payload.tasks[0].id, task.id);

        assert_cleanup(&path);
    }

    #[test]
    fn create_rejects_empty_title() {
        let path = temp_path("empty-title");
        let store = SchedulerStore::new(path.clone());
        let err = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "  ".to_string(),
                text: "x".into(),
                tags: vec![],
                interval_seconds: 60,
                profile_id: None,
            })
            .expect_err("empty");
        assert!(err.contains("not be empty"));
        assert_cleanup(&path);
    }

    #[test]
    fn create_trims_title() {
        let path = temp_path("trim-title");
        let store = SchedulerStore::new(path.clone());
        let task = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "  Trimmed  ".to_string(),
                text: "x".into(),
                tags: vec![],
                interval_seconds: 60,
                profile_id: None,
            })
            .expect("upsert");
        assert_eq!(task.title, "Trimmed");
        assert_cleanup(&path);
    }

    // ── Tags ─────────────────────────────────────────────────────────────

    #[test]
    fn tags_are_trimmed_and_deduped() {
        let path = temp_path("tags");
        let store = SchedulerStore::new(path.clone());
        let task = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "Tag test".into(),
                text: "x".into(),
                tags: vec!["  CI  ".into(), "ci".into(), "".into(), "daily".into()],
                interval_seconds: 60,
                profile_id: None,
            })
            .expect("upsert");
        assert_eq!(task.tags, vec!["CI", "ci", "daily"]);
        assert!(task.tags.iter().all(|t| !t.is_empty()));
        assert_cleanup(&path);
    }

    // ── Update ───────────────────────────────────────────────────────────

    #[test]
    fn update_preserves_created_at_and_changes_updated_at() {
        let path = temp_path("update");
        let store = SchedulerStore::new(path.clone());
        let t1 = store.upsert(sample_input("v1")).expect("upsert");
        let created = t1.created_at_ms;
        assert_eq!(t1.created_at_ms, t1.updated_at_ms);

        std::thread::sleep(std::time::Duration::from_millis(2));

        let t2 = store
            .upsert(UpsertScheduledTaskInput {
                id: Some(t1.id.clone()),
                title: "v2".to_string(),
                text: "updated".into(),
                tags: vec!["new".into()],
                interval_seconds: 7200,
                profile_id: None,
            })
            .expect("upsert");

        assert_eq!(t2.title, "v2");
        assert_eq!(t2.text, "updated");
        assert_eq!(t2.created_at_ms, created);
        assert!(
            t2.updated_at_ms > created,
            "updated_at_ms should be > created_at_ms"
        );
        assert_cleanup(&path);
    }

    #[test]
    fn update_with_unknown_id_creates_new() {
        let path = temp_path("update-unknown");
        let store = SchedulerStore::new(path.clone());
        let task = store
            .upsert(UpsertScheduledTaskInput {
                id: Some("my-custom-id".into()),
                title: "Custom".to_string(),
                text: "x".into(),
                tags: vec![],
                interval_seconds: 60,
                profile_id: None,
            })
            .expect("upsert");
        assert_eq!(task.id, "my-custom-id");
        assert_eq!(task.title, "Custom");
        assert_cleanup(&path);
    }

    // ── Delete ───────────────────────────────────────────────────────────

    #[test]
    fn delete_removes_task() {
        let path = temp_path("delete");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("to-delete")).expect("upsert");

        let removed = store.delete(&task.id).expect("delete");
        assert!(removed);

        let payload = store.list_payload();
        assert!(payload.tasks.is_empty());
        assert_cleanup(&path);
    }

    #[test]
    fn delete_unknown_id_returns_false() {
        let path = temp_path("delete-unknown");
        let store = SchedulerStore::new(path.clone());
        let removed = store.delete("nonexistent").expect("delete");
        assert!(!removed);
        assert_cleanup(&path);
    }

    // ── Enabled / disabled ───────────────────────────────────────────────

    #[test]
    fn set_enabled_toggles_flag() {
        let path = temp_path("enabled");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("toggle")).expect("upsert");
        assert!(task.enabled);

        let found = store.set_enabled(&task.id, false).expect("set_enabled");
        assert!(found);

        let updated = store.get(&task.id).expect("get");
        assert!(!updated.enabled);

        let found2 = store.set_enabled(&task.id, true).expect("re-enable");
        assert!(found2);

        let re_enabled = store.get(&task.id).expect("get");
        assert!(re_enabled.enabled);
        assert_cleanup(&path);
    }

    #[test]
    fn set_enabled_unknown_id_returns_false() {
        let path = temp_path("enabled-unknown");
        let store = SchedulerStore::new(path.clone());
        let found = store
            .set_enabled("nonexistent", false)
            .expect("set_enabled");
        assert!(!found);
        assert_cleanup(&path);
    }

    // ── Next run computation ─────────────────────────────────────────────

    #[test]
    fn next_run_manual_only_is_far_future() {
        assert_eq!(compute_next_run(1000, 0, None), MANUAL_NEXT_RUN_AT_MS);
    }

    #[test]
    fn next_run_no_last_run_is_now_plus_interval() {
        let next = compute_next_run(1000, 60, None);
        assert_eq!(next, 1000 + 60_000);
    }

    #[test]
    fn next_run_after_last_run_is_last_plus_interval() {
        let next = compute_next_run(200_000, 60, Some(100_000));
        // last_run_at_ms + interval_ms = 100_000 + 60_000 = 160_000
        // max(now_ms=200_000, 160_000) = 200_000
        assert_eq!(next, 200_000);
    }

    #[test]
    fn next_run_saturating_add_does_not_overflow() {
        let next = compute_next_run(u64::MAX - 1000, 3600, Some(u64::MAX - 1000));
        assert_eq!(next, MANUAL_NEXT_RUN_AT_MS);
    }

    // ── Run now — history ────────────────────────────────────────────────

    #[test]
    fn run_task_now_records_history_entry() {
        let path = temp_path("run-now");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("run-me")).expect("upsert");

        let result = store.run_task_now(&task.id).expect("run");
        assert_eq!(result.title, "run-me");
        assert!(result.last_run_at_ms.is_some());
        assert_eq!(result.last_error, None);

        let payload = store.list_payload();
        assert!(
            !payload.recent_history.is_empty(),
            "should have at least one history entry"
        );
        let entry = &payload.recent_history[0];
        assert_eq!(entry.task_id, task.id);
        assert_eq!(entry.status, "completed");
        assert!(entry.message.contains("Scheduler MVP"));
        assert_cleanup(&path);
    }

    #[test]
    fn run_task_now_with_trigger_store_queues_gateway_trigger() {
        let path = temp_path("run-now-trigger");
        let store = SchedulerStore::new(path.clone());
        let task = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "Morning review".into(),
                text: "summarize overnight changes".into(),
                tags: vec!["ops".into()],
                interval_seconds: 3600,
                profile_id: Some("ops".into()),
            })
            .expect("upsert");
        let triggers = crate::gateway::webhook::TriggerStore::new();

        let result = store
            .run_task_now_with_trigger_store(&task.id, &triggers)
            .expect("run");

        assert_eq!(result.last_error, None);
        let queued = triggers.list();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].message, "summarize overnight changes");
        assert_eq!(queued[0].profile_id.as_deref(), Some("ops"));

        let payload = store.list_payload();
        let entry = &payload.recent_history[0];
        assert_eq!(entry.task_id, task.id);
        assert_eq!(entry.status, "queued");
        assert!(entry.message.contains("Gateway"));
        assert_cleanup(&path);
    }

    #[test]
    fn run_task_now_on_disabled_task_skips() {
        let path = temp_path("run-disabled");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("disabled-task")).expect("upsert");
        store.set_enabled(&task.id, false).expect("disable");

        let result = store.run_task_now(&task.id).expect("run");
        assert_eq!(
            result.last_error.as_deref(),
            Some("Task is disabled — cannot run.")
        );

        let payload = store.list_payload();
        let entry = payload
            .recent_history
            .iter()
            .find(|h| h.task_id == task.id)
            .expect("history entry for disabled task");
        assert_eq!(entry.status, "skipped");
        assert_cleanup(&path);
    }

    #[test]
    fn run_task_now_unknown_id_errors() {
        let path = temp_path("run-unknown");
        let store = SchedulerStore::new(path.clone());
        let err = store.run_task_now("nonexistent").expect_err("unknown");
        assert!(err.contains("not found"));
        assert_cleanup(&path);
    }

    // ── Run due tasks ────────────────────────────────────────────────────

    #[test]
    fn run_due_tasks_triggers_due_items() {
        let path = temp_path("run-due");
        let store = SchedulerStore::new(path.clone());

        // Create a task with interval_seconds=0 so next_run is far-future and
        // one with an interval that makes it due immediately.
        // We'll manipulate the stored task directly to set next_run in the past.
        let future = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "Future".to_string(),
                text: "x".into(),
                tags: vec![],
                interval_seconds: 0, // manual only → far future
                profile_id: None,
            })
            .expect("future");

        let due = store
            .upsert(UpsertScheduledTaskInput {
                id: None,
                title: "Due".to_string(),
                text: "x".into(),
                tags: vec![],
                interval_seconds: 3600,
                profile_id: None,
            })
            .expect("due");

        // Set next_run_at_ms to 0 (epoch) so it's definitely due.
        {
            let mut tasks = store.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = tasks.iter_mut().find(|t| t.id == due.id) {
                t.next_run_at_ms = 0;
            }
        }

        let triggered = store.run_due_tasks().expect("run_due");
        assert!(triggered.contains(&due.id));
        assert!(!triggered.contains(&future.id));

        // History should have an entry for the due task.
        let payload = store.list_payload();
        let due_entries: Vec<_> = payload
            .recent_history
            .iter()
            .filter(|h| h.task_id == due.id)
            .collect();
        assert_eq!(due_entries.len(), 1);
        assert_eq!(due_entries[0].status, "completed");

        // No history for the future task.
        assert!(!payload
            .recent_history
            .iter()
            .any(|h| h.task_id == future.id));

        assert_cleanup(&path);
    }

    #[test]
    fn run_due_tasks_respects_disabled() {
        let path = temp_path("run-due-disabled");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("disabled-due")).expect("upsert");
        store.set_enabled(&task.id, false).expect("disable");

        // Set next_run to 0 so it would be due if enabled.
        {
            let mut tasks = store.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = tasks.iter_mut().find(|t| t.id == task.id) {
                t.next_run_at_ms = 0;
            }
        }

        let triggered = store.run_due_tasks().expect("run_due");
        // Disabled tasks should not be in the triggered list.
        assert!(!triggered.contains(&task.id));
        assert_cleanup(&path);
    }

    #[test]
    fn scheduler_tick_once_queues_due_tasks_through_gateway_trigger_store() {
        let path = temp_path("tick-once");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("tick-due")).expect("upsert");
        {
            let mut tasks = store.tasks.lock().unwrap_or_else(|e| e.into_inner());
            let t = tasks.iter_mut().find(|t| t.id == task.id).unwrap();
            t.next_run_at_ms = 0;
        }
        let trigger_store = crate::gateway::webhook::TriggerStore::new();
        let workspace = tempfile::tempdir().expect("workspace");

        let triggered =
            run_scheduler_tick_once(&store, &trigger_store, Some(workspace.path())).expect("tick");

        assert_eq!(triggered, vec![task.id]);
        let queued = trigger_store.list();
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued[0].workspace_path.as_deref(),
            Some(workspace.path().to_string_lossy().as_ref())
        );
        assert_cleanup(&path);
    }

    // ── Persistence ──────────────────────────────────────────────────────

    #[test]
    fn tasks_persist_across_store_reload() {
        let path = temp_path("persist");
        let store1 = SchedulerStore::new(path.clone());
        let task = store1.upsert(sample_input("Persisted")).expect("upsert");

        // Also run it to get history.
        store1.run_task_now(&task.id).expect("run");

        let store2 = SchedulerStore::new(path.clone());
        let payload = store2.list_payload();
        assert_eq!(payload.tasks.len(), 1);
        assert_eq!(payload.tasks[0].title, "Persisted");
        assert!(!payload.recent_history.is_empty());

        assert_cleanup(&path);
    }

    // ── Corrupt JSON ─────────────────────────────────────────────────────

    #[test]
    fn corrupt_json_loads_empty_and_reports_error() {
        let path = temp_path("corrupt");
        fs::write(&path, "not valid json {{{").expect("write corrupt");

        let store = SchedulerStore::new(path.clone());
        let payload = store.list_payload();
        assert!(payload.tasks.is_empty());
        assert!(payload.recent_history.is_empty());
        let err = store.load_error();
        assert!(err.is_some(), "should report load error");
        assert!(err.unwrap().contains("corrupt"));

        assert_cleanup(&path);
    }

    // ── Atomic save ──────────────────────────────────────────────────────

    #[test]
    fn save_does_not_leave_temp_file() {
        let path = temp_path("atomic");
        let store = SchedulerStore::new(path.clone());
        store.upsert(sample_input("Atomic")).expect("upsert");

        let tmp = path.with_extension("tmp");
        assert!(!tmp.exists(), "temp file should be gone after rename");

        assert_cleanup(&path);
    }

    #[test]
    fn saved_file_is_valid_json() {
        let path = temp_path("valid-json");
        let store = SchedulerStore::new(path.clone());
        store.upsert(sample_input("Json")).expect("upsert");

        let raw = fs::read_to_string(&path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(parsed["schema_version"].as_u64(), Some(1));
        assert_eq!(parsed["tasks"].as_array().unwrap().len(), 1);

        assert_cleanup(&path);
    }

    // ── History pruning ──────────────────────────────────────────────────

    #[test]
    fn history_is_trimmed_to_max_entries() {
        let path = temp_path("history-prune");
        let store = SchedulerStore::new(path.clone());
        let task = store.upsert(sample_input("prune-test")).expect("upsert");

        // Add many history entries — push more than MAX_HISTORY.
        for i in 0..600u64 {
            let entry = RunHistoryEntry {
                id: format!("hist-{i}"),
                task_id: task.id.clone(),
                started_at_ms: 1000 + i,
                ended_at_ms: 1100 + i,
                status: "completed".to_string(),
                message: format!("Entry {i}"),
            };
            store.push_history(entry);
        }

        let history = store.history.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            history.len(),
            MAX_HISTORY,
            "should be capped at MAX_HISTORY"
        );
        // Newest should be first (inserted at index 0).
        assert_eq!(history[0].message, "Entry 599");
        assert_eq!(history[MAX_HISTORY - 1].message, "Entry 100"); // 600 - 500 = 100 oldest kept
        assert_cleanup(&path);
    }
}
