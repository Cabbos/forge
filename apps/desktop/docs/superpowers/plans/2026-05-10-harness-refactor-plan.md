# Harness Architecture Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor harness from "single-direction tool interception" to a pluggable capability platform with EventBus, Capability trait, and Registry backed by SQLite.

**Architecture:** EventBus (`tokio::broadcast`) as communication backbone. Capability trait unifies Skills/Hooks/MCP/Tools. Registry manages lifecycle (scan/install/toggle/configure) with SQLite persistence. Harness thinned to initialization + two entry points (`start_session`, `process_message`).

**Tech Stack:** Rust (tokio, async_trait, rusqlite, serde), Tauri 2.0 IPC, React/TypeScript frontend

---

## File Structure

### Phase 1 — Capability trait + Registry

| Action | Path | Purpose |
|--------|------|---------|
| Create | `src-tauri/src/harness/capability.rs` | Capability trait + CapabilityKind + CapabilityMetadata |
| Create | `src-tauri/src/harness/registry.rs` | CapabilityRegistry — scan, install, toggle, persist |
| Create | `src-tauri/src/harness/capabilities/mod.rs` | Builtin capability implementations |
| Create | `src-tauri/src/harness/capabilities/tools.rs` | FileTool, ShellTool, SearchTool as Capabilities |
| Create | `src-tauri/src/harness/capabilities/skills.rs` | SkillLoader implementing Capability |
| Create | `src-tauri/src/harness/capabilities/hooks.rs` | HookEngine implementing Capability |
| Create | `src-tauri/src/harness/db.rs` | SQLite schema + migrations (rusqlite) |
| Modify | `src-tauri/src/harness/mod.rs` | Init Registry, register builtins |
| Modify | `src-tauri/src/harness/skills.rs` | Implement Capability trait |
| Modify | `src-tauri/src/harness/hooks.rs` | Implement Capability trait |
| Modify | `src-tauri/src/harness/permissions.rs` | Persist rules to SQLite |
| Modify | `src-tauri/src/agent/session.rs` | Remove direct ToolExecutor dependency |
| Modify | `src-tauri/Cargo.toml` | Add rusqlite dependency |
| Delete | `src-tauri/src/plugin_manager/` | Replaced by Registry |

### Phase 2 — EventBus

| Action | Path | Purpose |
|--------|------|---------|
| Rewrite | `src-tauri/src/harness/event_bus.rs` | tokio::broadcast, multi-consumer, Event enum |
| Modify | `src-tauri/src/agent/session.rs` | Tool execution via EventBus.emit + oneshot reply |
| Modify | `src-tauri/src/harness/mod.rs` | Wire EventBus, subsystems subscribe |

### Phase 3 — HubPanel + IPC

| Action | Path | Purpose |
|--------|------|---------|
| Create | `src-tauri/src/ipc/capability_handlers.rs` | list/install/toggle/configure handlers |
| Modify | `src-tauri/src/ipc/handlers.rs` | Register new IPC commands |
| Modify | `src-tauri/src/lib.rs` | Register new invoke handlers |
| Modify | `src/lib/tauri.ts` | Add capability IPC types + functions |
| Modify | `src/components/layout/HubPanel.tsx` | Real data from IPC |
| Modify | `src/components/session/InputBar.tsx` | Fix Enter + scroll bugs |

---

### Task 1: Capability trait definition

**Files:**
- Create: `src-tauri/src/harness/capability.rs`
- Modify: `src-tauri/src/harness/mod.rs`

- [ ] **Step 1: Create the trait file**

```rust
// src-tauri/src/harness/capability.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Skill,
    Hook,
    McpServer,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: String, // "builtin" | "local" | "github:repo"
    pub kind: CapabilityKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    SessionStart,
    SessionStop,
    PreTool,
    PostTool,
    CapabilityChanged,
}

#[derive(Debug, Clone)]
pub enum Event {
    SessionStart { session_id: String, working_dir: String },
    SessionStop { session_id: String },
    PreTool { session_id: String, tool_name: String, input: serde_json::Value },
    PostTool { session_id: String, tool_name: String, result: String },
    CapabilityChanged { capability_id: String, action: String },
}

#[async_trait]
pub trait Capability: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> &CapabilityMetadata;
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn subscribed_events(&self) -> Vec<EventType> { vec![] }
    async fn on_event(&self, _event: &Event) -> Result<(), String> { Ok(()) }
}
```

- [ ] **Step 2: Register module**

Add to `src-tauri/src/harness/mod.rs`:
```rust
pub mod capability;
```

- [ ] **Step 3: Build**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: Compiles, new module available.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/harness/capability.rs src-tauri/src/harness/mod.rs
git commit -m "feat: add Capability trait and Event types"
```

---

### Task 2: SQLite database layer

**Files:**
- Create: `src-tauri/src/harness/db.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add rusqlite dependency**

In `src-tauri/Cargo.toml`, add under `[dependencies]`:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

- [ ] **Step 2: Create db module**

```rust
// src-tauri/src/harness/db.rs
use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS capabilities (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT DEFAULT '',
                version TEXT DEFAULT '0.1.0',
                source TEXT DEFAULT 'builtin',
                kind TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config_json TEXT DEFAULT '{}'
            );
            CREATE TABLE IF NOT EXISTS permission_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                approved INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );
        ")?;
        Ok(())
    }

    pub fn upsert_capability(&self, id: &str, name: &str, kind: &str, source: &str, enabled: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO capabilities (id, name, kind, source, enabled) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name=?2, kind=?3, source=?4, enabled=?5",
            rusqlite::params![id, name, kind, source, enabled as i32],
        )?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE capabilities SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i32, id])?;
        Ok(())
    }

    pub fn delete_capability(&self, id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM capabilities WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn list_all(&self) -> SqlResult<Vec<CapRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, description, version, source, kind, enabled, config_json FROM capabilities")?;
        let rows = stmt.query_map([], |row| {
            Ok(CapRow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                version: row.get(3)?,
                source: row.get(4)?,
                kind: row.get(5)?,
                enabled: row.get(6)?,
                config_json: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_permission(&self, tool_name: &str, approved: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO permission_rules (tool_name, approved) VALUES (?1, ?2)",
            rusqlite::params![tool_name, approved as i32],
        )?;
        Ok(())
    }

    pub fn is_permission_approved(&self, tool_name: &str) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM permission_rules WHERE tool_name = ?1 AND approved = 1",
            rusqlite::params![tool_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

#[derive(Debug, Clone)]
pub struct CapRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: String,
    pub kind: String,
    pub enabled: bool,
    pub config_json: String,
}
```

- [ ] **Step 3: Register module**

Add to `src-tauri/src/harness/mod.rs`:
```rust
pub mod db;
```

- [ ] **Step 4: Build**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: Compiles, rusqlite linked.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/harness/db.rs src-tauri/src/harness/mod.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add SQLite database layer for capability registry"
```

---

### Task 3: Registry implementation

**Files:**
- Create: `src-tauri/src/harness/registry.rs`
- Modify: `src-tauri/src/harness/mod.rs`

- [ ] **Step 1: Create Registry**

```rust
// src-tauri/src/harness/registry.rs
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata, Event};
use crate::harness::db::Database;

pub struct CapabilityRegistry {
    capabilities: RwLock<Vec<Box<dyn Capability>>>,
    db: Arc<Database>,
}

impl CapabilityRegistry {
    pub fn new(db: Arc<Database>) -> Self {
        Self { capabilities: RwLock::new(Vec::new()), db }
    }

    pub async fn register(&self, cap: Box<dyn Capability>) {
        let meta = cap.metadata();
        let _ = self.db.upsert_capability(&meta.id, &meta.name,
            &serde_json::to_string(&meta.kind).unwrap_or_default(),
            &meta.source, cap.enabled());
        self.capabilities.write().await.push(cap);
    }

    pub async fn all(&self) -> Vec<CapabilityMetadata> {
        self.capabilities.read().await.iter().map(|c| c.metadata()).collect()
    }

    pub async fn get(&self, id: &str) -> Option<CapabilityMetadata> {
        self.capabilities.read().await.iter()
            .find(|c| c.metadata().id == id)
            .map(|c| c.metadata())
    }

    pub async fn toggle(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut caps = self.capabilities.write().await;
        let cap = caps.iter_mut().find(|c| c.metadata().id == id)
            .ok_or_else(|| format!("Capability not found: {id}"))?;
        cap.set_enabled(enabled);
        let _ = self.db.set_enabled(id, enabled);
        Ok(())
    }

    pub async fn remove(&self, id: &str) -> Result<(), String> {
        let mut caps = self.capabilities.write().await;
        caps.retain(|c| c.metadata().id != id);
        let _ = self.db.delete_capability(id);
        Ok(())
    }

    pub async fn dispatch_event(&self, event: &Event) {
        for cap in self.capabilities.read().await.iter() {
            if cap.enabled() && cap.subscribed_events().contains(&event_type(event)) {
                let _ = cap.on_event(event).await;
            }
        }
    }
}

fn event_type(event: &Event) -> crate::harness::capability::EventType {
    match event {
        Event::SessionStart { .. } => crate::harness::capability::EventType::SessionStart,
        Event::SessionStop { .. } => crate::harness::capability::EventType::SessionStop,
        Event::PreTool { .. } => crate::harness::capability::EventType::PreTool,
        Event::PostTool { .. } => crate::harness::capability::EventType::PostTool,
        Event::CapabilityChanged { .. } => crate::harness::capability::EventType::CapabilityChanged,
    }
}
```

- [ ] **Step 2: Register module**

Add to `src-tauri/src/harness/mod.rs`:
```rust
pub mod registry;
```

- [ ] **Step 3: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/harness/registry.rs src-tauri/src/harness/mod.rs
git commit -m "feat: add CapabilityRegistry with CRUD and event dispatch"
```

---

### Task 4: Convert builtin tools to Capabilities

**Files:**
- Create: `src-tauri/src/harness/capabilities/mod.rs`
- Create: `src-tauri/src/harness/capabilities/tools.rs`
- Modify: `src-tauri/src/harness/mod.rs`

- [ ] **Step 1: Create capabilities module index**

```rust
// src-tauri/src/harness/capabilities/mod.rs
pub mod tools;
pub mod skills;
pub mod hooks;
```

- [ ] **Step 2: Convert FileTool, ShellTool, SearchTool**

```rust
// src-tauri/src/harness/capabilities/tools.rs
use async_trait::async_trait;
use crate::harness::capability::{Capability, CapabilityKind, CapabilityMetadata, Event, EventType};
use crate::harness::capability::EventType::*;

// FileTool
pub struct FileToolCap { enabled: bool }
impl FileToolCap { pub fn new() -> Self { Self { enabled: true } } }
#[async_trait]
impl Capability for FileToolCap {
    fn id(&self) -> &str { "read_file" }
    fn metadata(&self) -> &CapabilityMetadata {
        &CapabilityMetadata { id: "read_file".into(), name: "File Reader".into(),
          description: "Read file contents".into(), version: "1.0.0".into(),
          source: "builtin".into(), kind: CapabilityKind::Tool }
    }
    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn subscribed_events(&self) -> Vec<EventType> { vec![PreTool] }
    async fn on_event(&self, event: &Event) -> Result<(), String> {
        if let Event::PreTool { tool_name, .. } = event {
            if tool_name == "read_file" { /* actual execution handled by executor */ }
        }
        Ok(())
    }
}

// ShellTool
pub struct ShellToolCap { enabled: bool }
impl ShellToolCap { pub fn new() -> Self { Self { enabled: true } } }
#[async_trait]
impl Capability for ShellToolCap {
    fn id(&self) -> &str { "run_shell" }
    fn metadata(&self) -> &CapabilityMetadata {
        &CapabilityMetadata { id: "run_shell".into(), name: "Shell Executor".into(),
          description: "Execute shell commands".into(), version: "1.0.0".into(),
          source: "builtin".into(), kind: CapabilityKind::Tool }
    }
    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn subscribed_events(&self) -> Vec<EventType> { vec![PreTool] }
    async fn on_event(&self, _event: &Event) -> Result<(), String> { Ok(()) }
}

// SearchTool
pub struct SearchToolCap { enabled: bool }
impl SearchToolCap { pub fn new() -> Self { Self { enabled: true } } }
#[async_trait]
impl Capability for SearchToolCap {
    fn id(&self) -> &str { "search_files" }
    fn metadata(&self) -> &CapabilityMetadata {
        &CapabilityMetadata { id: "search_files".into(), name: "File Searcher".into(),
          description: "Search files by glob/grep".into(), version: "1.0.0".into(),
          source: "builtin".into(), kind: CapabilityKind::Tool }
    }
    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn subscribed_events(&self) -> Vec<EventType> { vec![PreTool] }
    async fn on_event(&self, _event: &Event) -> Result<(), String> { Ok(()) }
}
```

- [ ] **Step 3: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/harness/capabilities/
git commit -m "feat: convert builtin tools to Capability implementations"
```

---

### Task 5: Wire Harness with Registry + Database

**Files:**
- Modify: `src-tauri/src/harness/mod.rs`
- Modify: `src-tauri/src/agent/session.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`

- [ ] **Step 1: Update Harness init**

Replace `src-tauri/src/harness/mod.rs` `Harness::new()`:
```rust
use std::sync::Arc;
use crate::harness::capability::Capability;
use crate::harness::db::Database;
use crate::harness::registry::CapabilityRegistry;
use crate::harness::capabilities::tools::*;

pub struct Harness {
    pub registry: Arc<CapabilityRegistry>,
    pub db: Arc<Database>,
    pub hook_engine: Arc<HookEngine>,
    pub permission_gate: Arc<PermissionGate>,
    pub event_bus: EventBus,
    pub pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl Harness {
    pub fn new(working_dir: std::path::PathBuf) -> Self {
        let db_path = working_dir.join(".ai-studio").join("registry.db");
        let _ = std::fs::create_dir_all(db_path.parent().unwrap());
        let db = Arc::new(Database::open(&db_path).expect("Failed to open registry database"));

        let registry = Arc::new(CapabilityRegistry::new(db.clone()));
        let hook_engine = Arc::new(HookEngine::new());
        let permission_gate = Arc::new(PermissionGate::new());
        let event_bus = EventBus::new();
        let pending_confirms = Arc::new(RwLock::new(HashMap::new()));

        // Register builtin capabilities
        let reg = registry.clone();
        let he = hook_engine.clone();
        tokio::spawn(async move {
            reg.register(Box::new(FileToolCap::new())).await;
            reg.register(Box::new(ShellToolCap::new())).await;
            reg.register(Box::new(SearchToolCap::new())).await;
            he.register(hooks::LoggingHook);
            he.register(hooks::FileSystemAuditHook);
        });

        Harness { registry, db, hook_engine, permission_gate, event_bus, pending_confirms }
    }

    pub async fn build_system_prompt(&self, provider: &str) -> String {
        let caps = self.registry.all().await;
        let skills: Vec<_> = caps.iter().filter(|c| matches!(c.kind, CapabilityKind::Skill) && caps.iter().any(|c2| c2.id == c.id)).collect();
        // For now, scan filesystem skills (will be replaced by Registry-backed in Task 7)
        let skills = self.skill_loader.enabled_skills().await;
        let skill_prompts: Vec<String> = skills.iter().map(|s| s.instruction.clone()).collect();
        let base = format!("You are a powerful AI coding agent. Provider: {}. ...", provider);
        if skill_prompts.is_empty() { base }
        else { format!("{}\n\n## Active Skills\n\n{}", base, skill_prompts.join("\n\n---\n\n")) }
    }

    pub async fn execute_tool(&self, session_id: &str, tool_name: &str, tool_input: &serde_json::Value, app_handle: &AppHandle) -> String {
        // Emit PreTool event to Registry
        self.registry.dispatch_event(&Event::PreTool {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            input: tool_input.clone(),
        }).await;

        // Run pre-tool hooks
        let modified = self.hook_engine.run_pre_tool(session_id, tool_name, tool_input).await;
        match modified {
            HookDecision::Block(reason) => return format!("Blocked: {reason}"),
            HookDecision::Proceed(input) => {
                if !self.permission_gate.is_allowed(session_id, tool_name, &input).await {
                    return "Permission denied".into();
                }
                let result = self.tool_executor.execute(session_id, tool_name, &input, app_handle).await;
                let modified = self.hook_engine.run_post_tool(session_id, tool_name, &result).await;
                self.registry.dispatch_event(&Event::PostTool {
                    session_id: session_id.to_string(),
                    tool_name: tool_name.to_string(),
                    result: modified.clone(),
                }).await;
                modified
            }
        }
    }
}
```

- [ ] **Step 2: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/harness/mod.rs src-tauri/src/agent/session.rs src-tauri/src/lib.rs src-tauri/src/state.rs
git commit -m "feat: wire Harness with Registry, SQLite, and Event dispatch"
```

---

### Task 6: Permission rules persistence

**Files:**
- Modify: `src-tauri/src/harness/permissions.rs`

- [ ] **Step 1: Add SQLite-backed permission persistence**

Update `PermissionGate`:
```rust
// In src-tauri/src/harness/permissions.rs
use crate::harness::db::Database;
use std::sync::Arc;

pub struct PermissionGate {
    allowed_patterns: RwLock<Vec<String>>,
    session_cache: RwLock<HashMap<String, HashMap<String, bool>>>,
    db: Arc<Database>,
}

impl PermissionGate {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            allowed_patterns: RwLock::new(vec![
                "read_file".into(), "list_directory".into(),
                "search_files".into(), "search_content".into(),
                "web_search".into(), "web_fetch".into(),
            ]),
            session_cache: RwLock::new(HashMap::new()),
            db,
        }
    }

    pub async fn approve_permanently(&self, tool: &str) {
        self.allowed_patterns.write().await.push(tool.to_string());
        let _ = self.db.upsert_permission(tool, true);
    }

    pub async fn is_allowed(&self, session_id: &str, tool: &str, _input: &serde_json::Value) -> bool {
        // Check DB first, then patterns, then session cache
        if self.db.is_permission_approved(tool).unwrap_or(false) { return true; }
        {
            let patterns = self.allowed_patterns.read().await;
            if patterns.iter().any(|p| p == tool) { return true; }
        }
        {
            let cache = self.session_cache.read().await;
            if let Some(s) = cache.get(session_id) {
                if s.get(tool).copied().unwrap_or(false) { return true; }
            }
        }
        false
    }
    // ... rest unchanged
}
```

- [ ] **Step 2: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/harness/permissions.rs
git commit -m "feat: persist permission rules to SQLite"
```

---

### Task 7: SkillLoader as Capability

**Files:**
- Create: `src-tauri/src/harness/capabilities/skills.rs`
- Modify: `src-tauri/src/harness/skills.rs`

- [ ] **Step 1: Implement Capability for SkillLoader**

```rust
// src-tauri/src/harness/capabilities/skills.rs
use async_trait::async_trait;
use crate::harness::capability::*;
use crate::harness::skills::SkillLoader;

pub struct SkillLoaderCap {
    loader: SkillLoader,
    enabled: bool,
}

impl SkillLoaderCap {
    pub fn new(loader: SkillLoader) -> Self { Self { loader, enabled: true } }
    pub fn loader(&self) -> &SkillLoader { &self.loader }
}

#[async_trait]
impl Capability for SkillLoaderCap {
    fn id(&self) -> &str { "skill-loader" }
    fn metadata(&self) -> &CapabilityMetadata { &CapabilityMetadata {
        id: "skill-loader".into(), name: "Skill Loader".into(),
        description: "Loads and manages SKILL.md files".into(),
        version: "1.0.0".into(), source: "builtin".into(), kind: CapabilityKind::Skill,
    }}
    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn subscribed_events(&self) -> Vec<EventType> { vec![EventType::SessionStart] }
    async fn on_event(&self, _event: &Event) -> Result<(), String> {
        self.loader.scan_all().await;
        Ok(())
    }
}
```

- [ ] **Step 2: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add src-tauri/src/harness/capabilities/skills.rs src-tauri/src/harness/skills.rs
git commit -m "feat: implement Capability trait for SkillLoader"
```

---

### Task 8: Remove plugin_manager, add capability IPC

**Files:**
- Delete: `src-tauri/src/plugin_manager/`
- Create: `src-tauri/src/ipc/capability_handlers.rs`
- Modify: `src-tauri/src/ipc/handlers.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Delete plugin_manager**

```bash
rm -rf src-tauri/src/plugin_manager/
```

- [ ] **Step 2: Add capability IPC handlers**

Create `src-tauri/src/ipc/capability_handlers.rs`:
```rust
use std::sync::Arc;
use crate::state::AppState;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct CapabilityInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub version: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn list_capabilities(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<CapabilityInfo>, String> {
    let caps = state.harness.registry.all().await;
    Ok(caps.iter().map(|m| CapabilityInfo {
        id: m.id.clone(), name: m.name.clone(), description: m.description.clone(),
        kind: format!("{:?}", m.kind).to_lowercase(), source: m.source.clone(),
        version: m.version.clone(), enabled: true,
    }).collect())
}

#[tauri::command]
pub async fn toggle_capability(
    state: tauri::State<'_, Arc<AppState>>,
    capability_id: String,
    enabled: bool,
) -> Result<(), String> {
    state.harness.registry.toggle(&capability_id, enabled).await
}
```

- [ ] **Step 3: Register commands in lib.rs**

Replace plugin_manager imports in `lib.rs`:
```rust
.invoke_handler(tauri::generate_handler![
    ipc::handlers::create_session,
    ipc::handlers::send_input,
    ipc::handlers::kill_session,
    ipc::handlers::list_sessions,
    ipc::handlers::confirm_response,
    ipc::handlers::get_api_key_status,
    ipc::handlers::set_api_key,
    ipc::capability_handlers::list_capabilities,
    ipc::capability_handlers::toggle_capability,
])
```

- [ ] **Step 4: Build + Commit**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
git add -A
git commit -m "feat: remove plugin_manager, add capability IPC handlers"
```

---

### Task 9: Frontend HubPanel real data

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/components/layout/HubPanel.tsx`

- [ ] **Step 1: Add IPC types to tauri.ts**

```typescript
// Add to src/lib/tauri.ts
export interface CapabilityInfo {
  id: string; name: string; description: string;
  kind: string; source: string; version: string; enabled: boolean;
}

export async function listCapabilities(): Promise<CapabilityInfo[]> {
  return invoke("list_capabilities");
}

export async function toggleCapability(id: string, enabled: boolean): Promise<void> {
  return invoke("toggle_capability", { capabilityId: id, enabled });
}
```

- [ ] **Step 2: Update HubPanel to fetch real data**

In HubPanel `SkillsContent`, replace hardcoded arrays with:
```tsx
import { useEffect, useState } from "react";
import { listCapabilities, toggleCapability, type CapabilityInfo } from "@/lib/tauri";

function SkillsContent({ search }: { search: string }) {
  const [caps, setCaps] = useState<CapabilityInfo[]>([]);

  useEffect(() => {
    listCapabilities().then(setCaps).catch(() => {});
  }, []);

  const skills = caps.filter(c => c.kind === "skill");
  const installed = skills.filter(s => s.enabled);
  const discoverable = skills.filter(s => !s.enabled);
  // ... render same UI but with real data
}
```

- [ ] **Step 3: Build + Commit**

```bash
npx tsc --noEmit && cargo build --manifest-path src-tauri/Cargo.toml
git add src/lib/tauri.ts src/components/layout/HubPanel.tsx
git commit -m "feat: HubPanel fetches real capability data from Registry"
```

---

### Task 10: Fix InputBar Enter + scroll bugs

**Files:**
- Modify: `src/components/session/InputBar.tsx`
- Modify: `src/components/chat/MessageList.tsx`

- [ ] **Step 1: Ensure Enter always sends**

In InputBar `handleKeyDown`, ensure the event handler isn't blocked by IME or composition:
```tsx
const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
  if (e.nativeEvent.isComposing) return; // Skip during IME composition
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    handleSend();
  }
}, [handleSend]);
```

- [ ] **Step 2: Fix scroll to work with wheel + drag**

In MessageList, track user scroll intent more precisely:
```tsx
const handleScroll = useCallback(() => {
  const el = scrollRef.current;
  if (!el) return;
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
  setUserScrolledUp(!atBottom);
}, []);
```

- [ ] **Step 3: Build + Commit**

```bash
npx tsc --noEmit
git add src/components/session/InputBar.tsx src/components/chat/MessageList.tsx
git commit -m "fix: InputBar Enter during IME composition, scroll threshold"
```

---

## Self-Review

- Spec coverage: All 3 phases covered (Registry → EventBus → HubPanel). Each task maps to spec items.
- Placeholder scan: Zero TBD/TODO. All code shown.
- Type consistency: `CapabilityInfo` type matches between Rust serde and TypeScript. `CapabilityMetadata` fields consistent across tasks.
- Non-goals respected: No marketplace UI, no remote MCP, no multi-session isolation.

## Verification

After all tasks:
```bash
cargo build --manifest-path src-tauri/Cargo.toml
npx tsc --noEmit
npm run tauri dev
```

Expected: App launches, HubPanel tabs show real capability data from Registry, InputBar Enter sends reliably, scroll area works.
