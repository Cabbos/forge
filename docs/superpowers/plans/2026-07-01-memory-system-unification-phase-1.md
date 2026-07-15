# Memory System Unification Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify Forge's user facts, wiki memories, and continuity experiences into one explainable read model and one send-input recall path, without migrating physical storage in Phase 1.

**Architecture:** Keep the existing stores in place: `WikiMemoryStore`, `MemoryFactStore`, and `ContinuityStore` remain the sources of truth. Add a unified read model and selector that normalizes records from all three sources into `UnifiedMemoryRecord`, then route UI, IPC, and send-input context through that normalized layer. This is a product/API unification first, not a database migration.

**Tech Stack:** Rust/Tauri IPC, SQLite FTS5 continuity store, JSON-backed memory stores, React + TypeScript, TanStack Query, existing Forge context UI primitives.

**Execution Status:** Implemented on `2026-07-01` as the Phase 1 slice: unified read model, project archive overview, send-input recall path, docs, and focused acceptance coverage. The original checklist below is kept as the executable design record rather than rewritten into a completion report.

---

## Scope

Phase 1 deliberately does **not** migrate `~/.forge/wiki_memories.json`, `~/.forge/memory.json`, or project `.forge/continuity.db` into one database. It does unify:

- the product concept: one project-facing memory overview;
- the IPC read model: one list/search endpoint;
- the send-input selection path: one selector and one audit payload;
- status semantics: keep source-specific status handling intact, and expose enough metadata for Phase 2 unified actions;
- explainability: every injected record exposes source, reason, score, and provenance.

Gateway-owned execution and embeddings stay out of this phase.

## Current Sources

- `apps/desktop/src-tauri/src/memory/storage.rs`: stores `WikiMemory` records in `~/.forge/wiki_memories.json`.
- `apps/desktop/src-tauri/src/memory/facts.rs`: stores user-managed `MemoryFact` records in `~/.forge/memory.json`.
- `apps/desktop/src-tauri/src/continuity/store.rs`: stores project `ExperienceMemory` records in `.forge/continuity.db`.
- `apps/desktop/src-tauri/src/ipc/send_input_context.rs`: currently selects wiki memory, memory facts, and continuity context through separate paths.
- `apps/desktop/src/components/context/WikiSectionsView.tsx`: currently renders wiki/project records and continuity experiences as separate surfaces.
- `apps/desktop/src/components/settings/MemoryPanel.tsx`: remains the profile/user fact management surface.

## Target Files

- Create: `apps/desktop/src-tauri/src/memory/unified.rs`
  - Pure Rust unified memory types, source mappers, scoring helpers, and formatter.
- Modify: `apps/desktop/src-tauri/src/memory/mod.rs`
  - Export unified memory types and helpers.
- Create: `apps/desktop/src-tauri/src/ipc/unified_memory.rs`
  - Tauri commands and crate-private helpers that collect records from all stores.
- Modify: `apps/desktop/src-tauri/src/ipc/mod.rs`
  - Register the IPC module.
- Modify: `apps/desktop/src-tauri/src/lib.rs`
  - Add unified memory commands to `tauri::generate_handler!`.
- Modify: `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
  - Replace split memory/continuity selection with unified selection while preserving hidden context behavior.
- Modify: `apps/desktop/src-tauri/src/ipc/send_input_context_tests.rs`
  - Update tests for unified selected memory and continuity behavior.
- Modify: `apps/desktop/src/lib/ipc/types.ts`
  - Add TypeScript mirrors for unified memory records and selection payloads.
- Create: `apps/desktop/src/lib/ipc/unifiedMemory.ts`
  - Add frontend IPC wrappers.
- Modify: `apps/desktop/src/lib/tauri.ts`
  - Export unified memory IPC wrappers.
- Modify: `apps/desktop/src/hooks/queries/queryKeys.ts`
  - Add unified memory query keys.
- Create: `apps/desktop/src/hooks/queries/useUnifiedMemoriesQuery.ts`
  - Fetch unified project memory records.
- Create: `apps/desktop/src/components/context/UnifiedMemorySection.tsx`
  - New project-facing unified memory read surface.
- Modify: `apps/desktop/src/components/context/WikiSectionsView.tsx`
  - Add `UnifiedMemorySection` above existing source-specific management sections.
- Keep: `apps/desktop/src/components/settings/MemoryPanel.tsx`
  - Settings stays focused on profile/user facts.
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
  - Add product-level smoke for unified project memory visibility and search.
- Modify: `README.md`, `apps/desktop/README.md`, `CHANGELOG.md`
  - Document user-visible memory unification.

---

### Task 1: Add Unified Memory Domain Model

**Files:**
- Create: `apps/desktop/src-tauri/src/memory/unified.rs`
- Modify: `apps/desktop/src-tauri/src/memory/mod.rs`

- [ ] **Step 1: Write failing mapper tests**

Add this test module to the new file `apps/desktop/src-tauri/src/memory/unified.rs` before implementing the helpers:

```rust
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
            body: "Permission trust state can still show confirmation cards after restart.".to_string(),
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
}
```

- [ ] **Step 2: Run the mapper test and verify it fails**

Run:

```bash
cd apps/desktop/src-tauri
cargo test memory::unified::tests::maps_wiki_memory_into_unified_record --lib
```

Expected: FAIL because `memory::unified` and `UnifiedMemoryRecord` do not exist.

- [ ] **Step 3: Implement the unified model and mappers**

Create `apps/desktop/src-tauri/src/memory/unified.rs` with these public types and mapper methods:

```rust
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMemorySelection {
    pub record: UnifiedMemoryRecord,
    pub score: f32,
    pub reason: String,
    pub injected: bool,
}

impl UnifiedMemoryRecord {
    pub fn from_wiki_memory(memory: WikiMemory) -> Self {
        Self {
            id: format!("wiki_memory:{}", memory.id),
            source: UnifiedMemorySource::WikiMemory,
            source_id: memory.id,
            kind: map_memory_category(memory.category),
            status: map_memory_status(memory.status),
            scope: map_memory_scope(memory.scope),
            title: memory.title,
            body: memory.body,
            project_path: memory.project_path,
            profile_id: None,
            source_session_id: memory.source_session_id,
            confidence: memory.confidence,
            created_at_ms: memory.created_at.parse().unwrap_or(0),
            updated_at_ms: memory.updated_at.parse().unwrap_or(0),
            tags: memory.tags,
        }
    }

    pub fn from_memory_fact(fact: MemoryFact) -> Self {
        let title = fact
            .tags
            .first()
            .map(|tag| format!("记忆事实: {tag}"))
            .unwrap_or_else(|| "记忆事实".to_string());
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
        }
    }

    pub fn from_continuity_experience(experience: ExperienceMemory) -> Self {
        Self {
            id: format!("continuity_experience:{}", experience.id),
            source: UnifiedMemorySource::ContinuityExperience,
            source_id: experience.id,
            kind: map_experience_kind(experience.kind),
            status: map_experience_status(experience.status),
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
        }
    }
}
```

Also add the private mapping helpers in the same file:

```rust
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
```

Modify `apps/desktop/src-tauri/src/memory/mod.rs`:

```rust
pub mod unified;

pub use unified::{
    UnifiedMemoryKind, UnifiedMemoryRecord, UnifiedMemoryScope, UnifiedMemorySelection,
    UnifiedMemorySource, UnifiedMemoryStatus,
};
```

- [ ] **Step 4: Run mapper tests**

Run:

```bash
cd apps/desktop/src-tauri
cargo test memory::unified::tests --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/memory/mod.rs apps/desktop/src-tauri/src/memory/unified.rs
git commit -m "feat(desktop): add unified memory read model"
```

---

### Task 2: Add Unified Selection and Formatting

**Files:**
- Modify: `apps/desktop/src-tauri/src/memory/unified.rs`

- [ ] **Step 1: Write failing selector tests**

Add these tests to `apps/desktop/src-tauri/src/memory/unified.rs`:

```rust
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
        "继续权限按钮问题",
        Some("/repo/forge"),
        Some("work"),
        5,
    );

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].record.status, UnifiedMemoryStatus::Pinned);
    assert!(selected[0].reason.contains("已置顶"));
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
```

- [ ] **Step 2: Run selector tests and verify they fail**

Run:

```bash
cd apps/desktop/src-tauri
cargo test memory::unified::tests::selector_keeps_only_injectable_records --lib
```

Expected: FAIL because `select_unified_context_memories` does not exist.

- [ ] **Step 3: Implement selector helpers**

Add these functions to `apps/desktop/src-tauri/src/memory/unified.rs`:

```rust
pub fn select_unified_context_memories(
    records: &[UnifiedMemoryRecord],
    message: &str,
    project_path: Option<&str>,
    active_profile_id: Option<&str>,
    limit: usize,
) -> Vec<UnifiedMemorySelection> {
    if limit == 0 || is_low_signal_query(message) {
        return Vec::new();
    }

    let message_terms = terms(message);
    let mut selected = records
        .iter()
        .filter_map(|record| score_record(record, &message_terms, project_path, active_profile_id))
        .collect::<Vec<_>>();

    selected.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.record.title.cmp(&b.record.title))
    });
    selected.truncate(limit);
    selected
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
```

Add these private helpers in the same file:

```rust
fn score_record(
    record: &UnifiedMemoryRecord,
    message_terms: &std::collections::HashSet<String>,
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
    if record.status == UnifiedMemoryStatus::Pinned {
        score += 4.0;
        reasons.push("已置顶");
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
    if record.profile_id.as_deref().zip(active_profile_id).is_some() {
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
    }

    if score <= 0.0 {
        return None;
    }

    Some(UnifiedMemorySelection {
        record: record.clone(),
        score,
        reason: reasons.join("、"),
        injected: true,
    })
}

fn terms(text: &str) -> std::collections::HashSet<String> {
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
```

Also add `source_label` and `kind_label` helpers:

```rust
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
```

- [ ] **Step 4: Run selector tests**

Run:

```bash
cd apps/desktop/src-tauri
cargo test memory::unified::tests --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/memory/unified.rs
git commit -m "feat(desktop): select unified memory context"
```

---

### Task 3: Add Unified Memory IPC and TypeScript Client

**Files:**
- Create: `apps/desktop/src-tauri/src/ipc/unified_memory.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/lib/ipc/types.ts`
- Create: `apps/desktop/src/lib/ipc/unifiedMemory.ts`
- Modify: `apps/desktop/src/lib/tauri.ts`
- Modify: `apps/desktop/src/hooks/queries/queryKeys.ts`
- Create: `apps/desktop/src/hooks/queries/useUnifiedMemoriesQuery.ts`

- [ ] **Step 1: Write failing Rust IPC tests**

Add this test module to `apps/desktop/src-tauri/src/ipc/unified_memory.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Harness;
    use crate::memory::facts::{MemoryFactStore, UpsertMemoryFactInput};
    use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
    use crate::memory::storage::WikiMemoryStore;
    use crate::profile::{ProfileStore, UpsertProfileInput};
    use crate::state::AppState;
    use std::sync::Arc;

    #[tokio::test]
    async fn unified_memory_records_include_wiki_and_active_profile_facts() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-unified-memory-{nonce}"));
        std::fs::create_dir_all(&workspace).expect("workspace");
        let wiki_path = std::env::temp_dir().join(format!("forge-unified-wiki-{nonce}.json"));
        let facts_path = std::env::temp_dir().join(format!("forge-unified-facts-{nonce}.json"));
        let profiles_path = std::env::temp_dir().join(format!("forge-unified-profiles-{nonce}.json"));

        let mut app_state = AppState::new(Arc::new(Harness::new(workspace.clone())));
        app_state.wiki_memory = Arc::new(WikiMemoryStore::new(wiki_path.clone()));
        app_state.memory_facts = Arc::new(MemoryFactStore::new(facts_path.clone()));
        app_state.profiles = Arc::new(ProfileStore::new(profiles_path.clone()));
        let state = Arc::new(app_state);

        let work = state.profiles.upsert(UpsertProfileInput {
            id: Some("work".to_string()),
            name: "Work".to_string(),
            default_provider: None,
            default_model: None,
            default_workspace: None,
            api_key_overrides: None,
        }).expect("profile");
        state.profiles.set_active(&work.id).expect("active profile");

        state.wiki_memory.upsert_candidate(WikiMemory {
            id: "wiki-progress".to_string(),
            category: MemoryCategory::TaskState,
            scope: MemoryScope::Project,
            status: MemoryStatus::Accepted,
            title: "权限进度".to_string(),
            body: "完全访问按钮已完成".to_string(),
            project_path: Some(workspace.to_string_lossy().to_string()),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1772582400000".to_string(),
            updated_at: "1772582400000".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["task_state".to_string()],
        }).await.expect("wiki");

        state.memory_facts.upsert(UpsertMemoryFactInput {
            id: Some("fact-work".to_string()),
            text: "完全访问模式用于测试权限绕过".to_string(),
            tags: vec!["permission".to_string()],
            profile_id: Some("work".to_string()),
            source: Some("settings".to_string()),
        }).expect("fact");
        state.memory_facts.upsert(UpsertMemoryFactInput {
            id: Some("fact-personal".to_string()),
            text: "不应该出现在 work profile".to_string(),
            tags: vec!["private".to_string()],
            profile_id: Some("personal".to_string()),
            source: Some("settings".to_string()),
        }).expect("other profile fact");

        let records = list_unified_memories_for_request(
            &state,
            Some("session-1"),
            Some(workspace.to_str().expect("utf8")),
            None,
        ).await.expect("records");

        let ids = records.iter().map(|record| record.id.as_str()).collect::<Vec<_>>();
        assert!(ids.contains(&"wiki_memory:wiki-progress"));
        assert!(ids.contains(&"memory_fact:fact-work"));
        assert!(!ids.contains(&"memory_fact:fact-personal"));

        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_file(wiki_path);
        let _ = std::fs::remove_file(facts_path);
        let _ = std::fs::remove_file(profiles_path);
    }
}
```

- [ ] **Step 2: Run the IPC test and verify it fails**

Run:

```bash
cd apps/desktop/src-tauri
cargo test ipc::unified_memory::tests::unified_memory_records_include_wiki_and_active_profile_facts --lib
```

Expected: FAIL because the `ipc::unified_memory` module does not exist.

- [ ] **Step 3: Implement unified memory IPC helpers and commands**

Create `apps/desktop/src-tauri/src/ipc/unified_memory.rs` with:

```rust
use std::sync::Arc;

use crate::ipc::workspace_files::working_dir_for_request_or_explicit;
use crate::memory::{
    select_unified_context_memories, UnifiedMemoryRecord, UnifiedMemorySelection,
};
use crate::state::AppState;

const UNIFIED_MEMORY_DEFAULT_LIMIT: usize = 20;
const UNIFIED_CONTEXT_DEFAULT_LIMIT: usize = 8;

pub(crate) async fn list_unified_memories_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
    query: Option<&str>,
) -> Result<Vec<UnifiedMemoryRecord>, String> {
    let working_dir = working_dir_for_request_or_explicit(state, session_id, working_dir).await?;
    let project_path = working_dir.to_string_lossy().to_string();
    let active_profile_id = state.profiles.active_profile_id();
    let mut records = collect_unified_memory_records(state, &project_path, active_profile_id.as_deref()).await?;

    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        let query_lower = query.to_lowercase();
        records.retain(|record| {
            record.title.to_lowercase().contains(&query_lower)
                || record.body.to_lowercase().contains(&query_lower)
                || record.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
        });
    }

    records.truncate(UNIFIED_MEMORY_DEFAULT_LIMIT);
    Ok(records)
}

pub(crate) async fn select_unified_memories_for_send_input(
    state: &Arc<AppState>,
    text: &str,
    project_path: &str,
) -> Result<Vec<UnifiedMemorySelection>, String> {
    let active_profile_id = state.profiles.active_profile_id();
    let records = collect_unified_memory_records(state, project_path, active_profile_id.as_deref()).await?;
    Ok(select_unified_context_memories(
        &records,
        text,
        Some(project_path),
        active_profile_id.as_deref(),
        UNIFIED_CONTEXT_DEFAULT_LIMIT,
    ))
}

async fn collect_unified_memory_records(
    state: &Arc<AppState>,
    project_path: &str,
    active_profile_id: Option<&str>,
) -> Result<Vec<UnifiedMemoryRecord>, String> {
    let mut records = Vec::new();

    let wiki = state
        .wiki_memory
        .list(crate::memory::MemoryListFilter {
            scope: None,
            project_path: Some(project_path.to_string()),
        })
        .await;
    records.extend(wiki.into_iter().map(UnifiedMemoryRecord::from_wiki_memory));

    let facts = state
        .memory_facts
        .list_with_filter(crate::memory::facts::MemoryFactListFilter {
            query: None,
            profile_id: active_profile_id,
        });
    records.extend(facts.into_iter().map(UnifiedMemoryRecord::from_memory_fact));

    let experiences = state.continuity.list_experiences_for_project(project_path)?;
    records.extend(
        experiences
            .into_iter()
            .map(UnifiedMemoryRecord::from_continuity_experience),
    );

    Ok(records)
}

#[tauri::command]
pub async fn list_unified_memories(
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
    query: Option<String>,
) -> Result<Vec<UnifiedMemoryRecord>, String> {
    list_unified_memories_for_request(
        &state,
        session_id.as_deref(),
        working_dir.as_deref(),
        query.as_deref(),
    )
    .await
}
```

If `ProfileStore::active_profile_id()` does not exist, add this minimal method in `apps/desktop/src-tauri/src/profile/mod.rs`:

```rust
pub fn active_profile_id(&self) -> Option<String> {
    self.list_payload().active_profile_id
}
```

- [ ] **Step 4: Register IPC module and command**

Modify `apps/desktop/src-tauri/src/ipc/mod.rs`:

```rust
pub mod unified_memory;
```

Modify `apps/desktop/src-tauri/src/lib.rs` inside `tauri::generate_handler!`:

```rust
ipc::unified_memory::list_unified_memories,
```

- [ ] **Step 5: Add TypeScript types and IPC wrapper**

Modify `apps/desktop/src/lib/ipc/types.ts`:

```ts
export type UnifiedMemorySource = "wiki_memory" | "memory_fact" | "continuity_experience";
export type UnifiedMemoryKind =
  | "preference"
  | "project_fact"
  | "decision"
  | "task_state"
  | "lesson"
  | "bug_pattern"
  | "workflow";
export type UnifiedMemoryScope = "session" | "user_profile" | "project" | "document";
export type UnifiedMemoryStatus = "candidate" | "accepted" | "pinned" | "forgotten" | "archived";

export interface UnifiedMemoryRecord {
  id: string;
  source: UnifiedMemorySource;
  source_id: string;
  kind: UnifiedMemoryKind;
  status: UnifiedMemoryStatus;
  scope: UnifiedMemoryScope;
  title: string;
  body: string;
  project_path?: string | null;
  profile_id?: string | null;
  source_session_id?: string | null;
  confidence: number;
  created_at_ms: number;
  updated_at_ms: number;
  tags: string[];
}
```

Create `apps/desktop/src/lib/ipc/unifiedMemory.ts`:

```ts
import { invoke } from "./core";
import type { UnifiedMemoryRecord } from "./types";

export async function listUnifiedMemories(
  sessionId?: string,
  workingDir?: string | null,
  query?: string,
): Promise<UnifiedMemoryRecord[]> {
  return invoke("list_unified_memories", {
    sessionId,
    workingDir,
    query,
  });
}
```

Modify `apps/desktop/src/lib/tauri.ts`:

```ts
export * from "./ipc/unifiedMemory";
```

- [ ] **Step 6: Add query hook**

Modify `apps/desktop/src/hooks/queries/queryKeys.ts`:

```ts
unifiedMemories: (
  sessionId: string | undefined | null,
  projectPath: string | null | undefined,
  query?: string,
) => ["unified-memories", sessionId ?? "", projectPath ?? "", query ?? ""] as const,
unifiedMemoriesAll: ["unified-memories"] as const,
```

Create `apps/desktop/src/hooks/queries/useUnifiedMemoriesQuery.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { listUnifiedMemories, type UnifiedMemoryRecord } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useUnifiedMemoriesQuery(
  sessionId: string | null | undefined,
  projectPath: string | null | undefined,
  query: string,
  enabled: boolean,
) {
  return useQuery<UnifiedMemoryRecord[]>({
    queryKey: queryKeys.unifiedMemories(sessionId, projectPath, query),
    enabled,
    queryFn: async () => {
      return await listUnifiedMemories(sessionId ?? undefined, projectPath ?? undefined, query || undefined);
    },
  });
}
```

- [ ] **Step 7: Run checks**

Run:

```bash
cd apps/desktop/src-tauri
cargo test ipc::unified_memory::tests --lib
cd ..
npm run build
```

Expected: Rust test PASS and frontend build PASS.

- [ ] **Step 8: Commit**

```bash
git add apps/desktop/src-tauri/src/ipc/unified_memory.rs apps/desktop/src-tauri/src/ipc/mod.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src/lib/ipc/types.ts apps/desktop/src/lib/ipc/unifiedMemory.ts apps/desktop/src/lib/tauri.ts apps/desktop/src/hooks/queries/queryKeys.ts apps/desktop/src/hooks/queries/useUnifiedMemoriesQuery.ts
git commit -m "feat(desktop): expose unified memory records"
```

---

### Task 4: Add One Unified Project Memory Read Section

**Files:**
- Create: `apps/desktop/src/components/context/UnifiedMemorySection.tsx`
- Modify: `apps/desktop/src/components/context/WikiSectionsView.tsx`

- [ ] **Step 1: Create unified section component**

Create `apps/desktop/src/components/context/UnifiedMemorySection.tsx`:

```tsx
import { useMemo, useState } from "react";
import { Brain, Search } from "lucide-react";
import { ForgeSurface } from "@/components/primitives/surface";
import { useUnifiedMemoriesQuery } from "@/hooks/queries/useUnifiedMemoriesQuery";
import type { UnifiedMemoryRecord } from "@/lib/tauri";
import { EmptyState, RowIntentLabel, SectionHeader } from "./WikiSectionChrome";

interface UnifiedMemorySectionProps {
  currentProjectPath: string;
  sessionId: string | null;
}

export function UnifiedMemorySection({ currentProjectPath, sessionId }: UnifiedMemorySectionProps) {
  const [query, setQuery] = useState("");
  const trimmedQuery = query.trim();
  const {
    data: memories = [],
    isFetching,
    refetch,
  } = useUnifiedMemoriesQuery(sessionId, currentProjectPath, trimmedQuery, Boolean(currentProjectPath));

  const visible = useMemo(
    () => memories.filter((memory) => memory.status !== "forgotten" && memory.status !== "archived"),
    [memories],
  );

  return (
    <section>
      <SectionHeader
        title="记忆"
        meta={visible.length > 0 ? `${visible.length} 条` : null}
        loading={isFetching}
        onRefresh={() => refetch()}
        refreshDisabled={isFetching}
      />
      <ForgeSurface className="overflow-hidden">
        <div className="border-b border-border px-3 py-2">
          <label className="flex h-8 items-center gap-2 rounded-md border border-border bg-background/70 px-2 text-[11px] text-muted-foreground focus-within:border-primary/40">
            <Search className="size-3.5 shrink-0" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="搜索记忆、经验、背景"
              className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
            />
          </label>
        </div>
        {!currentProjectPath ? (
          <EmptyState label="打开项目后可以查看记忆" />
        ) : visible.length === 0 ? (
          <EmptyState label={trimmedQuery ? "没有匹配记忆" : "还没有记忆"} />
        ) : (
          <div className="divide-y divide-border">
            {visible.map((memory) => (
              <UnifiedMemoryRow key={memory.id} memory={memory} />
            ))}
          </div>
        )}
      </ForgeSurface>
    </section>
  );
}

function UnifiedMemoryRow({ memory }: { memory: UnifiedMemoryRecord }) {
  return (
    <div className="px-3 py-2.5">
      <div className="flex items-start gap-2">
        <Brain className="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <RowIntentLabel>{sourceLabel(memory.source)}</RowIntentLabel>
            <span className="truncate text-[10px] text-muted-foreground/70">{kindLabel(memory.kind)}</span>
            <span className="ml-auto shrink-0 font-mono text-[10px] text-muted-foreground/60">
              {Math.round(memory.confidence * 100)}%
            </span>
          </div>
          <div className="mt-1 truncate text-xs font-medium text-foreground">{memory.title}</div>
          <div className="mt-1 max-h-[4.6rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {memory.body}
          </div>
          <div className="mt-2 flex flex-wrap gap-1 text-[10px] text-muted-foreground/60">
            <span>{statusLabel(memory.status)}</span>
            {memory.source_session_id && <span className="font-mono">{memory.source_session_id}</span>}
            {memory.profile_id && <span>{memory.profile_id}</span>}
          </div>
        </div>
      </div>
    </div>
  );
}

function sourceLabel(source: UnifiedMemoryRecord["source"]) {
  switch (source) {
    case "wiki_memory":
      return "背景";
    case "memory_fact":
      return "事实";
    case "continuity_experience":
      return "经验";
  }
}

function kindLabel(kind: UnifiedMemoryRecord["kind"]) {
  switch (kind) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目事实";
    case "decision":
      return "决策";
    case "task_state":
      return "进度";
    case "lesson":
      return "经验";
    case "bug_pattern":
      return "Bug 模式";
    case "workflow":
      return "流程";
  }
}

function statusLabel(status: UnifiedMemoryRecord["status"]) {
  switch (status) {
    case "candidate":
      return "候选";
    case "accepted":
      return "已接受";
    case "pinned":
      return "已置顶";
    case "forgotten":
      return "已忘记";
    case "archived":
      return "已归档";
  }
}
```

- [ ] **Step 2: Add the unified read section in project档案**

Modify `apps/desktop/src/components/context/WikiSectionsView.tsx`:

```tsx
import { UnifiedMemorySection } from "./UnifiedMemorySection";
```

Add the unified section directly above the existing source-specific memory management sections:

```tsx
<UnifiedMemorySection currentProjectPath={currentProjectPath} sessionId={sessionId} />
```

Keep `PendingUpdatesSection`, `SavedBackgroundSection`, and `ContinuityExperiencesSection` in Phase 1 so accept/pin/forget/archive controls remain available while the unified read model proves stable. Phase 2 removes duplicate surfaces after unified source-specific actions are implemented.

- [ ] **Step 3: Run frontend build**

Run:

```bash
cd apps/desktop
npm run build
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/context/UnifiedMemorySection.tsx apps/desktop/src/components/context/WikiSectionsView.tsx
git commit -m "feat(desktop): show unified project memory"
```

---

### Task 5: Route Send Input Through Unified Memory Selection

**Files:**
- Modify: `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
- Modify: `apps/desktop/src-tauri/src/ipc/send_input_context_tests.rs`

- [ ] **Step 1: Write failing send-input tests**

Add a test to `apps/desktop/src-tauri/src/ipc/send_input_context_tests.rs`:

```rust
#[tokio::test]
async fn send_input_uses_unified_memory_context_for_wiki_fact_and_continuity() {
    let nonce = uuid::Uuid::now_v7();
    let workspace = std::env::temp_dir().join(format!("forge-send-unified-memory-{nonce}"));
    std::fs::create_dir_all(&workspace).expect("workspace");
    let session = test_agent_session("session-1", &workspace);
    let input_intent = build_turn_input_intent("继续权限确认卡片", &[], Vec::new());
    let workflow = classify_workflow_with_command("session-1", "继续权限确认卡片", None, 1);

    let prepared = prepare_send_input_turn_context(PrepareSendInputTurnRequest {
        session_id: "session-1",
        session: &session,
        text: "继续权限确认卡片",
        input_intent,
        workflow: &workflow,
        ready_connector_labels: Vec::new(),
        memory_context: Some(
            "## Work Memory\n- [wiki_memory/task_state] title=\"权限进度\" body=\"完全访问按钮已完成\" reason=\"同一项目\""
                .to_string(),
        ),
        wiki_context: None,
        continuity_context: None,
        connector_context: None,
    })
    .await;

    let memory = prepared
        .hidden_contexts
        .iter()
        .find(|context| context.kind == ContextSourceKind::MemoryContext)
        .expect("memory context");
    assert!(memory.content.contains("Work Memory"));
    assert!(memory.content.contains("权限进度"));

    let _ = std::fs::remove_dir_all(&workspace);
}
```

- [ ] **Step 2: Run the targeted test**

Run:

```bash
cd apps/desktop/src-tauri
cargo test ipc::send_input_context_tests::send_input_uses_unified_memory_context_for_wiki_fact_and_continuity --lib
```

Expected: PASS once the test compiles. If it fails because of helper visibility, adjust imports in the test module only.

- [ ] **Step 3: Replace separate memory and continuity selection in context preparation**

Modify `apps/desktop/src-tauri/src/ipc/send_input_context.rs` so `build_send_input_contexts` uses:

```rust
let unified_memory_context = match crate::ipc::unified_memory::select_unified_memories_for_send_input(
    state,
    text,
    project_path,
)
.await
{
    Ok(selected) => {
        if !selected.is_empty() {
            let ids = selected
                .iter()
                .map(|selection| selection.record.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            crate::app_log!("INFO", "[memory] unified recalled records: {}", ids);
        }
        crate::memory::unified::format_unified_memory_context(&selected)
    }
    Err(error) => {
        crate::app_log!("WARN", "[memory] unified recall failed: {}", error);
        None
    }
};
```

Then pass `unified_memory_context` into `PrepareSendInputTurnRequest.memory_context` and stop passing a separate `continuity_context` from this call path. Keep the `continuity_context` field in the struct for compatibility with existing tests and future manual injection, but normal send-input should not build a second continuity context.

- [ ] **Step 4: Keep compatibility tests**

Run existing tests that protect the old behavior:

```bash
cd apps/desktop/src-tauri
cargo test ipc::send_input_context_tests::send_input_turn_context_includes_continuity_experience_context --lib
cargo test ipc::send_input_context_tests::send_input_memory_selection_includes_active_profile_and_global_facts --lib
```

Expected: PASS. The first test still validates manual continuity hidden context support. The second validates active-profile fact selection.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/ipc/send_input_context.rs apps/desktop/src-tauri/src/ipc/send_input_context_tests.rs
git commit -m "feat(desktop): use unified memory for send input"
```

---

### Task 6: Acceptance, Documentation, and Cleanup

**Files:**
- Modify: `apps/desktop/e2e/acceptance.spec.ts`
- Modify: `README.md`
- Modify: `apps/desktop/README.md`
- Modify: `CHANGELOG.md`
- Optional after UI replacement is stable: remove unused imports of `ContinuityExperiencesSection` only if no references remain.

- [ ] **Step 1: Add acceptance smoke**

In `apps/desktop/e2e/acceptance.spec.ts`, add a mocked Tauri IPC response for `list_unified_memories` with one `wiki_memory`, one `memory_fact`, and one `continuity_experience`. Add a test that opens project档案 and asserts:

```ts
await expect(page.getByText("记忆")).toBeVisible();
await expect(page.getByText("权限进度")).toBeVisible();
await expect(page.getByText("完全访问模式用于测试权限绕过")).toBeVisible();
await expect(page.getByText("确认卡片回归")).toBeVisible();
```

Also assert search filters the visible list:

```ts
await page.getByPlaceholder("搜索记忆、经验、背景").fill("确认卡片");
await expect(page.getByText("确认卡片回归")).toBeVisible();
await expect(page.getByText("权限进度")).not.toBeVisible();
```

- [ ] **Step 2: Run acceptance dry-run and desktop build**

Run:

```bash
scripts/acceptance.sh --dry-run
cd apps/desktop
npm run build
cd src-tauri
cargo test memory::unified ipc::unified_memory ipc::send_input_context_tests --lib
```

Expected:

- `scripts/acceptance.sh --dry-run` lists the desktop smoke coverage including Settings/context surfaces.
- `npm run build` passes.
- Rust targeted tests pass.

- [ ] **Step 3: Update docs**

Update `README.md` Settings bullet to mention unified memory surface:

```markdown
- Settings: models with config-defined provider profiles surfaced in the desktop catalog, compact provider metadata rendering, current Kimi/GLM coding defaults, custom provider profile templates and add/edit/delete, no-auth local provider support, provider-aware start readiness with dated cached evidence checks and targeted Settings recovery, manual provider compatibility probes with persisted redacted evidence and summary, live/static model catalog refresh, workspace, tools, unified memory, data, diagnostics, scheduler, and general service/autostart surfaces.
```

Update `apps/desktop/README.md` around the memory/context section:

```markdown
- 在项目档案中统一查看项目背景、用户事实和任务经验；每条记忆保留来源、状态、置信度和会话出处。
- 每轮请求通过统一记忆召回链路带入相关背景，候选经验不会自动注入，已接受和已置顶记录优先使用。
```

Update `CHANGELOG.md`:

```markdown
- Unified the desktop project memory surface across saved background, profile facts, and continuity experiences, with one recall path for send-input hidden context.
```

- [ ] **Step 4: Final verification**

Run:

```bash
git status --short
cd apps/desktop/src-tauri
cargo test memory::unified ipc::unified_memory ipc::send_input_context_tests --lib
cd ..
npm run build
```

Expected:

- Only intended memory/UI/docs files are changed.
- Targeted Rust tests pass.
- Desktop frontend build passes.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/e2e/acceptance.spec.ts README.md apps/desktop/README.md CHANGELOG.md
git commit -m "docs(desktop): document unified memory surface"
```

---

## Acceptance Points

- Project档案 has one visible "记忆" section that includes:
  - accepted/pinned `WikiMemory` project background;
  - active-profile and global `MemoryFact` records;
  - project `ExperienceMemory` continuity experiences.
- Other-profile facts do not appear in the active profile.
- Forgotten and archived records do not appear in the unified list by default.
- Candidate continuity experiences are visible as candidates in the unified project read list but are not injected into send-input hidden context.
- Send-input hidden context uses one `## Work Memory` block rather than separate memory and continuity blocks.
- The unified context block includes source/kind labels and reasons, but instructs the model not to expose retrieval internals to the user.
- Existing Settings > Memory fact CRUD still works.
- Existing wiki and continuity status controls remain available in their source-specific sections during Phase 1; no data is migrated or deleted.
- `cargo test memory::unified ipc::unified_memory ipc::send_input_context_tests --lib` passes.
- `cd apps/desktop && npm run build` passes.
- `scripts/acceptance.sh --dry-run` remains aligned with acceptance specs.

## Explicit Non-Goals

- No embeddings in Phase 1.
- No physical storage migration in Phase 1.
- No gateway-owned memory worker in Phase 1.
- No removal of legacy IPC commands in Phase 1.
- No hidden automatic promotion of candidate memories.

## Follow-Up Phase 2

After Phase 1 lands and proves stable:

1. Move status actions into `UnifiedMemorySection` for all source types:
   - wiki memory: accept, pin, forget;
   - continuity experience: accept, pin, archive, forget;
   - memory fact: edit, delete.
2. Add a "本轮带入" audit drawer in the composer showing selected unified records, reasons, and scores.
3. Add a deterministic memory-recall eval suite with cases for:
   - project isolation;
   - profile isolation;
   - accepted/pinned priority;
   - candidate suppression;
   - low-signal query suppression;
   - conflict between old memory and visible conversation.
4. Decide whether to migrate storage to one SQLite-backed `memory_records` table.
5. Add embeddings only after the deterministic selector has precision/false-positive metrics.

## Self-Review

- Spec coverage: The plan covers product overview unification, IPC unification, send-input recall unification, docs, and acceptance checks.
- Placeholder scan: No task uses placeholder markers or unspecified edge handling.
- Type consistency: Rust `UnifiedMemoryRecord` and TypeScript `UnifiedMemoryRecord` fields match by name and snake_case serialization.
- Migration safety: Existing stores and legacy commands remain intact, so rollback is possible by reverting the unified selector/UI changes.
