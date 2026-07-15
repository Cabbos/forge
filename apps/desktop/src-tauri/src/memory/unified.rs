use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::continuity::{ExperienceKind, ExperienceMemory, ExperienceStatus};
use crate::memory::facts::MemoryFact;
use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemorySource {
    WikiMemory,
    MemoryFact,
    ContinuityExperience,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryKind {
    Preference,
    ProjectFact,
    Decision,
    TaskState,
    Lesson,
    BugPattern,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryScope {
    Session,
    UserProfile,
    Project,
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryStatus {
    Candidate,
    Accepted,
    Pinned,
    Forgotten,
    Archived,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedMemoryVisibility {
    #[default]
    UserVisible,
    HiddenContext,
    AuditOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnifiedMemoryProvenance {
    pub owner: String,
    pub storage: String,
    pub source_label: String,
}

impl Default for UnifiedMemoryProvenance {
    fn default() -> Self {
        Self {
            owner: "desktop_runtime".to_string(),
            storage: "unknown".to_string(),
            source_label: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryAuthorityDescriptor {
    pub source: String,
    pub owner: String,
    pub storage: String,
    pub scope: String,
    pub archive_policy: String,
    pub forget_policy: String,
    pub edit_policy: String,
    pub recall_policy: String,
    pub can_archive: bool,
    pub can_forget: bool,
    pub recall_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMemoryRecord {
    pub id: String,
    pub source: UnifiedMemorySource,
    pub source_id: String,
    pub kind: UnifiedMemoryKind,
    pub status: UnifiedMemoryStatus,
    pub scope: UnifiedMemoryScope,
    pub title: String,
    pub body: String,
    pub project_path: Option<String>,
    pub profile_id: Option<String>,
    pub source_session_id: Option<String>,
    pub confidence: f32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub tags: Vec<String>,
    #[serde(default)]
    pub visibility: UnifiedMemoryVisibility,
    #[serde(default)]
    pub provenance: UnifiedMemoryProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at_ms: Option<u64>,
    #[serde(default)]
    pub forget_policy: String,
    #[serde(default)]
    pub recall_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMemorySelection {
    pub record: UnifiedMemoryRecord,
    pub score: f32,
    pub reason: String,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecallDecision {
    Injected,
    Duplicate,
    ExcludedStatus,
    ExcludedProject,
    ExcludedProfile,
    NoRelevanceSignal,
    LowSignalQuery,
    BudgetExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecallCandidateAudit {
    pub memory_id: String,
    pub source: String,
    pub source_id: String,
    pub kind: String,
    pub status: UnifiedMemoryStatus,
    pub score: f32,
    pub reason: String,
    pub decision: RecallDecision,
    pub project_match: bool,
    pub profile_match: bool,
    pub estimated_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecallBudget {
    pub candidate_count: usize,
    pub injection_limit: usize,
    pub budget_tokens: u32,
    pub estimated_injected_tokens: u32,
    pub injected_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecallPlan {
    pub selected_memory_ids: Vec<String>,
    pub candidates: Vec<RecallCandidateAudit>,
    pub budget: RecallBudget,
    #[serde(skip)]
    selected: Vec<UnifiedMemorySelection>,
}

impl RecallPlan {
    pub(crate) fn into_selected(self) -> Vec<UnifiedMemorySelection> {
        self.selected
    }
}

impl UnifiedMemoryRecord {
    pub fn from_wiki_memory(memory: WikiMemory) -> Self {
        let status = map_memory_status(memory.status);
        let updated_at_ms = memory.updated_at.parse().unwrap_or(0);
        Self {
            id: format!("wiki_memory:{}", memory.id),
            source: UnifiedMemorySource::WikiMemory,
            source_id: memory.id,
            kind: map_memory_category(memory.category),
            status: status.clone(),
            scope: map_memory_scope(memory.scope),
            title: memory.title,
            body: memory.body,
            project_path: memory.project_path,
            profile_id: None,
            source_session_id: memory.source_session_id,
            confidence: memory.confidence,
            created_at_ms: memory.created_at.parse().unwrap_or(0),
            updated_at_ms,
            tags: memory.tags,
            visibility: UnifiedMemoryVisibility::UserVisible,
            provenance: provenance("wiki_memory_store", "wiki_memory"),
            last_used_at_ms: memory.last_used_at.and_then(|value| value.parse().ok()),
            archived_at_ms: archived_at_ms(status.clone(), updated_at_ms),
            forget_policy: "forget_supported".to_string(),
            recall_policy: recall_policy_for_status(status),
        }
    }

    pub fn from_memory_fact(fact: MemoryFact) -> Self {
        let title = fact
            .tags
            .first()
            .map(|tag| format!("记忆事实: {tag}"))
            .unwrap_or_else(|| "记忆事实".to_string());
        let recall_policy = if fact.profile_id.is_some() {
            "eligible_when_profile_matches"
        } else {
            "eligible_when_project_matches"
        }
        .to_string();
        Self {
            id: format!("memory_fact:{}", fact.id),
            source: UnifiedMemorySource::MemoryFact,
            source_id: fact.id,
            kind: UnifiedMemoryKind::ProjectFact,
            status: UnifiedMemoryStatus::Accepted,
            scope: if fact.profile_id.is_some() {
                UnifiedMemoryScope::UserProfile
            } else {
                UnifiedMemoryScope::Project
            },
            title,
            body: fact.text,
            project_path: None,
            profile_id: fact.profile_id,
            source_session_id: None,
            confidence: 1.0,
            created_at_ms: fact.created_at_ms,
            updated_at_ms: fact.updated_at_ms,
            tags: fact.tags,
            visibility: UnifiedMemoryVisibility::UserVisible,
            provenance: provenance("profile_memory_facts", "memory_fact"),
            last_used_at_ms: None,
            archived_at_ms: None,
            forget_policy: "delete_supported".to_string(),
            recall_policy,
        }
    }

    pub fn from_continuity_experience(experience: ExperienceMemory) -> Self {
        let status = map_experience_status(experience.status);
        Self {
            id: format!("continuity_experience:{}", experience.id),
            source: UnifiedMemorySource::ContinuityExperience,
            source_id: experience.id,
            kind: map_experience_kind(experience.kind),
            status: status.clone(),
            scope: UnifiedMemoryScope::Project,
            title: experience.title,
            body: experience.body,
            project_path: experience.project_path,
            profile_id: None,
            source_session_id: experience.source_session_id,
            confidence: experience.confidence,
            created_at_ms: experience.created_at_ms,
            updated_at_ms: experience.updated_at_ms,
            tags: experience.tags,
            visibility: UnifiedMemoryVisibility::UserVisible,
            provenance: provenance("continuity_store", "continuity_experience"),
            last_used_at_ms: None,
            archived_at_ms: archived_at_ms(status.clone(), experience.updated_at_ms),
            forget_policy: "forget_supported".to_string(),
            recall_policy: recall_policy_for_status(status),
        }
    }
}

pub fn memory_authority_map_v2() -> Vec<MemoryAuthorityDescriptor> {
    vec![
        authority(
            "wiki_memory",
            "wiki_memory_store",
            "session/user_profile/project/document",
            "archive_supported",
            "forget_supported",
            "wiki_memory_editor",
            "eligible_when_current",
            true,
            true,
            true,
        ),
        authority(
            "memory_fact",
            "profile_memory_facts",
            "user_profile/project",
            "archive_not_supported",
            "delete_supported",
            "profile_fact_detail_editor",
            "eligible_when_profile_or_project_matches",
            false,
            true,
            true,
        ),
        authority(
            "continuity_experience",
            "continuity_store",
            "project",
            "archive_supported",
            "forget_supported",
            "continuity_status_only",
            "eligible_when_current",
            true,
            true,
            true,
        ),
        authority(
            "saved_background",
            "project_records",
            "project",
            "archive_supported",
            "delete_not_supported",
            "project_archive_editor",
            "eligible_when_selected",
            true,
            false,
            true,
        ),
        authority(
            "project_archive",
            "forge_wiki_project_archive",
            "project",
            "archive_supported",
            "archive_only",
            "project_archive_editor",
            "eligible_when_selected",
            true,
            false,
            true,
        ),
        authority(
            "turn_recall_audit",
            "session_transcript",
            "session",
            "archive_not_supported",
            "audit_retained_until_session_prune",
            "read_only_audit",
            "not_recallable",
            false,
            false,
            false,
        ),
        authority(
            "future_embedding_index",
            "not_created",
            "project/user_profile",
            "migration_design_required",
            "migration_design_required",
            "not_editable",
            "not_recallable_until_index_exists",
            false,
            false,
            false,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn authority(
    source: &str,
    storage: &str,
    scope: &str,
    archive_policy: &str,
    forget_policy: &str,
    edit_policy: &str,
    recall_policy: &str,
    can_archive: bool,
    can_forget: bool,
    recall_eligible: bool,
) -> MemoryAuthorityDescriptor {
    MemoryAuthorityDescriptor {
        source: source.to_string(),
        owner: "desktop_runtime".to_string(),
        storage: storage.to_string(),
        scope: scope.to_string(),
        archive_policy: archive_policy.to_string(),
        forget_policy: forget_policy.to_string(),
        edit_policy: edit_policy.to_string(),
        recall_policy: recall_policy.to_string(),
        can_archive,
        can_forget,
        recall_eligible,
    }
}

fn provenance(storage: &str, source_label: &str) -> UnifiedMemoryProvenance {
    UnifiedMemoryProvenance {
        owner: "desktop_runtime".to_string(),
        storage: storage.to_string(),
        source_label: source_label.to_string(),
    }
}

fn archived_at_ms(status: UnifiedMemoryStatus, updated_at_ms: u64) -> Option<u64> {
    if status == UnifiedMemoryStatus::Archived {
        Some(updated_at_ms)
    } else {
        None
    }
}

fn recall_policy_for_status(status: UnifiedMemoryStatus) -> String {
    match status {
        UnifiedMemoryStatus::Archived => "excluded_when_archived",
        UnifiedMemoryStatus::Forgotten => "excluded_when_forgotten",
        UnifiedMemoryStatus::Candidate => "excluded_when_candidate",
        UnifiedMemoryStatus::Accepted | UnifiedMemoryStatus::Pinned => "eligible_when_current",
    }
    .to_string()
}

pub fn select_unified_context_memories(
    records: &[UnifiedMemoryRecord],
    message: &str,
    project_path: Option<&str>,
    active_profile_id: Option<&str>,
    limit: usize,
) -> Vec<UnifiedMemorySelection> {
    plan_unified_context_memory_recall(
        records,
        message,
        project_path,
        active_profile_id,
        limit,
        u32::MAX,
    )
    .into_selected()
}

pub fn plan_unified_context_memory_recall(
    records: &[UnifiedMemoryRecord],
    message: &str,
    project_path: Option<&str>,
    active_profile_id: Option<&str>,
    limit: usize,
    budget_tokens: u32,
) -> RecallPlan {
    let mut candidates = Vec::with_capacity(records.len());
    let mut scored = Vec::new();
    let mut seen_keys = HashSet::new();
    let low_signal = is_low_signal_query(message);
    let message_terms = terms(message);

    for record in records {
        let estimated_tokens = estimate_record_tokens(record);
        let project_match = project_matches(record, project_path);
        let profile_match = profile_matches(record, active_profile_id);
        let base = RecallCandidateAudit {
            memory_id: record.id.clone(),
            source: source_label(&record.source).to_string(),
            source_id: record.source_id.clone(),
            kind: kind_label(&record.kind).to_string(),
            status: record.status.clone(),
            score: 0.0,
            reason: String::new(),
            decision: RecallDecision::NoRelevanceSignal,
            project_match,
            profile_match: record.profile_id.is_some() && profile_match,
            estimated_tokens,
            rank: None,
        };

        if limit == 0 || low_signal {
            candidates.push(RecallCandidateAudit {
                decision: RecallDecision::LowSignalQuery,
                reason: "low_signal_query".to_string(),
                ..base
            });
            continue;
        }
        if matches!(
            record.status,
            UnifiedMemoryStatus::Forgotten
                | UnifiedMemoryStatus::Archived
                | UnifiedMemoryStatus::Candidate
        ) {
            candidates.push(RecallCandidateAudit {
                decision: RecallDecision::ExcludedStatus,
                reason: recall_policy_for_status(record.status.clone()),
                ..base
            });
            continue;
        }
        if !project_match {
            candidates.push(RecallCandidateAudit {
                decision: RecallDecision::ExcludedProject,
                reason: "project_path_mismatch".to_string(),
                ..base
            });
            continue;
        }
        if !profile_match {
            candidates.push(RecallCandidateAudit {
                decision: RecallDecision::ExcludedProfile,
                reason: "profile_mismatch".to_string(),
                ..base
            });
            continue;
        }

        let dedupe_key = format!("{}:{}", source_label(&record.source), record.source_id);
        if !seen_keys.insert(dedupe_key) {
            candidates.push(RecallCandidateAudit {
                decision: RecallDecision::Duplicate,
                reason: "deduped_by_source_id".to_string(),
                ..base
            });
            continue;
        }

        match score_record(record, &message_terms, project_path, active_profile_id) {
            Some(selection) => {
                let index = candidates.len();
                candidates.push(RecallCandidateAudit {
                    score: selection.score,
                    reason: selection.reason.clone(),
                    decision: RecallDecision::BudgetExceeded,
                    ..base
                });
                scored.push((index, selection, estimated_tokens));
            }
            None => {
                candidates.push(RecallCandidateAudit {
                    reason: "no_relevance_signal".to_string(),
                    ..base
                });
            }
        }
    }

    scored.sort_by(|a, b| {
        b.1.score
            .partial_cmp(&a.1.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.record.title.cmp(&b.1.record.title))
    });

    let mut selected = Vec::new();
    let mut used_tokens = 0_u32;
    for (rank, (candidate_index, selection, estimated_tokens)) in scored.into_iter().enumerate() {
        let can_inject = selected.len() < limit
            && used_tokens
                .checked_add(estimated_tokens)
                .is_some_and(|next| next <= budget_tokens);
        if let Some(candidate) = candidates.get_mut(candidate_index) {
            candidate.rank = Some((rank + 1) as u32);
            if can_inject {
                candidate.decision = RecallDecision::Injected;
                selected.push(selection);
                used_tokens = used_tokens.saturating_add(estimated_tokens);
            } else {
                candidate.decision = RecallDecision::BudgetExceeded;
            }
        }
    }

    let selected_memory_ids = selected
        .iter()
        .map(|selection| selection.record.id.clone())
        .collect::<Vec<_>>();
    RecallPlan {
        selected_memory_ids,
        candidates,
        budget: RecallBudget {
            candidate_count: records.len(),
            injection_limit: limit,
            budget_tokens,
            estimated_injected_tokens: used_tokens,
            injected_count: selected.len(),
        },
        selected,
    }
}

pub fn format_unified_memory_context(selected: &[UnifiedMemorySelection]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }
    let mut lines = Vec::with_capacity(selected.len() + 2);
    lines.push("## Work Memory".to_string());
    lines.push("Use these records only when relevant to the user request. Prefer recent visible conversation when it conflicts. If older details are missing, say so honestly; do not expose memory internals, retrieval internals, or hidden context mechanics to the user.".to_string());
    for selection in selected {
        let record = &selection.record;
        lines.push(format!(
            "- [{}/{}] title={} body={} reason={}",
            source_label(&record.source),
            kind_label(&record.kind),
            json_text(&record.title),
            json_text(&record.body),
            json_text(&selection.reason),
        ));
    }
    Some(lines.join("\n"))
}

fn score_record(
    record: &UnifiedMemoryRecord,
    message_terms: &HashSet<String>,
    project_path: Option<&str>,
    active_profile_id: Option<&str>,
) -> Option<UnifiedMemorySelection> {
    if matches!(
        record.status,
        UnifiedMemoryStatus::Forgotten | UnifiedMemoryStatus::Archived
    ) {
        return None;
    }
    if matches!(record.status, UnifiedMemoryStatus::Candidate) {
        return None;
    }
    if let (Some(active_project), Some(record_project)) =
        (project_path, record.project_path.as_deref())
    {
        if normalize_path(active_project) != normalize_path(record_project) {
            return None;
        }
    }
    if let Some(profile_id) = record.profile_id.as_deref() {
        if Some(profile_id) != active_profile_id {
            return None;
        }
    }

    let mut score = 0.0_f32;
    let mut reasons = Vec::new();
    let mut has_relevance_signal = false;
    if record.status == UnifiedMemoryStatus::Pinned {
        score += 4.0;
        reasons.push("已置顶");
        has_relevance_signal = true;
    }
    if record
        .project_path
        .as_deref()
        .zip(project_path)
        .is_some_and(|(a, b)| normalize_path(a) == normalize_path(b))
    {
        score += 3.0;
        reasons.push("同一项目");
    }
    if record
        .profile_id
        .as_deref()
        .zip(active_profile_id)
        .is_some()
    {
        score += 2.0;
        reasons.push("当前 Profile");
    }

    let record_terms = terms(&format!(
        "{} {} {}",
        record.title,
        record.body,
        record.tags.join(" ")
    ));
    let overlap = message_terms
        .iter()
        .filter(|term| record_terms.contains(*term))
        .count();
    if overlap > 0 {
        score += overlap as f32;
        reasons.push("关键词匹配");
        has_relevance_signal = true;
    }

    let message_text = message_terms.iter().cloned().collect::<Vec<_>>().join(" ");
    if kind_matches_message(&record.kind, &message_text) {
        score += 1.5;
        reasons.push("类别相关");
        has_relevance_signal = true;
    }

    if !has_relevance_signal || score <= 0.0 {
        return None;
    }

    Some(UnifiedMemorySelection {
        record: record.clone(),
        score,
        reason: reasons.join("、"),
        injected: true,
    })
}

fn project_matches(record: &UnifiedMemoryRecord, project_path: Option<&str>) -> bool {
    match (record.project_path.as_deref(), project_path) {
        (Some(record_project), Some(active_project)) => {
            normalize_path(record_project) == normalize_path(active_project)
        }
        _ => true,
    }
}

fn profile_matches(record: &UnifiedMemoryRecord, active_profile_id: Option<&str>) -> bool {
    record
        .profile_id
        .as_deref()
        .is_none_or(|profile_id| Some(profile_id) == active_profile_id)
}

fn estimate_record_tokens(record: &UnifiedMemoryRecord) -> u32 {
    let text_len = record.title.len()
        + record.body.len()
        + record.tags.iter().map(|tag| tag.len()).sum::<usize>()
        + 24;
    ((text_len as u32).saturating_add(3) / 4).max(1)
}

fn map_memory_category(category: MemoryCategory) -> UnifiedMemoryKind {
    match category {
        MemoryCategory::Preference => UnifiedMemoryKind::Preference,
        MemoryCategory::ProjectFact => UnifiedMemoryKind::ProjectFact,
        MemoryCategory::Decision => UnifiedMemoryKind::Decision,
        MemoryCategory::TaskState => UnifiedMemoryKind::TaskState,
    }
}

fn map_memory_scope(scope: MemoryScope) -> UnifiedMemoryScope {
    match scope {
        MemoryScope::Session => UnifiedMemoryScope::Session,
        MemoryScope::UserProfile => UnifiedMemoryScope::UserProfile,
        MemoryScope::Project => UnifiedMemoryScope::Project,
        MemoryScope::Document => UnifiedMemoryScope::Document,
    }
}

fn map_memory_status(status: MemoryStatus) -> UnifiedMemoryStatus {
    match status {
        MemoryStatus::Candidate => UnifiedMemoryStatus::Candidate,
        MemoryStatus::Accepted => UnifiedMemoryStatus::Accepted,
        MemoryStatus::Pinned => UnifiedMemoryStatus::Pinned,
        MemoryStatus::Forgotten => UnifiedMemoryStatus::Forgotten,
        MemoryStatus::Archived => UnifiedMemoryStatus::Archived,
    }
}

fn map_experience_kind(kind: ExperienceKind) -> UnifiedMemoryKind {
    match kind {
        ExperienceKind::Lesson => UnifiedMemoryKind::Lesson,
        ExperienceKind::BugPattern => UnifiedMemoryKind::BugPattern,
        ExperienceKind::Workflow => UnifiedMemoryKind::Workflow,
        ExperienceKind::Decision => UnifiedMemoryKind::Decision,
        ExperienceKind::Preference => UnifiedMemoryKind::Preference,
        ExperienceKind::ProjectFact => UnifiedMemoryKind::ProjectFact,
    }
}

fn map_experience_status(status: ExperienceStatus) -> UnifiedMemoryStatus {
    match status {
        ExperienceStatus::Candidate => UnifiedMemoryStatus::Candidate,
        ExperienceStatus::Accepted => UnifiedMemoryStatus::Accepted,
        ExperienceStatus::Pinned => UnifiedMemoryStatus::Pinned,
        ExperienceStatus::Forgotten => UnifiedMemoryStatus::Forgotten,
        ExperienceStatus::Archived => UnifiedMemoryStatus::Archived,
    }
}

fn terms(text: &str) -> HashSet<String> {
    text.split_whitespace()
        .map(|term| term.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect()
}

fn is_low_signal_query(message: &str) -> bool {
    let normalized = message.split_whitespace().collect::<Vec<_>>().join("");
    matches!(normalized.as_str(), "继续" | "可以" | "好的" | "ok" | "OK")
}

fn normalize_path(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

fn json_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    serde_json::to_string(&normalized).unwrap_or_else(|_| "\"\"".to_string())
}

fn kind_matches_message(kind: &UnifiedMemoryKind, message: &str) -> bool {
    match kind {
        UnifiedMemoryKind::Preference => contains_any(message, &["偏好", "习惯", "以后", "默认"]),
        UnifiedMemoryKind::Decision => contains_any(message, &["方向", "方案", "决定", "之前"]),
        UnifiedMemoryKind::TaskState => contains_any(
            message,
            &["进度", "继续", "接着", "做到", "下一步", "continue"],
        ),
        UnifiedMemoryKind::ProjectFact => {
            contains_any(message, &["项目", "工作区", "路径", "repo"])
        }
        UnifiedMemoryKind::Lesson | UnifiedMemoryKind::BugPattern | UnifiedMemoryKind::Workflow => {
            contains_any(
                message,
                &["经验", "问题", "修复", "测试", "失败", "bug", "workflow"],
            )
        }
    }
}

fn contains_any(text: &str, signals: &[&str]) -> bool {
    signals.iter().any(|signal| text.contains(signal))
}

fn source_label(source: &UnifiedMemorySource) -> &'static str {
    match source {
        UnifiedMemorySource::WikiMemory => "wiki_memory",
        UnifiedMemorySource::MemoryFact => "memory_fact",
        UnifiedMemorySource::ContinuityExperience => "continuity_experience",
    }
}

fn kind_label(kind: &UnifiedMemoryKind) -> &'static str {
    match kind {
        UnifiedMemoryKind::Preference => "preference",
        UnifiedMemoryKind::ProjectFact => "project_fact",
        UnifiedMemoryKind::Decision => "decision",
        UnifiedMemoryKind::TaskState => "task_state",
        UnifiedMemoryKind::Lesson => "lesson",
        UnifiedMemoryKind::BugPattern => "bug_pattern",
        UnifiedMemoryKind::Workflow => "workflow",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::{ExperienceKind, ExperienceMemory, ExperienceStatus};
    use crate::memory::facts::MemoryFact;
    use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};

    fn wiki_memory() -> WikiMemory {
        WikiMemory {
            id: "wiki-1".to_string(),
            category: MemoryCategory::TaskState,
            scope: MemoryScope::Project,
            status: MemoryStatus::Accepted,
            title: "权限进度".to_string(),
            body: "已经完成完全访问按钮，下一步检查确认卡片".to_string(),
            project_path: Some("/repo/forge".to_string()),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: vec!["msg-1".to_string()],
            confidence: 0.82,
            created_at: "1772582400000".to_string(),
            updated_at: "1772582400001".to_string(),
            last_used_at: None,
            use_count: 2,
            tags: vec!["task_state".to_string()],
        }
    }

    #[test]
    fn maps_wiki_memory_into_unified_record() {
        let record = UnifiedMemoryRecord::from_wiki_memory(wiki_memory());

        assert_eq!(record.id, "wiki_memory:wiki-1");
        assert_eq!(record.source, UnifiedMemorySource::WikiMemory);
        assert_eq!(record.source_id, "wiki-1");
        assert_eq!(record.kind, UnifiedMemoryKind::TaskState);
        assert_eq!(record.status, UnifiedMemoryStatus::Accepted);
        assert_eq!(record.scope, UnifiedMemoryScope::Project);
        assert_eq!(record.project_path.as_deref(), Some("/repo/forge"));
        assert_eq!(record.profile_id, None);
        assert_eq!(record.title, "权限进度");
        assert!(record.body.contains("完全访问按钮"));
        assert_eq!(record.source_session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn memory_authority_map_v2_documents_required_sources() {
        let map = memory_authority_map_v2();
        let sources = map
            .iter()
            .map(|entry| entry.source.as_str())
            .collect::<Vec<_>>();

        assert!(sources.contains(&"wiki_memory"));
        assert!(sources.contains(&"memory_fact"));
        assert!(sources.contains(&"continuity_experience"));
        assert!(sources.contains(&"saved_background"));
        assert!(sources.contains(&"project_archive"));
        assert!(sources.contains(&"turn_recall_audit"));
        assert!(sources.contains(&"future_embedding_index"));

        let fact = map
            .iter()
            .find(|entry| entry.source == "memory_fact")
            .expect("memory fact authority");
        assert_eq!(fact.owner, "desktop_runtime");
        assert_eq!(fact.storage, "profile_memory_facts");
        assert!(!fact.can_archive);
        assert!(fact.can_forget);
        assert_eq!(fact.edit_policy, "profile_fact_detail_editor");

        let recall_audit = map
            .iter()
            .find(|entry| entry.source == "turn_recall_audit")
            .expect("turn recall audit authority");
        assert!(!recall_audit.recall_eligible);
        assert_eq!(
            recall_audit.forget_policy,
            "audit_retained_until_session_prune"
        );
    }

    #[test]
    fn unified_memory_record_v2_metadata_is_populated_for_existing_sources() {
        let wiki = UnifiedMemoryRecord::from_wiki_memory(wiki_memory());
        assert_eq!(wiki.visibility, UnifiedMemoryVisibility::UserVisible);
        assert_eq!(wiki.provenance.owner, "desktop_runtime");
        assert_eq!(wiki.provenance.storage, "wiki_memory_store");
        assert_eq!(wiki.last_used_at_ms, None);
        assert_eq!(wiki.forget_policy, "forget_supported");
        assert_eq!(wiki.recall_policy, "eligible_when_current");

        let fact = UnifiedMemoryRecord::from_memory_fact(MemoryFact {
            id: "fact-1".to_string(),
            text: "默认使用完全访问模式测试 Forge demo".to_string(),
            tags: vec!["preference".to_string()],
            profile_id: Some("work".to_string()),
            source: Some("settings".to_string()),
            created_at_ms: 1772582400000,
            updated_at_ms: 1772582400001,
        });
        assert_eq!(fact.provenance.storage, "profile_memory_facts");
        assert_eq!(fact.forget_policy, "delete_supported");
        assert_eq!(fact.recall_policy, "eligible_when_profile_matches");

        let mut archived = wiki_memory();
        archived.status = MemoryStatus::Archived;
        archived.updated_at = "1772582400002".to_string();
        let archived = UnifiedMemoryRecord::from_wiki_memory(archived);
        assert_eq!(archived.status, UnifiedMemoryStatus::Archived);
        assert_eq!(archived.archived_at_ms, Some(1772582400002));
        assert_eq!(archived.recall_policy, "excluded_when_archived");
    }

    #[test]
    fn maps_profile_fact_into_unified_record() {
        let fact = MemoryFact {
            id: "fact-1".to_string(),
            text: "默认使用完全访问模式测试 Forge demo".to_string(),
            tags: vec!["preference".to_string()],
            profile_id: Some("work".to_string()),
            source: Some("settings".to_string()),
            created_at_ms: 1772582400000,
            updated_at_ms: 1772582400001,
        };

        let record = UnifiedMemoryRecord::from_memory_fact(fact);

        assert_eq!(record.id, "memory_fact:fact-1");
        assert_eq!(record.source, UnifiedMemorySource::MemoryFact);
        assert_eq!(record.source_id, "fact-1");
        assert_eq!(record.kind, UnifiedMemoryKind::ProjectFact);
        assert_eq!(record.status, UnifiedMemoryStatus::Accepted);
        assert_eq!(record.scope, UnifiedMemoryScope::UserProfile);
        assert_eq!(record.profile_id.as_deref(), Some("work"));
        assert!(record.body.contains("完全访问模式"));
    }

    #[test]
    fn maps_continuity_experience_into_unified_record() {
        let experience = ExperienceMemory {
            id: "experience-1".to_string(),
            kind: ExperienceKind::BugPattern,
            status: ExperienceStatus::Pinned,
            title: "确认卡片回归".to_string(),
            body: "Permission trust state can still show confirmation cards after restart."
                .to_string(),
            project_path: Some("/repo/forge".to_string()),
            source_session_id: Some("session-2".to_string()),
            confidence: 0.91,
            created_at_ms: 1772582400000,
            updated_at_ms: 1772582400001,
            tags: vec!["permission".to_string()],
        };

        let record = UnifiedMemoryRecord::from_continuity_experience(experience);

        assert_eq!(record.id, "continuity_experience:experience-1");
        assert_eq!(record.source, UnifiedMemorySource::ContinuityExperience);
        assert_eq!(record.source_id, "experience-1");
        assert_eq!(record.kind, UnifiedMemoryKind::BugPattern);
        assert_eq!(record.status, UnifiedMemoryStatus::Pinned);
        assert_eq!(record.scope, UnifiedMemoryScope::Project);
        assert_eq!(record.project_path.as_deref(), Some("/repo/forge"));
        assert_eq!(record.source_session_id.as_deref(), Some("session-2"));
    }

    #[test]
    fn selector_keeps_only_injectable_records() {
        let mut pinned = UnifiedMemoryRecord::from_wiki_memory(wiki_memory());
        pinned.status = UnifiedMemoryStatus::Pinned;
        pinned.body = "完全访问 权限 按钮 已完成".to_string();

        let mut candidate = pinned.clone();
        candidate.id = "continuity_experience:candidate".to_string();
        candidate.source = UnifiedMemorySource::ContinuityExperience;
        candidate.status = UnifiedMemoryStatus::Candidate;
        candidate.body = "候选经验不应自动注入 权限".to_string();

        let selected = select_unified_context_memories(
            &[candidate, pinned],
            "继续 权限 按钮 问题",
            Some("/repo/forge"),
            Some("work"),
            5,
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].record.status, UnifiedMemoryStatus::Pinned);
        assert!(selected[0].reason.contains("已置顶"));
    }

    #[test]
    fn recall_planner_audits_candidates_dedupes_and_reports_budget() {
        let mut primary = UnifiedMemoryRecord::from_wiki_memory(wiki_memory());
        primary.id = "wiki_memory:primary".to_string();
        primary.source_id = "primary".to_string();
        primary.title = "permission confirmation card".to_string();
        primary.body = "permission confirmation fix requires ledger evidence".to_string();
        primary.tags = vec!["permission".to_string()];

        let duplicate = primary.clone();

        let mut archived = primary.clone();
        archived.id = "wiki_memory:archived".to_string();
        archived.source_id = "archived".to_string();
        archived.status = UnifiedMemoryStatus::Archived;

        let mut other_project = primary.clone();
        other_project.id = "wiki_memory:other-project".to_string();
        other_project.source_id = "other-project".to_string();
        other_project.project_path = Some("/repo/other".to_string());

        let mut low_relevance = primary.clone();
        low_relevance.id = "wiki_memory:low-relevance".to_string();
        low_relevance.source_id = "low-relevance".to_string();
        low_relevance.title = "unrelated coffee note".to_string();
        low_relevance.body = "espresso grind size".to_string();
        low_relevance.tags.clear();

        let records = vec![primary, duplicate, archived, other_project, low_relevance];
        let plan = plan_unified_context_memory_recall(
            &records,
            "permission confirmation fix",
            Some("/repo/forge"),
            None,
            1,
            256,
        );

        assert_eq!(plan.budget.candidate_count, 5);
        assert_eq!(plan.budget.injection_limit, 1);
        assert_eq!(plan.budget.injected_count, 1);
        assert_eq!(plan.selected_memory_ids, vec!["wiki_memory:primary"]);
        assert!(plan.budget.estimated_injected_tokens > 0);

        let decisions = plan
            .candidates
            .iter()
            .map(|candidate| (candidate.memory_id.as_str(), candidate.decision.clone()))
            .collect::<Vec<_>>();
        assert!(decisions.contains(&("wiki_memory:primary", RecallDecision::Injected)));
        assert!(decisions.contains(&("wiki_memory:primary", RecallDecision::Duplicate)));
        assert!(decisions.contains(&("wiki_memory:archived", RecallDecision::ExcludedStatus)));
        assert!(decisions.contains(&("wiki_memory:other-project", RecallDecision::ExcludedProject)));
        assert!(decisions.contains(&(
            "wiki_memory:low-relevance",
            RecallDecision::NoRelevanceSignal
        )));
        let json = serde_json::to_string(&plan).expect("serialize recall plan");
        assert!(json.contains("\"budget\""));
        assert!(!json.contains("permission confirmation fix requires ledger evidence"));
    }

    #[test]
    fn formatter_preserves_source_and_reason_without_leaking_internals() {
        let record = UnifiedMemoryRecord::from_wiki_memory(wiki_memory());
        let selected = vec![UnifiedMemorySelection {
            record,
            score: 7.0,
            reason: "同一项目、关键词匹配".to_string(),
            injected: true,
        }];

        let context = format_unified_memory_context(&selected).expect("context");

        assert!(context.contains("## Work Memory"));
        assert!(context.contains("[wiki_memory/task_state]"));
        assert!(context.contains("权限进度"));
        assert!(context.contains("同一项目"));
        assert!(context.contains("do not expose memory internals"));
    }
}
