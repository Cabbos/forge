#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GoalLedger {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current: Option<GoalState>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GoalState {
    pub id: String,
    pub objective: String,
    pub status: GoalStatus,
    pub tasks: Vec<GoalTask>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GoalTask {
    pub id: String,
    pub title: String,
    pub status: GoalTaskStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalStatus {
    Active,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalTaskStatus {
    Pending,
    InProgress,
    Completed,
    Skipped,
}

impl GoalLedger {
    pub(crate) fn new_active(
        id: impl Into<String>,
        objective: impl Into<String>,
        task_titles: Vec<String>,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            current: Some(GoalState {
                id: id.into(),
                objective: objective.into(),
                status: GoalStatus::Active,
                tasks: task_titles
                    .into_iter()
                    .enumerate()
                    .map(|(index, title)| GoalTask {
                        id: format!("task-{}", index + 1),
                        title,
                        status: GoalTaskStatus::Pending,
                        created_at_ms: timestamp_ms,
                        updated_at_ms: timestamp_ms,
                        resume_note: None,
                    })
                    .collect(),
                blocked_reason: None,
                created_at_ms: timestamp_ms,
                updated_at_ms: timestamp_ms,
                closed_at_ms: None,
            }),
        }
    }

    pub(crate) fn current_goal(&self) -> Option<&GoalState> {
        self.current.as_ref()
    }

    pub(crate) fn active_goal(&self) -> Option<&GoalState> {
        self.current
            .as_ref()
            .filter(|goal| goal.status == GoalStatus::Active)
    }

    pub(crate) fn update_task_status(
        &mut self,
        task_id: &str,
        status: GoalTaskStatus,
        timestamp_ms: u64,
    ) -> bool {
        let Some(goal) = self.current.as_mut() else {
            return false;
        };
        if goal.status != GoalStatus::Active {
            return false;
        }
        let Some(task) = goal.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };

        task.status = status;
        task.resume_note = None;
        task.updated_at_ms = timestamp_ms;
        goal.updated_at_ms = timestamp_ms;
        true
    }

    pub(crate) fn complete_active(&mut self, timestamp_ms: u64) -> bool {
        let Some(goal) = self.current.as_mut() else {
            return false;
        };
        if goal.status != GoalStatus::Active {
            return false;
        }

        goal.status = GoalStatus::Completed;
        goal.blocked_reason = None;
        goal.closed_at_ms = Some(timestamp_ms);
        goal.updated_at_ms = timestamp_ms;
        for task in &mut goal.tasks {
            if matches!(
                task.status,
                GoalTaskStatus::Pending | GoalTaskStatus::InProgress
            ) {
                task.status = GoalTaskStatus::Completed;
                task.updated_at_ms = timestamp_ms;
                task.resume_note = None;
            }
        }
        true
    }

    pub(crate) fn block_active(&mut self, reason: impl Into<String>, timestamp_ms: u64) -> bool {
        let Some(goal) = self.current.as_mut() else {
            return false;
        };
        if goal.status != GoalStatus::Active {
            return false;
        }

        let reason = reason.into();
        goal.status = GoalStatus::Blocked;
        goal.blocked_reason = (!reason.trim().is_empty()).then_some(reason);
        goal.closed_at_ms = Some(timestamp_ms);
        goal.updated_at_ms = timestamp_ms;
        true
    }

    /// Returns true if the active goal has at least one task in Pending or
    /// InProgress status — meaning there is still work to do.
    pub(crate) fn has_pending_tasks(&self) -> bool {
        let Some(goal) = self.current.as_ref() else {
            return false;
        };
        if goal.status != GoalStatus::Active {
            return false;
        }
        goal.tasks.iter().any(|t| {
            matches!(
                t.status,
                GoalTaskStatus::Pending | GoalTaskStatus::InProgress
            )
        })
    }

    pub(crate) fn normalize_for_resume(&mut self, timestamp_ms: u64) {
        let Some(goal) = self.current.as_mut() else {
            return;
        };
        if goal.status != GoalStatus::Active {
            return;
        }

        let mut changed = false;
        for task in &mut goal.tasks {
            if task.status == GoalTaskStatus::InProgress {
                task.status = GoalTaskStatus::Pending;
                task.resume_note =
                    Some("task was in progress when the session was restored".to_string());
                task.updated_at_ms = timestamp_ms;
                changed = true;
            }
        }
        if changed {
            goal.updated_at_ms = timestamp_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_creates_active_goal_with_pending_tasks() {
        let ledger = GoalLedger::new_active(
            "goal-1",
            "Ship goal ledger foundation",
            vec!["Model state".to_string(), "Persist later".to_string()],
            10,
        );

        let active = ledger.active_goal().expect("active goal");

        assert_eq!(active.id, "goal-1");
        assert_eq!(active.objective, "Ship goal ledger foundation");
        assert_eq!(active.status, GoalStatus::Active);
        assert_eq!(active.tasks.len(), 2);
        assert_eq!(active.tasks[0].status, GoalTaskStatus::Pending);
        assert_eq!(active.created_at_ms, 10);
        assert_eq!(active.updated_at_ms, 10);
        assert_eq!(active.closed_at_ms, None);
    }

    #[test]
    fn ledger_updates_task_status_and_tracks_goal_timestamp() {
        let mut ledger = GoalLedger::new_active(
            "goal-1",
            "Ship goal ledger foundation",
            vec!["Model state".to_string()],
            10,
        );

        assert!(ledger.update_task_status("task-1", GoalTaskStatus::InProgress, 20));

        let active = ledger.active_goal().expect("active goal");
        assert_eq!(active.tasks[0].status, GoalTaskStatus::InProgress);
        assert_eq!(active.tasks[0].updated_at_ms, 20);
        assert_eq!(active.updated_at_ms, 20);
    }

    #[test]
    fn ledger_closes_active_goal_as_complete_or_blocked() {
        let mut completed = GoalLedger::new_active(
            "goal-1",
            "Ship goal ledger foundation",
            vec!["Model state".to_string()],
            10,
        );
        completed.complete_active(30);

        let goal = completed.current_goal().expect("completed goal retained");
        assert_eq!(goal.status, GoalStatus::Completed);
        assert_eq!(goal.closed_at_ms, Some(30));
        assert!(completed.active_goal().is_none());
        assert_eq!(goal.tasks[0].status, GoalTaskStatus::Completed);

        let mut blocked = GoalLedger::new_active(
            "goal-2",
            "Persist goal ledger",
            vec!["Wire snapshot".to_string()],
            40,
        );
        blocked.block_active("needs snapshot integration", 50);

        let goal = blocked.current_goal().expect("blocked goal retained");
        assert_eq!(goal.status, GoalStatus::Blocked);
        assert_eq!(
            goal.blocked_reason.as_deref(),
            Some("needs snapshot integration")
        );
        assert_eq!(goal.closed_at_ms, Some(50));
        assert!(blocked.active_goal().is_none());
    }

    #[test]
    fn ledger_serializes_roundtrip_with_snake_case_statuses() {
        let mut ledger = GoalLedger::new_active(
            "goal-1",
            "Ship goal ledger foundation",
            vec!["Model state".to_string()],
            10,
        );

        ledger.update_task_status("task-1", GoalTaskStatus::InProgress, 20);

        let json = serde_json::to_string(&ledger).expect("serialize ledger");
        assert!(json.contains(r#""status":"active""#));
        assert!(json.contains(r#""status":"in_progress""#));

        let restored: GoalLedger = serde_json::from_str(&json).expect("deserialize ledger");

        assert_eq!(restored.active_goal().expect("active").id, "goal-1");
        assert_eq!(
            restored.active_goal().expect("active").tasks[0].status,
            GoalTaskStatus::InProgress
        );
    }

    #[test]
    fn ledger_resume_normalizes_in_progress_task_back_to_pending() {
        let mut ledger = GoalLedger::new_active(
            "goal-1",
            "Ship goal ledger foundation",
            vec!["Model state".to_string()],
            10,
        );
        ledger.update_task_status("task-1", GoalTaskStatus::InProgress, 20);

        ledger.normalize_for_resume(30);

        let active = ledger.active_goal().expect("active goal");
        assert_eq!(active.status, GoalStatus::Active);
        assert_eq!(active.tasks[0].status, GoalTaskStatus::Pending);
        assert_eq!(
            active.tasks[0].resume_note.as_deref(),
            Some("task was in progress when the session was restored")
        );
        assert_eq!(active.updated_at_ms, 30);
    }

    #[test]
    fn has_pending_tasks_returns_true_when_tasks_are_pending() {
        let ledger = GoalLedger::new_active("g", "o", vec!["t1".to_string(), "t2".to_string()], 1);
        assert!(ledger.has_pending_tasks());
    }

    #[test]
    fn has_pending_tasks_returns_true_when_task_is_in_progress() {
        let mut ledger = GoalLedger::new_active("g", "o", vec!["t1".to_string()], 1);
        ledger.update_task_status("task-1", GoalTaskStatus::InProgress, 2);
        assert!(ledger.has_pending_tasks());
    }

    #[test]
    fn has_pending_tasks_returns_false_when_all_tasks_completed() {
        let mut ledger = GoalLedger::new_active("g", "o", vec!["t1".to_string()], 1);
        ledger.update_task_status("task-1", GoalTaskStatus::Completed, 2);
        assert!(!ledger.has_pending_tasks());
    }

    #[test]
    fn has_pending_tasks_returns_false_when_no_active_goal() {
        let ledger = GoalLedger { current: None };
        assert!(!ledger.has_pending_tasks());
    }

    #[test]
    fn has_pending_tasks_returns_false_when_goal_is_blocked() {
        let mut ledger = GoalLedger::new_active("g", "o", vec!["t1".to_string()], 1);
        ledger.complete_active(2);
        ledger.block_active("blocked", 3);
        assert!(!ledger.has_pending_tasks());
    }
}
