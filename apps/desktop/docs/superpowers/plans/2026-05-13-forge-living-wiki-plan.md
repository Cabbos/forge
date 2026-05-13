# Forge Living Wiki Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Forge's first Progressive Living Wiki: local structured memory, transparent selected-context display, project Wiki sections, and safe edit/pin/forget actions.

**Architecture:** Add a Rust `memory` module for models, local JSON persistence, risk filtering, candidate extraction, and lightweight relevance scoring. Wire it into `send_input` so relevant memories are selected before the agent call and low-risk candidates are extracted after successful turns. Mirror memory events in TypeScript and render them inside the existing right-side `上下文` panel without replacing the lightweight project status card.

**Tech Stack:** Tauri 2, Rust, serde/serde_json, tokio `RwLock`, React 18, TypeScript, Zustand, Tailwind CSS, Lucide React

---

## File Structure

| Action | File | Purpose |
|---|---|---|
| Create | `src-tauri/src/memory/mod.rs` | Public memory module exports and context formatting helper |
| Create | `src-tauri/src/memory/model.rs` | Shared Wiki memory enums and structs |
| Create | `src-tauri/src/memory/risk.rs` | Sensitive-content classifier |
| Create | `src-tauri/src/memory/scoring.rs` | Lightweight relevance scoring and selection |
| Create | `src-tauri/src/memory/storage.rs` | Local JSON store under `~/.tui-to-gui/wiki_memories.json` |
| Create | `src-tauri/src/memory/extraction.rs` | First-version rule-based candidate extraction |
| Create | `src-tauri/src/ipc/memory_handlers.rs` | Tauri commands for list/update/pin/forget/select |
| Modify | `src-tauri/src/lib.rs` | Register memory module and IPC commands |
| Modify | `src-tauri/src/state.rs` | Store shared `WikiMemoryStore` in `AppState` |
| Modify | `src-tauri/src/protocol/events.rs` | Add memory stream events |
| Modify | `src-tauri/src/agent/session.rs` | Inject selected context without mutating visible user text |
| Modify | `src-tauri/src/ipc/handlers.rs` | Select memories before agent call and extract candidates after success |
| Modify | `src/lib/protocol.ts` | Mirror memory event and type definitions |
| Modify | `src/lib/tauri.ts` | Add memory IPC wrappers and TS models |
| Modify | `src/store/index.ts` | Track selected memories and memory list |
| Create | `src/components/context/WikiSections.tsx` | Related background and Project Wiki UI sections |
| Modify | `src/components/layout/HubPanel.tsx` | Place Wiki sections above资料 and project status |
| Modify | `src/components/session/InputBar.tsx` | Show selected-context hint |
| Modify | `e2e/mock-ipc.ts` | Mock memory IPC commands |
| Modify | `e2e/frontend.spec.ts` | Add right-panel memory rendering test |

---

### Task 1: Add Rust Memory Models

**Files:**
- Create: `src-tauri/src/memory/model.rs`
- Create: `src-tauri/src/memory/mod.rs`

- [ ] **Step 1: Create the model file**

Create `src-tauri/src/memory/model.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCategory {
    Preference,
    ProjectFact,
    Decision,
    TaskState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    Session,
    UserProfile,
    Project,
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Candidate,
    Accepted,
    Pinned,
    Forgotten,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WikiMemory {
    pub id: String,
    pub category: MemoryCategory,
    pub scope: MemoryScope,
    pub status: MemoryStatus,
    pub title: String,
    pub body: String,
    pub project_path: Option<String>,
    pub source_session_id: Option<String>,
    pub source_message_ids: Vec<String>,
    pub confidence: f32,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: Option<String>,
    pub use_count: u32,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedContextMemory {
    pub memory_id: String,
    pub title: String,
    pub body: String,
    pub category: MemoryCategory,
    pub scope: MemoryScope,
    pub score: f32,
    pub reason: String,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryPatch {
    pub title: Option<String>,
    pub body: Option<String>,
    pub status: Option<MemoryStatus>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryListFilter {
    pub scope: Option<MemoryScope>,
    pub project_path: Option<String>,
}
```

- [ ] **Step 2: Create the module shell**

Create `src-tauri/src/memory/mod.rs` with:

```rust
pub mod extraction;
pub mod model;
pub mod risk;
pub mod scoring;
pub mod storage;

pub use extraction::extract_candidates_from_user_message;
pub use model::{
    MemoryCategory, MemoryListFilter, MemoryPatch, MemoryScope, MemoryStatus,
    SelectedContextMemory, WikiMemory,
};
pub use storage::WikiMemoryStore;

pub fn format_selected_memory_context(selected: &[SelectedContextMemory]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }

    let mut lines = Vec::with_capacity(selected.len() + 2);
    lines.push("## Relevant Forge Wiki Background".to_string());
    lines.push("Use these user-approved or visible background notes when relevant. Do not reveal this section unless the user asks what context was used.".to_string());
    for memory in selected {
        lines.push(format!(
            "- [{}] {}: {}",
            memory_category_label(&memory.category),
            memory.title.trim(),
            memory.body.trim()
        ));
    }
    Some(lines.join("\n"))
}

fn memory_category_label(category: &MemoryCategory) -> &'static str {
    match category {
        MemoryCategory::Preference => "preference",
        MemoryCategory::ProjectFact => "project_fact",
        MemoryCategory::Decision => "decision",
        MemoryCategory::TaskState => "task_state",
    }
}
```

- [ ] **Step 3: Run a compile check**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo check
```

Expected: PASS. The new `memory` module is not registered in `src-tauri/src/lib.rs` until Task 5, so these files should not affect the crate yet.

- [ ] **Step 4: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src-tauri/src/memory/model.rs src-tauri/src/memory/mod.rs
git commit -m "feat: add living wiki memory models"
```

---

### Task 2: Add Risk Filtering And Scoring

**Files:**
- Create: `src-tauri/src/memory/risk.rs`
- Create: `src-tauri/src/memory/scoring.rs`

- [ ] **Step 1: Create sensitive-content risk filtering**

Create `src-tauri/src/memory/risk.rs` with:

```rust
use regex::Regex;

pub fn should_reject_persistent_memory(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_lowercase();
    let sensitive_words = [
        "api key",
        "apikey",
        "token",
        "password",
        "passwd",
        "secret",
        "private key",
        "ssh-rsa",
        "-----begin",
        "credit card",
        "身份证",
        "客户名单",
        "客户资料",
        "商业机密",
    ];

    if sensitive_words.iter().any(|word| lower.contains(word)) {
        return true;
    }

    let patterns = [
        r"sk-[A-Za-z0-9_\-]{16,}",
        r"ghp_[A-Za-z0-9_]{16,}",
        r"AIza[0-9A-Za-z_\-]{20,}",
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN [A-Z ]+PRIVATE KEY-----",
    ];

    patterns.iter().any(|pattern| {
        Regex::new(pattern)
            .map(|regex| regex.is_match(trimmed))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::should_reject_persistent_memory;

    #[test]
    fn rejects_api_keys_and_tokens() {
        assert!(should_reject_persistent_memory("my API key is sk-1234567890abcdefghijkl"));
        assert!(should_reject_persistent_memory("token: ghp_1234567890abcdefghijkl"));
        assert!(should_reject_persistent_memory("-----BEGIN OPENSSH PRIVATE KEY-----"));
    }

    #[test]
    fn allows_low_risk_preferences() {
        assert!(!should_reject_persistent_memory("以后都用中文和我交流"));
        assert!(!should_reject_persistent_memory("这个项目方向是小白优先，开发者也舒服"));
    }
}
```

- [ ] **Step 2: Create relevance scoring**

Create `src-tauri/src/memory/scoring.rs` with:

```rust
use crate::memory::model::{
    MemoryCategory, MemoryStatus, SelectedContextMemory, WikiMemory,
};

pub fn select_relevant_memories(
    memories: &[WikiMemory],
    message: &str,
    project_path: Option<&str>,
    limit: usize,
) -> Vec<SelectedContextMemory> {
    if limit == 0 {
        return Vec::new();
    }

    let message_terms = terms(message);
    let message_lower = message.to_lowercase();

    let mut scored = memories
        .iter()
        .filter(|memory| {
            memory.status != MemoryStatus::Forgotten
                && memory.status != MemoryStatus::Archived
                && memory.status != MemoryStatus::Candidate
        })
        .filter_map(|memory| {
            let mut score = 0.0_f32;
            let mut reasons = Vec::new();

            if memory.status == MemoryStatus::Pinned {
                score += 4.0;
                reasons.push("已固定");
            }

            if let (Some(active), Some(memory_project)) = (project_path, memory.project_path.as_deref()) {
                if normalize_path(active) == normalize_path(memory_project) {
                    score += 3.0;
                    reasons.push("同一项目");
                }
            }

            let memory_text = format!("{} {} {}", memory.title, memory.body, memory.tags.join(" "));
            let memory_terms = terms(&memory_text);
            let overlap = message_terms
                .iter()
                .filter(|term| memory_terms.contains(term))
                .count();
            if overlap > 0 {
                score += overlap as f32;
                reasons.push("关键词匹配");
            }

            if matches!(memory.category, MemoryCategory::Preference) && contains_preference_signal(&message_lower) {
                score += 1.5;
                reasons.push("偏好相关");
            }

            if matches!(memory.category, MemoryCategory::Decision) && contains_direction_signal(&message_lower) {
                score += 1.5;
                reasons.push("方向相关");
            }

            if memory.use_count > 0 {
                score += (memory.use_count.min(3) as f32) * 0.4;
                reasons.push("曾被使用");
            }

            if score <= 0.0 {
                return None;
            }

            Some(SelectedContextMemory {
                memory_id: memory.id.clone(),
                title: memory.title.clone(),
                body: memory.body.clone(),
                category: memory.category.clone(),
                scope: memory.scope.clone(),
                score,
                reason: reasons.join("、"),
                injected: true,
            })
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.title.cmp(&b.title))
    });
    scored.truncate(limit);
    scored
}

fn contains_preference_signal(message: &str) -> bool {
    ["偏好", "习惯", "还是", "不用", "以后", "按我"].iter().any(|signal| message.contains(signal))
}

fn contains_direction_signal(message: &str) -> bool {
    ["方向", "方案", "之前", "继续", "按之前", "产品"].iter().any(|signal| message.contains(signal))
}

fn terms(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
        .filter_map(|term| {
            let term = term.trim();
            if term.len() < 2 {
                None
            } else {
                Some(term.to_string())
            }
        })
        .collect()
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::select_relevant_memories;
    use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};

    fn memory(id: &str, status: MemoryStatus, body: &str) -> WikiMemory {
        WikiMemory {
            id: id.to_string(),
            category: MemoryCategory::Decision,
            scope: MemoryScope::Project,
            status,
            title: "Forge 方向".to_string(),
            body: body.to_string(),
            project_path: Some("/tmp/forge".to_string()),
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.8,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["forge".to_string(), "资料系统".to_string()],
        }
    }

    #[test]
    fn selects_pinned_same_project_memory() {
        let memories = vec![memory("m1", MemoryStatus::Pinned, "渐进式 Living Wiki")];
        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].memory_id, "m1");
        assert!(selected[0].reason.contains("已固定"));
    }

    #[test]
    fn excludes_forgotten_archived_and_candidates() {
        let memories = vec![
            memory("forgotten", MemoryStatus::Forgotten, "资料系统"),
            memory("archived", MemoryStatus::Archived, "资料系统"),
            memory("candidate", MemoryStatus::Candidate, "资料系统"),
        ];
        let selected = select_relevant_memories(&memories, "继续做资料系统", Some("/tmp/forge"), 5);
        assert!(selected.is_empty());
    }
}
```

- [ ] **Step 3: Run focused Rust tests**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo test memory::risk && cargo test memory::scoring
```

Expected: PASS for `rejects_api_keys_and_tokens`, `allows_low_risk_preferences`, `selects_pinned_same_project_memory`, and `excludes_forgotten_archived_and_candidates`.

- [ ] **Step 4: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src-tauri/src/memory/risk.rs src-tauri/src/memory/scoring.rs
git commit -m "feat: score living wiki memories"
```

---

### Task 3: Add Local Memory Storage

**Files:**
- Create: `src-tauri/src/memory/storage.rs`

- [ ] **Step 1: Create the JSON-backed store**

Create `src-tauri/src/memory/storage.rs` with:

```rust
use std::fs;
use std::path::PathBuf;

use tokio::sync::RwLock;

use crate::memory::model::{
    MemoryListFilter, MemoryPatch, MemoryScope, MemoryStatus, SelectedContextMemory, WikiMemory,
};
use crate::memory::scoring::select_relevant_memories;

pub struct WikiMemoryStore {
    path: PathBuf,
    memories: RwLock<Vec<WikiMemory>>,
}

impl WikiMemoryStore {
    pub fn default() -> Self {
        Self::new(default_memory_path())
    }

    pub fn new(path: PathBuf) -> Self {
        let memories = load_memories(&path);
        Self {
            path,
            memories: RwLock::new(memories),
        }
    }

    pub async fn list(&self, filter: MemoryListFilter) -> Vec<WikiMemory> {
        let memories = self.memories.read().await;
        memories
            .iter()
            .filter(|memory| memory.status != MemoryStatus::Forgotten)
            .filter(|memory| memory.status != MemoryStatus::Archived)
            .filter(|memory| {
                filter
                    .scope
                    .as_ref()
                    .map(|scope| &memory.scope == scope)
                    .unwrap_or(true)
            })
            .filter(|memory| match (&filter.project_path, &memory.project_path) {
                (Some(active), Some(path)) => normalize_path(active) == normalize_path(path),
                (Some(_), None) => memory.scope == MemoryScope::UserProfile,
                (None, _) => true,
            })
            .cloned()
            .collect()
    }

    pub async fn upsert_candidate(&self, candidate: WikiMemory) -> Result<Option<WikiMemory>, String> {
        let mut memories = self.memories.write().await;
        if memories.iter().any(|memory| {
            memory.status == MemoryStatus::Forgotten
                && memory.title == candidate.title
                && memory.body == candidate.body
        }) {
            return Ok(None);
        }

        if let Some(existing) = memories.iter_mut().find(|memory| {
            memory.status != MemoryStatus::Forgotten
                && memory.category == candidate.category
                && memory.scope == candidate.scope
                && memory.project_path == candidate.project_path
                && memory.title == candidate.title
        }) {
            existing.body = candidate.body;
            existing.confidence = existing.confidence.max(candidate.confidence);
            existing.updated_at = candidate.updated_at;
            let updated = existing.clone();
            save_memories(&self.path, &memories)?;
            return Ok(Some(updated));
        }

        memories.push(candidate.clone());
        save_memories(&self.path, &memories)?;
        Ok(Some(candidate))
    }

    pub async fn update(&self, memory_id: &str, patch: MemoryPatch) -> Result<WikiMemory, String> {
        let mut memories = self.memories.write().await;
        let memory = memories
            .iter_mut()
            .find(|memory| memory.id == memory_id)
            .ok_or_else(|| format!("Memory not found: {memory_id}"))?;

        if let Some(title) = patch.title {
            memory.title = title.trim().to_string();
        }
        if let Some(body) = patch.body {
            memory.body = body.trim().to_string();
        }
        if let Some(status) = patch.status {
            memory.status = status;
        }
        if let Some(tags) = patch.tags {
            memory.tags = tags;
        }
        memory.updated_at = now_string();

        let updated = memory.clone();
        save_memories(&self.path, &memories)?;
        Ok(updated)
    }

    pub async fn pin(&self, memory_id: &str) -> Result<WikiMemory, String> {
        self.update(
            memory_id,
            MemoryPatch {
                status: Some(MemoryStatus::Pinned),
                ..MemoryPatch::default()
            },
        )
        .await
    }

    pub async fn forget(&self, memory_id: &str) -> Result<WikiMemory, String> {
        self.update(
            memory_id,
            MemoryPatch {
                status: Some(MemoryStatus::Forgotten),
                ..MemoryPatch::default()
            },
        )
        .await
    }

    pub async fn select(
        &self,
        message: &str,
        project_path: Option<&str>,
        limit: usize,
    ) -> Vec<SelectedContextMemory> {
        let selected = {
            let memories = self.memories.read().await;
            select_relevant_memories(&memories, message, project_path, limit)
        };

        if !selected.is_empty() {
            let selected_ids = selected
                .iter()
                .map(|memory| memory.memory_id.as_str())
                .collect::<Vec<_>>();
            let mut memories = self.memories.write().await;
            let now = now_string();
            for memory in memories.iter_mut() {
                if selected_ids.iter().any(|id| *id == memory.id) {
                    memory.use_count = memory.use_count.saturating_add(1);
                    memory.last_used_at = Some(now.clone());
                    memory.updated_at = now.clone();
                }
            }
            if let Err(error) = save_memories(&self.path, &memories) {
                crate::app_log!("WARN", "[living_wiki] failed to update usage: {}", error);
            }
        }

        selected
    }
}

fn load_memories(path: &PathBuf) -> Vec<WikiMemory> {
    if !path.exists() {
        return Vec::new();
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str::<Vec<WikiMemory>>(&content).ok())
        .unwrap_or_default()
}

fn save_memories(path: &PathBuf, memories: &[WikiMemory]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create memory directory: {error}"))?;
    }
    let json = serde_json::to_string_pretty(memories)
        .map_err(|error| format!("Failed to serialize memories: {error}"))?;
    fs::write(path, json).map_err(|error| format!("Failed to write memories: {error}"))
}

pub fn now_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn default_memory_path() -> PathBuf {
    home_dir().join(".tui-to-gui").join("wiki_memories.json")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn normalize_path(path: &str) -> String {
    path.trim().trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::{now_string, WikiMemoryStore};
    use crate::memory::model::{
        MemoryCategory, MemoryListFilter, MemoryScope, MemoryStatus, WikiMemory,
    };

    fn sample_memory(id: &str) -> WikiMemory {
        WikiMemory {
            id: id.to_string(),
            category: MemoryCategory::Preference,
            scope: MemoryScope::UserProfile,
            status: MemoryStatus::Accepted,
            title: "交流语言".to_string(),
            body: "以后都用中文交流".to_string(),
            project_path: None,
            source_session_id: Some("session-1".to_string()),
            source_message_ids: Vec::new(),
            confidence: 0.9,
            created_at: now_string(),
            updated_at: now_string(),
            last_used_at: None,
            use_count: 0,
            tags: vec!["中文".to_string()],
        }
    }

    #[tokio::test]
    async fn persists_and_loads_memories() {
        let path = std::env::temp_dir().join(format!("forge-memory-{}.json", uuid::Uuid::now_v7()));
        let store = WikiMemoryStore::new(path.clone());
        store.upsert_candidate(sample_memory("m1")).await.unwrap();

        let reloaded = WikiMemoryStore::new(path.clone());
        let memories = reloaded.list(MemoryListFilter::default()).await;
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].title, "交流语言");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn forget_hides_memory_from_list() {
        let path = std::env::temp_dir().join(format!("forge-memory-{}.json", uuid::Uuid::now_v7()));
        let store = WikiMemoryStore::new(path.clone());
        store.upsert_candidate(sample_memory("m1")).await.unwrap();
        store.forget("m1").await.unwrap();

        let memories = store.list(MemoryListFilter::default()).await;
        assert!(memories.is_empty());

        let _ = std::fs::remove_file(path);
    }
}
```

- [ ] **Step 2: Run focused storage tests**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo test memory::storage
```

Expected: PASS for `persists_and_loads_memories` and `forget_hides_memory_from_list`.

- [ ] **Step 3: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src-tauri/src/memory/storage.rs
git commit -m "feat: persist living wiki memories"
```

---

### Task 4: Add Candidate Extraction

**Files:**
- Create: `src-tauri/src/memory/extraction.rs`

- [ ] **Step 1: Create rule-based extraction**

Create `src-tauri/src/memory/extraction.rs` with:

```rust
use uuid::Uuid;

use crate::memory::model::{MemoryCategory, MemoryScope, MemoryStatus, WikiMemory};
use crate::memory::risk::should_reject_persistent_memory;
use crate::memory::storage::now_string;

pub fn extract_candidates_from_user_message(
    session_id: &str,
    project_path: Option<&str>,
    text: &str,
) -> Vec<WikiMemory> {
    let normalized = collapse_whitespace(text);
    if normalized.chars().count() < 8 || should_reject_persistent_memory(&normalized) {
        return Vec::new();
    }

    let mut memories = Vec::new();

    if is_preference(&normalized) {
        memories.push(new_memory(
            MemoryCategory::Preference,
            MemoryScope::UserProfile,
            "用户偏好",
            &normalized,
            None,
            session_id,
            0.72,
            vec!["preference".to_string()],
        ));
    }

    if is_project_decision(&normalized) {
        memories.push(new_memory(
            MemoryCategory::Decision,
            MemoryScope::Project,
            "项目已定方案",
            &normalized,
            project_path.map(str::to_string),
            session_id,
            0.68,
            vec!["decision".to_string()],
        ));
    }

    if is_task_state(&normalized) {
        memories.push(new_memory(
            MemoryCategory::TaskState,
            MemoryScope::Project,
            "当前进度",
            &normalized,
            project_path.map(str::to_string),
            session_id,
            0.6,
            vec!["task_state".to_string()],
        ));
    }

    memories
}

fn new_memory(
    category: MemoryCategory,
    scope: MemoryScope,
    title: &str,
    body: &str,
    project_path: Option<String>,
    session_id: &str,
    confidence: f32,
    tags: Vec<String>,
) -> WikiMemory {
    let now = now_string();
    WikiMemory {
        id: Uuid::now_v7().to_string(),
        category,
        scope,
        status: MemoryStatus::Candidate,
        title: title.to_string(),
        body: truncate(body, 360),
        project_path,
        source_session_id: Some(session_id.to_string()),
        source_message_ids: Vec::new(),
        confidence,
        created_at: now.clone(),
        updated_at: now,
        last_used_at: None,
        use_count: 0,
        tags,
    }
}

fn is_preference(text: &str) -> bool {
    ["以后", "我希望", "我喜欢", "不用验证", "我来验证", "默认", "优先"].iter().any(|signal| text.contains(signal))
}

fn is_project_decision(text: &str) -> bool {
    ["方向", "定", "选择", "就用", "方案", "产品", "兼容"].iter().any(|signal| text.contains(signal))
}

fn is_task_state(text: &str) -> bool {
    ["继续", "接下来", "已经", "做到", "先把", "下一步"].iter().any(|signal| text.contains(signal))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output = text.chars().take(max_chars.saturating_sub(3)).collect::<String>();
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::extract_candidates_from_user_message;
    use crate::memory::model::{MemoryCategory, MemoryScope};

    #[test]
    fn extracts_low_risk_preference() {
        let memories = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/forge"),
            "以后都用中文和我交流，我自己来验证",
        );
        assert!(memories.iter().any(|memory| memory.category == MemoryCategory::Preference));
        assert!(memories.iter().any(|memory| memory.scope == MemoryScope::UserProfile));
    }

    #[test]
    fn rejects_secret_like_content() {
        let memories = extract_candidates_from_user_message(
            "session-1",
            Some("/tmp/forge"),
            "这个 API key 是 sk-1234567890abcdefghijkl，帮我记住",
        );
        assert!(memories.is_empty());
    }
}
```

- [ ] **Step 2: Run extraction tests**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo test memory::extraction
```

Expected: PASS for `extracts_low_risk_preference` and `rejects_secret_like_content`.

- [ ] **Step 3: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src-tauri/src/memory/extraction.rs
git commit -m "feat: extract living wiki candidates"
```

---

### Task 5: Wire Backend State, Events, IPC, And Agent Injection

**Files:**
- Create: `src-tauri/src/ipc/memory_handlers.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/ipc/mod.rs`
- Modify: `src-tauri/src/protocol/events.rs`
- Modify: `src-tauri/src/agent/session.rs`
- Modify: `src-tauri/src/ipc/handlers.rs`

- [ ] **Step 1: Add the store to app state**

Modify `src-tauri/src/state.rs`.

Add this import:

```rust
use crate::memory::WikiMemoryStore;
```

Add this field to `AppState`:

```rust
pub wiki_memory: Arc<WikiMemoryStore>,
```

Initialize it in `AppState::new`:

```rust
wiki_memory: Arc::new(WikiMemoryStore::default()),
```

- [ ] **Step 2: Register the memory module and commands**

Modify `src-tauri/src/lib.rs`.

Add the module:

```rust
mod memory;
```

Add these commands inside the existing `tauri::generate_handler!` command list:

```rust
ipc::memory_handlers::list_memories,
ipc::memory_handlers::update_memory,
ipc::memory_handlers::forget_memory,
ipc::memory_handlers::pin_memory,
ipc::memory_handlers::select_context_memories,
```

Modify `src-tauri/src/ipc/mod.rs` and add:

```rust
pub mod memory_handlers;
```

- [ ] **Step 3: Add memory stream events**

Modify `src-tauri/src/protocol/events.rs`.

Add this import near the top:

```rust
use crate::memory::{SelectedContextMemory, WikiMemory};
```

Add these enum variants before `SessionStarted`:

```rust
    // ── Living Wiki ──
    #[serde(rename = "memory_selection")]
    MemorySelection {
        session_id: String,
        selected: Vec<SelectedContextMemory>,
    },
    #[serde(rename = "memory_candidate")]
    MemoryCandidate {
        session_id: String,
        memory: WikiMemory,
    },
    #[serde(rename = "memory_updated")]
    MemoryUpdated {
        session_id: String,
        memory: WikiMemory,
    },
```

Add the new variants to `session_id()`:

```rust
            | MemorySelection { session_id, .. }
            | MemoryCandidate { session_id, .. }
            | MemoryUpdated { session_id, .. }
```

- [ ] **Step 4: Add memory IPC handlers**

Create `src-tauri/src/ipc/memory_handlers.rs` with:

```rust
use std::sync::Arc;

use tauri::Emitter;

use crate::memory::{MemoryListFilter, MemoryPatch, MemoryScope, WikiMemory};
use crate::protocol::events::StreamEvent;
use crate::state::AppState;

#[tauri::command]
pub async fn list_memories(
    state: tauri::State<'_, Arc<AppState>>,
    scope: Option<String>,
    project_path: Option<String>,
) -> Result<Vec<WikiMemory>, String> {
    let filter = MemoryListFilter {
        scope: parse_scope(scope.as_deref()),
        project_path,
    };
    Ok(state.wiki_memory.list(filter).await)
}

#[tauri::command]
pub async fn update_memory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    memory_id: String,
    patch: MemoryPatch,
) -> Result<WikiMemory, String> {
    let memory = state.wiki_memory.update(&memory_id, patch).await?;
    if let Some(session_id) = session_id {
        let _ = app_handle.emit(
            "session-output",
            StreamEvent::MemoryUpdated {
                session_id,
                memory: memory.clone(),
            },
        );
    }
    Ok(memory)
}

#[tauri::command]
pub async fn forget_memory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    memory_id: String,
) -> Result<WikiMemory, String> {
    let memory = state.wiki_memory.forget(&memory_id).await?;
    if let Some(session_id) = session_id {
        let _ = app_handle.emit(
            "session-output",
            StreamEvent::MemoryUpdated {
                session_id,
                memory: memory.clone(),
            },
        );
    }
    Ok(memory)
}

#[tauri::command]
pub async fn pin_memory(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    session_id: Option<String>,
    memory_id: String,
) -> Result<WikiMemory, String> {
    let memory = state.wiki_memory.pin(&memory_id).await?;
    if let Some(session_id) = session_id {
        let _ = app_handle.emit(
            "session-output",
            StreamEvent::MemoryUpdated {
                session_id,
                memory: memory.clone(),
            },
        );
    }
    Ok(memory)
}

#[tauri::command]
pub async fn select_context_memories(
    state: tauri::State<'_, Arc<AppState>>,
    message: String,
    project_path: Option<String>,
) -> Result<Vec<crate::memory::SelectedContextMemory>, String> {
    Ok(state
        .wiki_memory
        .select(&message, project_path.as_deref(), 8)
        .await)
}

fn parse_scope(scope: Option<&str>) -> Option<MemoryScope> {
    match scope {
        Some("session") => Some(MemoryScope::Session),
        Some("user_profile") => Some(MemoryScope::UserProfile),
        Some("project") => Some(MemoryScope::Project),
        Some("document") => Some(MemoryScope::Document),
        _ => None,
    }
}
```

- [ ] **Step 5: Let `AgentSession` inject selected memories**

Modify `src-tauri/src/agent/session.rs`.

Add a wrapper method above `pub async fn send_message`:

```rust
    pub async fn send_message(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
    ) -> Result<(), String> {
        self.send_message_with_context(text, app_handle, None).await
    }
```

Rename the existing `pub async fn send_message` implementation to:

```rust
    pub async fn send_message_with_context(
        &self,
        text: &str,
        app_handle: &tauri::AppHandle,
        memory_context: Option<String>,
    ) -> Result<(), String> {
```

Inside both message assembly blocks, after inserting the previous summary and before inserting the system prompt, add:

```rust
            if let Some(ref memory_context) = memory_context {
                if !memory_context.trim().is_empty() {
                    msgs_with_context.insert(0, ChatMessage::user(memory_context));
                }
            }
```

For the final text-only summary block, add the same idea using `msgs`:

```rust
            if let Some(ref memory_context) = memory_context {
                if !memory_context.trim().is_empty() {
                    msgs.insert(0, ChatMessage::user(memory_context));
                }
            }
```

- [ ] **Step 6: Select and extract memories in `send_input`**

Modify `src-tauri/src/ipc/handlers.rs`.

Add this import:

```rust
use crate::memory::{extract_candidates_from_user_message, format_selected_memory_context};
```

Replace the body of `send_input`'s `Some(s)` match branch with:

```rust
        Some(s) => {
            let project_path = s.harness.working_dir.to_string_lossy().to_string();
            let selected = state
                .wiki_memory
                .select(&text, Some(&project_path), 8)
                .await;
            let _ = app_handle.emit(
                "session-output",
                StreamEvent::MemorySelection {
                    session_id: session_id.clone(),
                    selected: selected.clone(),
                },
            );

            let memory_context = format_selected_memory_context(&selected);
            let result = s
                .send_message_with_context(&text, &app_handle, memory_context)
                .await;

            if result.is_ok() {
                let candidates = extract_candidates_from_user_message(
                    &session_id,
                    Some(&project_path),
                    &text,
                );
                for candidate in candidates {
                    match state.wiki_memory.upsert_candidate(candidate).await {
                        Ok(Some(memory)) => {
                            let _ = app_handle.emit(
                                "session-output",
                                StreamEvent::MemoryCandidate {
                                    session_id: session_id.clone(),
                                    memory,
                                },
                            );
                        }
                        Ok(None) => {}
                        Err(error) => {
                            crate::app_log!("WARN", "[living_wiki] {}", error);
                        }
                    }
                }
            }

            if let Err(error) = save_session_snapshot(&s.snapshot()) {
                crate::app_log!("WARN", "[session_snapshot] {}", error);
            }
            result
        }
```

- [ ] **Step 7: Run backend tests**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo test memory
```

Expected: PASS for all memory tests.

- [ ] **Step 8: Run backend compile check**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo check
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src-tauri/src/lib.rs src-tauri/src/state.rs src-tauri/src/ipc/mod.rs src-tauri/src/ipc/memory_handlers.rs src-tauri/src/protocol/events.rs src-tauri/src/agent/session.rs src-tauri/src/ipc/handlers.rs
git commit -m "feat: wire living wiki into agent context"
```

---

### Task 6: Mirror Memory Types In TypeScript Store And IPC

**Files:**
- Modify: `src/lib/protocol.ts`
- Modify: `src/lib/tauri.ts`
- Modify: `src/store/index.ts`

- [ ] **Step 1: Add TypeScript memory types and events**

Modify `src/lib/protocol.ts`.

Add these types above `export type StreamEvent`:

```ts
export type MemoryCategory = "preference" | "project_fact" | "decision" | "task_state";
export type MemoryScope = "session" | "user_profile" | "project" | "document";
export type MemoryStatus = "candidate" | "accepted" | "pinned" | "forgotten" | "archived";

export interface WikiMemory {
  id: string;
  category: MemoryCategory;
  scope: MemoryScope;
  status: MemoryStatus;
  title: string;
  body: string;
  project_path?: string | null;
  source_session_id?: string | null;
  source_message_ids: string[];
  confidence: number;
  created_at: string;
  updated_at: string;
  last_used_at?: string | null;
  use_count: number;
  tags: string[];
}

export interface SelectedContextMemory {
  memory_id: string;
  title: string;
  body: string;
  category: MemoryCategory;
  scope: MemoryScope;
  score: number;
  reason: string;
  injected: boolean;
}

export interface MemoryPatch {
  title?: string;
  body?: string;
  status?: MemoryStatus;
  tags?: string[];
}
```

Add these union members before session status events:

```ts
  // ── Living Wiki ──
  | { event_type: "memory_selection"; session_id: string; selected: SelectedContextMemory[] }
  | { event_type: "memory_candidate"; session_id: string; memory: WikiMemory }
  | { event_type: "memory_updated"; session_id: string; memory: WikiMemory }
```

- [ ] **Step 2: Add memory IPC wrappers**

Modify `src/lib/tauri.ts`.

Import memory types:

```ts
import type { MemoryPatch, MemoryScope, SelectedContextMemory, WikiMemory } from "./protocol";
```

Add these functions near other IPC wrappers:

```ts
export async function listMemories(scope?: MemoryScope, projectPath?: string): Promise<WikiMemory[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_memories", { scope: scope ?? null, projectPath: projectPath ?? null });
}

export async function updateMemory(memoryId: string, patch: MemoryPatch, sessionId?: string): Promise<WikiMemory> {
  return invoke("update_memory", { memoryId, patch, sessionId: sessionId ?? null });
}

export async function forgetMemory(memoryId: string, sessionId?: string): Promise<WikiMemory> {
  return invoke("forget_memory", { memoryId, sessionId: sessionId ?? null });
}

export async function pinMemory(memoryId: string, sessionId?: string): Promise<WikiMemory> {
  return invoke("pin_memory", { memoryId, sessionId: sessionId ?? null });
}

export async function selectContextMemories(message: string, projectPath?: string): Promise<SelectedContextMemory[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("select_context_memories", { message, projectPath: projectPath ?? null });
}
```

- [ ] **Step 3: Extend Zustand state**

Modify `src/store/index.ts`.

Change the import to include memory types:

```ts
import type { BlockState, SelectedContextMemory, StreamEvent, SessionState, WikiMemory } from "../lib/protocol";
```

Add these fields to `AppStore`:

```ts
  memories: WikiMemory[];
  selectedContextBySession: Map<string, SelectedContextMemory[]>;
  setMemories: (memories: WikiMemory[]) => void;
  upsertMemory: (memory: WikiMemory) => void;
```

Add initial state:

```ts
  memories: [],
  selectedContextBySession: new Map(),
```

Add actions before `dispatchOutputEvent`:

```ts
  setMemories: (memories) => set({ memories }),

  upsertMemory: (memory) => {
    const current = get().memories.filter((item) => item.id !== memory.id);
    const memories = memory.status === "forgotten" ? current : [memory, ...current];
    set({ memories });
  },
```

Inside `dispatchOutputEvent`, after `let blocks = [...session.blocks];`, add:

```ts
    if (event_type === "memory_selection") {
      const memoryEvent = event as Extract<StreamEvent, { event_type: "memory_selection" }>;
      const selectedContextBySession = new Map(get().selectedContextBySession);
      selectedContextBySession.set(session_id, memoryEvent.selected);
      set({ selectedContextBySession });
      return;
    }

    if (event_type === "memory_candidate" || event_type === "memory_updated") {
      const memoryEvent = event as Extract<StreamEvent, { event_type: "memory_candidate" | "memory_updated" }>;
      get().upsertMemory(memoryEvent.memory);
      return;
    }
```

- [ ] **Step 4: Run frontend type check**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && npx tsc --noEmit
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src/lib/protocol.ts src/lib/tauri.ts src/store/index.ts
git commit -m "feat: mirror living wiki state in frontend"
```

---

### Task 7: Render Living Wiki In The Context Panel

**Files:**
- Create: `src/components/context/WikiSections.tsx`
- Modify: `src/components/layout/HubPanel.tsx`
- Modify: `src/components/session/InputBar.tsx`

- [ ] **Step 1: Create Wiki UI sections**

Create `src/components/context/WikiSections.tsx` with:

```tsx
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { CheckCircle2, Edit3, Pin, PinOff, Sparkles, Trash2 } from "lucide-react";
import { forgetMemory, listMemories, pinMemory, updateMemory } from "@/lib/tauri";
import type { MemoryCategory, SelectedContextMemory, WikiMemory } from "@/lib/protocol";
import { useStore } from "@/store";
import { cn } from "@/lib/utils";

interface WikiSectionsProps {
  sessionId: string | null;
  projectPath: string | null;
}

export function WikiSections({ sessionId, projectPath }: WikiSectionsProps) {
  const memories = useStore((s) => s.memories);
  const setMemories = useStore((s) => s.setMemories);
  const selected = useStore((s) =>
    sessionId ? s.selectedContextBySession.get(sessionId) ?? [] : [],
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const all = await listMemories(undefined, projectPath ?? undefined);
      setMemories(all);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [projectPath, setMemories]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const projectMemories = useMemo(
    () => memories.filter((memory) => memory.scope === "project"),
    [memories],
  );

  return (
    <div className="flex flex-col gap-4">
      <RelatedBackgroundSection
        selected={selected}
        memories={memories}
        sessionId={sessionId}
        onChanged={refresh}
      />
      <ProjectWikiSection
        memories={projectMemories}
        loading={loading}
        error={error}
        sessionId={sessionId}
        onChanged={refresh}
      />
    </div>
  );
}

function RelatedBackgroundSection({
  selected,
  memories,
  sessionId,
  onChanged,
}: {
  selected: SelectedContextMemory[];
  memories: WikiMemory[];
  sessionId: string | null;
  onChanged: () => void;
}) {
  const selectedWithMemory = selected.map((item) => ({
    selected: item,
    memory: memories.find((memory) => memory.id === item.memory_id),
  }));

  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">相关背景</h3>
        {selected.length > 0 && (
          <span className="text-[10px] text-primary">已带入 {selected.length} 条</span>
        )}
      </div>

      <div className="overflow-hidden rounded-md border border-border bg-card">
        {selected.length === 0 ? (
          <EmptyState icon={<Sparkles className="size-4" />} text="没有找到相关背景" />
        ) : (
          <div className="divide-y divide-border">
            {selectedWithMemory.map(({ selected, memory }) => (
              <MemoryRow
                key={selected.memory_id}
                memory={memory}
                fallbackTitle={selected.title}
                fallbackBody={selected.body}
                category={selected.category}
                meta={selected.reason}
                sessionId={sessionId}
                onChanged={onChanged}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function ProjectWikiSection({
  memories,
  loading,
  error,
  sessionId,
  onChanged,
}: {
  memories: WikiMemory[];
  loading: boolean;
  error: string;
  sessionId: string | null;
  onChanged: () => void;
}) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[11px] font-medium text-muted-foreground">项目 Wiki</h3>
        {loading && <span className="text-[10px] text-muted-foreground/70">读取中</span>}
      </div>

      <div className="overflow-hidden rounded-md border border-border bg-card">
        {error ? (
          <div className="px-3 py-2 text-[11px] leading-relaxed text-destructive">{error}</div>
        ) : memories.length === 0 ? (
          <EmptyState icon={<CheckCircle2 className="size-4" />} text="还没有项目 Wiki" />
        ) : (
          <div className="divide-y divide-border">
            {memories.map((memory) => (
              <MemoryRow
                key={memory.id}
                memory={memory}
                category={memory.category}
                meta={memoryStatusLabel(memory)}
                sessionId={sessionId}
                onChanged={onChanged}
              />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function MemoryRow({
  memory,
  fallbackTitle,
  fallbackBody,
  category,
  meta,
  sessionId,
  onChanged,
}: {
  memory?: WikiMemory;
  fallbackTitle?: string;
  fallbackBody?: string;
  category: MemoryCategory;
  meta: string;
  sessionId: string | null;
  onChanged: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(memory?.title ?? fallbackTitle ?? "");
  const [body, setBody] = useState(memory?.body ?? fallbackBody ?? "");
  const canMutate = Boolean(memory);

  const save = async () => {
    if (!memory) return;
    await updateMemory(memory.id, { title, body, status: memory.status === "candidate" ? "accepted" : memory.status }, sessionId ?? undefined);
    setEditing(false);
    onChanged();
  };

  const pin = async () => {
    if (!memory) return;
    await pinMemory(memory.id, sessionId ?? undefined);
    onChanged();
  };

  const forget = async () => {
    if (!memory) return;
    await forgetMemory(memory.id, sessionId ?? undefined);
    onChanged();
  };

  return (
    <div className="px-3 py-2.5">
      {editing ? (
        <div className="space-y-2">
          <input
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            className="h-7 w-full rounded border border-border bg-background px-2 text-xs text-foreground outline-none"
          />
          <textarea
            value={body}
            onChange={(event) => setBody(event.target.value)}
            className="min-h-[64px] w-full resize-none rounded border border-border bg-background px-2 py-1.5 text-xs leading-relaxed text-foreground outline-none"
          />
          <div className="flex justify-end gap-1.5">
            <button className="rounded px-2 py-1 text-[11px] text-muted-foreground hover:text-foreground" onClick={() => setEditing(false)}>取消</button>
            <button className="rounded bg-primary px-2 py-1 text-[11px] text-primary-foreground" onClick={save}>保存</button>
          </div>
        </div>
      ) : (
        <>
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <div className="truncate text-xs font-medium text-foreground">{title}</div>
              <div className="mt-1 line-clamp-3 text-[11px] leading-relaxed text-muted-foreground">{body}</div>
            </div>
            {canMutate && (
              <div className="flex shrink-0 items-center gap-1">
                <IconButton title="编辑" onClick={() => setEditing(true)} icon={<Edit3 className="size-3" />} />
                <IconButton
                  title={memory?.status === "pinned" ? "已固定" : "固定"}
                  onClick={pin}
                  icon={memory?.status === "pinned" ? <PinOff className="size-3" /> : <Pin className="size-3" />}
                />
                <IconButton title="忘记" onClick={forget} icon={<Trash2 className="size-3" />} />
              </div>
            )}
          </div>
          <div className="mt-2 flex min-w-0 items-center justify-between gap-2 text-[10px] text-muted-foreground/70">
            <span>{categoryLabel(category)}</span>
            <span className="truncate text-right">{meta}</span>
          </div>
        </>
      )}
    </div>
  );
}

function IconButton({ title, icon, onClick }: { title: string; icon: ReactNode; onClick: () => void }) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      className="rounded p-1 text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
    >
      {icon}
    </button>
  );
}

function EmptyState({ icon, text }: { icon: ReactNode; text: string }) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 px-3 py-6 text-center text-muted-foreground">
      <div className="text-muted-foreground/60">{icon}</div>
      <div className="text-xs">{text}</div>
    </div>
  );
}

function categoryLabel(category: MemoryCategory) {
  switch (category) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目信息";
    case "decision":
      return "已定方案";
    case "task_state":
      return "当前进度";
  }
}

function memoryStatusLabel(memory: WikiMemory) {
  const labels: Record<WikiMemory["status"], string> = {
    candidate: "候选",
    accepted: "已保存",
    pinned: "已固定",
    forgotten: "已忘记",
    archived: "已归档",
  };
  return cn(labels[memory.status], memory.confidence ? `置信度 ${Math.round(memory.confidence * 100)}%` : "");
}
```

- [ ] **Step 2: Add Wiki sections to the right panel**

Modify `src/components/layout/HubPanel.tsx`.

Add this import:

```tsx
import { WikiSections } from "@/components/context/WikiSections";
```

Compute a project path near the existing session values:

```tsx
const projectPath = getRememberedWorkingDir();
```

Add `WikiSections` before `ContextFilesSection`:

```tsx
<WikiSections sessionId={activeId} projectPath={projectPath} />
```

- [ ] **Step 3: Add selected-context hint to the input bar**

Modify `src/components/session/InputBar.tsx`.

Add this selector near other store selectors:

```tsx
const selectedContextCount = useStore((s) => s.selectedContextBySession.get(sessionId)?.length ?? 0);
```

Add this UI just above the suggestion popup:

```tsx
{selectedContextCount > 0 && (
  <div className="mb-2 inline-flex items-center gap-1.5 rounded-md border border-border bg-card px-2 py-1 text-[11px] text-muted-foreground">
    <Sparkles className="size-3 text-primary" />
    已带入 {selectedContextCount} 条相关背景
  </div>
)}
```

- [ ] **Step 4: Run frontend type check**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && npx tsc --noEmit
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src/components/context/WikiSections.tsx src/components/layout/HubPanel.tsx src/components/session/InputBar.tsx
git commit -m "feat: show living wiki in context panel"
```

---

### Task 8: Add Browser-Level Coverage For Memory UI

**Files:**
- Modify: `e2e/mock-ipc.ts`
- Modify: `e2e/frontend.spec.ts`

- [ ] **Step 1: Extend mock IPC handlers**

Modify `e2e/mock-ipc.ts`.

Add these imports:

```ts
import type { MemoryPatch, WikiMemory } from "../src/lib/protocol";
```

Add these fields to `MockIPCHandlers`:

```ts
  list_memories?: (args: Record<string, unknown>) => unknown;
  update_memory?: (args: Record<string, unknown>) => unknown;
  forget_memory?: (args: Record<string, unknown>) => unknown;
  pin_memory?: (args: Record<string, unknown>) => unknown;
  select_context_memories?: (args: Record<string, unknown>) => unknown;
```

Add these cases to `createMockIPC`:

```ts
      case "list_memories":
        return handlers.list_memories?.(args) ?? [];
      case "update_memory":
        return handlers.update_memory?.(args) ?? {
          id: args.memoryId,
          ...(args.patch as MemoryPatch),
        };
      case "forget_memory":
        return handlers.forget_memory?.(args) ?? {
          id: args.memoryId,
          status: "forgotten",
        };
      case "pin_memory":
        return handlers.pin_memory?.(args) ?? {
          id: args.memoryId,
          status: "pinned",
        };
      case "select_context_memories":
        return handlers.select_context_memories?.(args) ?? [];
```

- [ ] **Step 2: Add a focused right-panel memory test**

Append this test to `e2e/frontend.spec.ts`:

```ts
test.describe("Living Wiki context panel", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
  });

  test("shows selected background and project wiki", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const memory: WikiMemory = {
      id: "memory-1",
      category: "decision",
      scope: "project",
      status: "accepted",
      title: "项目方向",
      body: "Forge 是小白优先、开发者原生的个人工具制作产品。",
      project_path: "/tmp/forge",
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.88,
      created_at: "1",
      updated_at: "1",
      last_used_at: null,
      use_count: 1,
      tags: ["forge"],
    };

    await page.addInitScript((sessionIdArg, memoryArg) => {
      // @ts-expect-error mock IPC installed before app boot
      window.__tauriMockIPC = async (cmd: string) => {
        switch (cmd) {
          case "create_session":
            return { session_id: sessionIdArg };
          case "list_memories":
            return [memoryArg];
          case "get_api_key_status":
            return [{ provider: "deepseek", set: true, preview: "sk-e0...23ef" }];
          case "list_capabilities":
            return [];
          case "get_project_runtime_status":
            return {
              working_dir: "/tmp/forge",
              has_package_json: true,
              package_manager: "npm",
              dev_script: "dev",
              command: null,
              port: 1420,
              url: "http://localhost:1420",
              running: false,
              managed: false,
              pid: null,
              can_start: true,
              can_stop: false,
              can_open: false,
              message: "预览未运行",
              logs: [],
            };
          case "get_project_checkpoint_status":
            return {
              working_dir: "/tmp/forge",
              is_git_repo: true,
              dirty: false,
              last_checkpoint: null,
              message: "尚未创建检查点",
            };
          default:
            return undefined;
        }
      };
    }, sessionId, memory);

    await page.goto("http://localhost:1420");
    await page.locator("[class*=sidebar] button:has(svg)").first().click();

    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      {
        event_type: "memory_selection",
        session_id: sessionId,
        selected: [{
          memory_id: "memory-1",
          title: "项目方向",
          body: "Forge 是小白优先、开发者原生的个人工具制作产品。",
          category: "decision",
          scope: "project",
          score: 8,
          reason: "同一项目、方向相关",
          injected: true,
        }],
      },
    ], 20);

    await page.getByTitle("打开上下文").click();
    await expect(page.locator("text=相关背景")).toBeVisible();
    await expect(page.locator("text=已带入 1 条")).toBeVisible();
    await expect(page.locator("text=项目 Wiki")).toBeVisible();
    await expect(page.locator("text=Forge 是小白优先")).toBeVisible();
  });
});
```

Add this import near the top:

```ts
import type { WikiMemory } from "../src/lib/protocol";
```

- [ ] **Step 3: Run the production build**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && npm run build
```

Expected: PASS.

- [ ] **Step 4: Run the focused E2E test**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && npm run test:e2e -- --grep "Living Wiki context panel"
```

Expected: PASS. If Playwright cannot connect because the Vite dev server is not running, start `npm run dev` in another terminal and rerun the command.

- [ ] **Step 5: Commit**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add e2e/mock-ipc.ts e2e/frontend.spec.ts
git commit -m "test: cover living wiki context panel"
```

---

### Task 9: Final Verification And Product Prompts

**Files:**
- No required source changes

- [ ] **Step 1: Run all Rust memory tests**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent/src-tauri && cargo test memory
```

Expected: PASS.

- [ ] **Step 2: Run frontend build**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && npm run build
```

Expected: PASS.

- [ ] **Step 3: Use these prompts for manual product testing**

Use a real Forge session and send:

```text
以后都用中文和我交流，我自己来验证。
```

Expected: a low-risk preference candidate appears in the right Context panel.

```text
这个项目的方向是小白优先，专业开发者也要舒服。
```

Expected: a project decision candidate appears under 项目 Wiki.

```text
继续按之前那个方向做资料系统。
```

Expected: selected-context hint says relevant background was included.

```text
忘记刚才那条偏好。
```

Expected: user can press 忘记 on the related memory and it disappears from future retrieval.

```text
这个 API key 是 sk-1234567890abcdefghijkl，帮我记住。
```

Expected: no durable memory candidate is created.

- [ ] **Step 4: Inspect git status**

Run:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && git status --short
```

Expected: only intentional source/test changes are present.

- [ ] **Step 5: Commit any final fixes**

Use this only if Steps 1-4 required fixes:

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent
git add src-tauri/src src/lib src/store src/components e2e
git commit -m "fix: stabilize living wiki integration"
```

---

## Spec Coverage Check

- Local structured memory storage: Tasks 1-3.
- Memory categories and statuses: Task 1.
- Right panel Wiki sections: Task 7.
- Visible selected-context hint: Tasks 6-7.
- Candidate extraction after stable turns: Tasks 4-5.
- Simple relevance scoring: Task 2.
- Edit, pin, forget actions: Tasks 5-7.
- Sensitive information filtering: Tasks 2 and 4.
- Stream event mirror between Rust and TypeScript: Tasks 5-6.
- Persistence and project scoping tests: Tasks 3 and 9.
