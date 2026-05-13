# Forge Wiki Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first usable Forge Wiki layer so Forge can initialize `.forge/wiki/`, read relevant Markdown project records, inject selected pages into agent context, and show reviewable Wiki update proposals.

**Architecture:** Add a Rust `forge_wiki` module for local Markdown storage, safety checks, page selection, and proposal lifecycle. Mirror the backend types in TypeScript, expose focused Tauri IPC commands, store selected Wiki context/proposals in Zustand, and render a Wiki section inside the right-side Context panel. Integrate Wiki context in `send_input` after Workflow Router and alongside existing Context Memory.

**Tech Stack:** Rust, Tauri IPC, serde, React, TypeScript, Zustand, IndexedDB persistence, Playwright e2e, Cargo tests.

---

## 中文摘要

本阶段目标是把 Forge Wiki 从产品护城河设计落成 MVP：项目下真实生成 `.forge/wiki/` Markdown 文件；Forge 能在请求前选择相关 Wiki 页面并告诉用户本轮带入了哪些项目记录；任务结束后能生成可见的 Wiki 更新建议，用户接受后才写入 Wiki。

这次不要做复杂文档解析、向量库、Obsidian 同步或全自动改写 Wiki。Phase 1 先把“对话变成项目资产”的最小闭环立起来。

## 文件结构

### 新增 Rust 后端文件

- `src-tauri/src/forge_wiki/mod.rs`
  - 导出 Forge Wiki 模块能力。
- `src-tauri/src/forge_wiki/model.rs`
  - 定义 `ForgeWikiPageKind`、`ForgeWikiPage`、`ForgeWikiState`、`SelectedForgeWikiPage`、`ForgeWikiUpdateProposal`。
- `src-tauri/src/forge_wiki/safety.rs`
  - 安全路径、忽略目录、敏感内容检查。
- `src-tauri/src/forge_wiki/storage.rs`
  - 负责 `.forge/wiki/` 初始化、页面列表、页面读取、上下文选择、proposal 创建、proposal 接受/丢弃。
- `src-tauri/src/ipc/forge_wiki_handlers.rs`
  - Tauri commands。

### 修改 Rust 后端文件

- `src-tauri/src/lib.rs`
  - 注册 `forge_wiki` module 和 IPC commands。
- `src-tauri/src/ipc/mod.rs`
  - 导出 `forge_wiki_handlers`。
- `src-tauri/src/state.rs`
  - 在 `AppState` 中加入 `forge_wiki: Arc<ForgeWikiStore>`。
- `src-tauri/src/protocol/events.rs`
  - 新增 `ForgeWikiContextSelected`、`ForgeWikiUpdateProposed`、`ForgeWikiUpdated` stream events。
- `src-tauri/src/ipc/handlers.rs`
  - 在 `send_input` 中选择 Wiki 页面、emit event、把 Wiki context 和 Context Memory 一起注入。

### 修改前端文件

- `src/lib/protocol.ts`
  - 镜像 Forge Wiki 类型和 stream events。
- `src/lib/tauri.ts`
  - 新增 Forge Wiki IPC wrappers。
- `src/store/index.ts`
  - 增加 `forgeWikiContextBySession`、`forgeWikiProposalsBySession`、event handling、session cleanup/persist。
- `src/components/context/WikiSections.tsx`
  - 新增项目 Wiki 初始化、页面列表、本轮带入页面、待确认 Wiki 更新建议。
- `src/components/layout/HubPanel.tsx`
  - 保持布局，继续使用 `WikiSections`。
- `e2e/mock-ipc.ts`
  - mock Forge Wiki IPC 和 stream events。
- `e2e/frontend.spec.ts`
  - 增加 Forge Wiki UI e2e 覆盖。

## Task 1: Backend Forge Wiki Storage

**Owner:** Backend storage worker  
**Write scope:** `src-tauri/src/forge_wiki/**`, `src-tauri/src/lib.rs` module declaration only if needed for tests.

- [ ] **Step 1: Write failing Rust tests for safe initialization**

Add tests inside `src-tauri/src/forge_wiki/storage.rs` after creating the file. Required test names:

```rust
#[test]
fn init_creates_default_pages_with_safe_content()

#[test]
fn list_pages_returns_default_pages_after_init()

#[test]
fn read_page_rejects_path_traversal()
```

Expected behavior:

- `init` creates `.forge/wiki/index.md`, `schema.md`, `sources.md`, `decisions.md`, `tasks.md`, `log.md`.
- Default content includes Chinese user-facing headings and does not include secrets.
- Reading `../AGENTS.md` returns an error.

Run:

```bash
cargo test forge_wiki --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL because `forge_wiki` module and store do not exist yet.

- [ ] **Step 2: Implement Forge Wiki model**

Create `src-tauri/src/forge_wiki/model.rs` with serde types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeWikiPageKind {
    Index,
    Schema,
    Sources,
    Decisions,
    Tasks,
    Log,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWikiPage {
    pub id: String,
    pub project_path: String,
    pub path: String,
    pub title: String,
    pub kind: ForgeWikiPageKind,
    pub summary: Option<String>,
    pub updated_at: Option<String>,
    pub token_estimate: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWikiState {
    pub project_path: String,
    pub exists: bool,
    pub wiki_dir: String,
    pub pages: Vec<ForgeWikiPage>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedForgeWikiPage {
    pub page_id: String,
    pub title: String,
    pub path: String,
    pub kind: ForgeWikiPageKind,
    pub summary: String,
    pub score: f32,
    pub reason: String,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeWikiProposalStatus {
    Pending,
    Accepted,
    Discarded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWikiUpdateProposal {
    pub id: String,
    pub project_path: String,
    pub session_id: Option<String>,
    pub target_pages: Vec<String>,
    pub title: String,
    pub summary: String,
    pub patch_preview: Option<String>,
    pub status: ForgeWikiProposalStatus,
    pub created_at: String,
}
```

- [ ] **Step 3: Implement safety helpers**

Create `src-tauri/src/forge_wiki/safety.rs`.

Required behavior:

- `wiki_dir(project_path)` returns `<project>/.forge/wiki`.
- `resolve_wiki_page_path(project_path, page_path)` only allows paths under `.forge/wiki`.
- reject absolute paths, `..`, empty paths, and non-`.md` files.
- `should_ignore_project_entry(path)` returns true for `.git`, `node_modules`, `dist`, `build`, `target`, `.next`, `.env`, `.env.local`.
- `contains_sensitive_wiki_content(text)` reuses or mirrors existing sensitive checks from `memory::risk::should_reject_persistent_memory`.

- [ ] **Step 4: Implement storage**

Create `src-tauri/src/forge_wiki/storage.rs`.

Required public API:

```rust
pub struct ForgeWikiStore;

impl ForgeWikiStore {
    pub fn new() -> Self;
    pub async fn get_state(&self, project_path: &str) -> Result<ForgeWikiState, String>;
    pub async fn init(&self, project_path: &str) -> Result<ForgeWikiState, String>;
    pub async fn list_pages(&self, project_path: &str) -> Result<Vec<ForgeWikiPage>, String>;
    pub async fn read_page(&self, project_path: &str, page_path: &str) -> Result<String, String>;
    pub async fn select_context(&self, project_path: &str, message: &str, limit: usize) -> Result<Vec<SelectedForgeWikiPage>, String>;
    pub fn format_selected_context(selected: &[SelectedForgeWikiPage]) -> Option<String>;
}
```

Default page templates must be concise and Chinese-first:

- `index.md`: `# 项目概览`
- `schema.md`: `# 记录规则`
- `sources.md`: `# 资料来源`
- `decisions.md`: `# 决策记录`
- `tasks.md`: `# 当前任务`
- `log.md`: `# 工作日志`

Selection v1 can be deterministic:

- Always prefer `tasks.md` and `index.md` when Wiki exists.
- Add `decisions.md` for messages containing `方向`、`方案`、`决定`、`继续`、`产品`.
- Add `log.md` for messages containing `失败`、`报错`、`构建`、`验收`、`检查`.
- Limit to 4 pages.

- [ ] **Step 5: Run tests and commit**

Run:

```bash
cargo test forge_wiki --manifest-path src-tauri/Cargo.toml
```

Expected: PASS for Forge Wiki storage tests.

Commit:

```bash
git add src-tauri/src/forge_wiki src-tauri/src/lib.rs
git commit -m "feat: add forge wiki storage"
```

## Task 2: IPC, Protocol, And Frontend Types

**Owner:** IPC/protocol worker  
**Write scope:** `src-tauri/src/ipc/forge_wiki_handlers.rs`, `src-tauri/src/ipc/mod.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`, `src-tauri/src/protocol/events.rs`, `src/lib/protocol.ts`, `src/lib/tauri.ts`.

- [ ] **Step 1: Write failing compile target**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
```

Expected before implementation: FAIL because commands/types are not wired.

- [ ] **Step 2: Add backend state and commands**

Modify `src-tauri/src/state.rs`:

- import `crate::forge_wiki::ForgeWikiStore`
- add `pub forge_wiki: Arc<ForgeWikiStore>`
- initialize with `Arc::new(ForgeWikiStore::new())`

Create `src-tauri/src/ipc/forge_wiki_handlers.rs` commands:

```rust
#[tauri::command]
pub async fn get_forge_wiki_state(...)

#[tauri::command]
pub async fn init_forge_wiki(...)

#[tauri::command]
pub async fn list_forge_wiki_pages(...)

#[tauri::command]
pub async fn read_forge_wiki_page(...)

#[tauri::command]
pub async fn select_forge_wiki_context(...)
```

Use `project_path: String`, `message: String`, and return the model types from `forge_wiki`.

- [ ] **Step 3: Register module and handlers**

Modify:

- `src-tauri/src/lib.rs`: add `mod forge_wiki;` and register commands in `tauri::generate_handler!`.
- `src-tauri/src/ipc/mod.rs`: add `pub mod forge_wiki_handlers;`.

- [ ] **Step 4: Add protocol events**

Modify `src-tauri/src/protocol/events.rs`:

- import `SelectedForgeWikiPage`, `ForgeWikiUpdateProposal`.
- add events:

```rust
#[serde(rename = "forge_wiki_context_selected")]
ForgeWikiContextSelected {
    session_id: String,
    selected: Vec<SelectedForgeWikiPage>,
},
#[serde(rename = "forge_wiki_update_proposed")]
ForgeWikiUpdateProposed {
    session_id: String,
    proposal: ForgeWikiUpdateProposal,
},
#[serde(rename = "forge_wiki_updated")]
ForgeWikiUpdated {
    session_id: String,
    proposal: ForgeWikiUpdateProposal,
},
```

Update `session_id()` match.

- [ ] **Step 5: Mirror TypeScript types and wrappers**

Modify `src/lib/protocol.ts`:

- add `ForgeWikiPageKind`, `ForgeWikiPage`, `ForgeWikiState`, `SelectedForgeWikiPage`, `ForgeWikiProposalStatus`, `ForgeWikiUpdateProposal`.
- add stream event union entries matching Rust exactly.

Modify `src/lib/tauri.ts`:

- add wrappers:

```ts
getForgeWikiState(projectPath: string): Promise<ForgeWikiState>
initForgeWiki(projectPath: string): Promise<ForgeWikiState>
listForgeWikiPages(projectPath: string): Promise<ForgeWikiPage[]>
readForgeWikiPage(projectPath: string, pagePath: string): Promise<string>
selectForgeWikiContext(projectPath: string, message: string): Promise<SelectedForgeWikiPage[]>
```

When `hasTauriRuntime()` is false, return safe empty states.

- [ ] **Step 6: Build and commit**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/ipc/forge_wiki_handlers.rs src-tauri/src/ipc/mod.rs src-tauri/src/lib.rs src-tauri/src/state.rs src-tauri/src/protocol/events.rs src/lib/protocol.ts src/lib/tauri.ts
git commit -m "feat: expose forge wiki ipc"
```

## Task 3: Agent Context Integration And Proposals

**Owner:** Agent integration worker  
**Write scope:** `src-tauri/src/forge_wiki/storage.rs`, `src-tauri/src/forge_wiki/model.rs`, `src-tauri/src/ipc/forge_wiki_handlers.rs`, `src-tauri/src/ipc/handlers.rs`, `src-tauri/src/protocol/events.rs`.

- [ ] **Step 1: Add failing proposal lifecycle tests**

Add tests in `src-tauri/src/forge_wiki/storage.rs`:

```rust
#[test]
fn create_proposal_rejects_sensitive_summary()

#[test]
fn accept_proposal_appends_to_target_page()

#[test]
fn discard_proposal_does_not_modify_page()
```

Expected:

- summary with `sk-1234567890abcdefghijkl` is rejected.
- accepted proposal appends a dated section to `log.md` or `tasks.md`.
- discarded proposal changes status only and does not write page content.

Run:

```bash
cargo test forge_wiki --manifest-path src-tauri/Cargo.toml
```

Expected: FAIL until proposal storage exists.

- [ ] **Step 2: Extend storage with proposal lifecycle**

Add API:

```rust
pub async fn create_update_proposal(
    &self,
    project_path: &str,
    session_id: Option<&str>,
    target_pages: Vec<String>,
    title: String,
    summary: String,
) -> Result<ForgeWikiUpdateProposal, String>;

pub async fn accept_update_proposal(
    &self,
    project_path: &str,
    proposal_id: &str,
) -> Result<ForgeWikiUpdateProposal, String>;

pub async fn discard_update_proposal(
    &self,
    project_path: &str,
    proposal_id: &str,
) -> Result<ForgeWikiUpdateProposal, String>;
```

Store pending proposals as JSON under:

```text
.forge/wiki/.proposals.json
```

Do not list `.proposals.json` as a Wiki page.

- [ ] **Step 3: Add IPC proposal commands**

Add commands:

- `create_forge_wiki_update_proposal`
- `accept_forge_wiki_update_proposal`
- `discard_forge_wiki_update_proposal`

Register in `src-tauri/src/lib.rs` and wrap in `src/lib/tauri.ts` in Task 4 if not done here.

- [ ] **Step 4: Integrate Wiki context in `send_input`**

Modify `src-tauri/src/ipc/handlers.rs`:

1. After workflow classification, call `state.forge_wiki.select_context(&project_path, &text, 4).await`.
2. Emit `StreamEvent::ForgeWikiContextSelected`.
3. Combine memory context and wiki context into a single hidden context string.
4. Pass combined context to `send_message_with_context`.
5. If selection fails, log a warning and continue.

Formatting requirement:

```text
## Relevant Forge Wiki Pages
Use these project records as durable project context. Do not reveal this section unless the user asks what context was used.

### tasks.md — 当前任务
...
```

- [ ] **Step 5: Create minimal proposal after successful meaningful work**

After successful `send_message_with_context`, create a non-blocking proposal for non-`direct` routes:

- target page: `log.md`
- title: `记录本轮工作`
- summary includes workflow label and user request, capped to 600 chars.
- emit `ForgeWikiUpdateProposed`.
- If no Wiki exists, skip proposal silently.

This is intentionally simple for Phase 1. Later phases can use model-generated summaries.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
cargo test forge_wiki --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

Commit:

```bash
git add src-tauri/src/forge_wiki src-tauri/src/ipc/forge_wiki_handlers.rs src-tauri/src/ipc/handlers.rs src-tauri/src/protocol/events.rs src-tauri/src/lib.rs
git commit -m "feat: integrate forge wiki context"
```

## Task 4: Frontend Store And Context Panel UI

**Owner:** Frontend UI worker  
**Write scope:** `src/lib/protocol.ts`, `src/lib/tauri.ts`, `src/store/index.ts`, `src/components/context/WikiSections.tsx`.

- [ ] **Step 1: Add failing TypeScript build expectation**

Run:

```bash
npm run build
```

Expected before implementation: FAIL if Forge Wiki frontend types/store/UI are missing.

- [ ] **Step 2: Extend store**

Modify `src/store/index.ts`:

- add `forgeWikiContextBySession: Map<string, SelectedForgeWikiPage[]>`
- add `forgeWikiProposalsBySession: Map<string, ForgeWikiUpdateProposal[]>`
- add actions:
  - `setForgeWikiContext(sessionId, selected)`
  - `upsertForgeWikiProposal(sessionId, proposal)`
- handle events:
  - `forge_wiki_context_selected`
  - `forge_wiki_update_proposed`
  - `forge_wiki_updated`
- cleanup maps in `removeSession`.

Persisting selected context is optional for Phase 1; persisting proposals with sessions is acceptable if small.

- [ ] **Step 3: Extend Tauri wrappers**

If not already done in Task 2/3, add wrappers for:

- `createForgeWikiUpdateProposal`
- `acceptForgeWikiUpdateProposal`
- `discardForgeWikiUpdateProposal`

- [ ] **Step 4: Update `WikiSections` UI**

Modify `src/components/context/WikiSections.tsx`.

Required UI sections, in this order:

1. `项目记录`
   - If no project path: `打开项目后可以建立项目 Wiki`
   - If no Wiki: `还没有项目 Wiki` + button `建立项目 Wiki`
   - If Wiki exists: list pages with title, path, summary.

2. `本轮带入`
   - Show selected Forge Wiki pages and selected Context Memory.
   - Forge Wiki label should say `项目记录` instead of raw `wiki`.

3. `建议更新项目记录`
   - Show pending proposals.
   - Buttons: accept (`Check`) and discard (`X`).
   - Show target pages and summary.

4. Existing Context Memory sections:
   - Rename `项目 Wiki` memory section to `上下文记忆` to avoid confusion with real Forge Wiki.

Do not make the panel wider. Keep text truncation and `break-words`.

- [ ] **Step 5: Run build and commit**

Run:

```bash
npm run build
```

Expected: PASS.

Commit:

```bash
git add src/lib/protocol.ts src/lib/tauri.ts src/store/index.ts src/components/context/WikiSections.tsx
git commit -m "feat: show forge wiki in context panel"
```

## Task 5: E2E And Mock IPC

**Owner:** E2E worker  
**Write scope:** `e2e/mock-ipc.ts`, `e2e/frontend.spec.ts`.

- [ ] **Step 1: Add failing e2e test**

Add Playwright test:

```ts
test("Forge Wiki context panel initializes wiki and shows selected pages", async ({ page }) => {
  // create session
  // open right Context panel
  // assert empty state "还没有项目 Wiki"
  // click "建立项目 Wiki"
  // assert default page "当前任务" or "项目概览"
  // emit forge_wiki_context_selected
  // assert "已带入" or selected page appears
  // emit forge_wiki_update_proposed
  // assert "建议更新项目记录"
});
```

Run:

```bash
npx playwright test e2e/frontend.spec.ts --grep "Forge Wiki"
```

Expected: FAIL until mocks/UI are wired.

- [ ] **Step 2: Extend mock IPC**

Modify `e2e/mock-ipc.ts`:

- mock `get_forge_wiki_state`
- mock `init_forge_wiki`
- mock `list_forge_wiki_pages`
- mock `read_forge_wiki_page`
- mock `select_forge_wiki_context`
- mock proposal accept/discard commands if UI calls them.

- [ ] **Step 3: Run focused e2e and full frontend e2e**

Run:

```bash
npx playwright test e2e/frontend.spec.ts --grep "Forge Wiki"
npx playwright test e2e/frontend.spec.ts
```

Expected: PASS.

Commit:

```bash
git add e2e/mock-ipc.ts e2e/frontend.spec.ts
git commit -m "test: cover forge wiki context panel"
```

## Task 6: Final Integration Review And Verification

**Owner:** Controller plus review subagents  
**Write scope:** no new feature code unless reviews find issues.

- [ ] **Step 1: Spec compliance review**

Dispatch reviewer with:

- `docs/superpowers/specs/2026-05-13-forge-wiki-design.md`
- this plan
- final diff

Reviewer must check:

- `.forge/wiki/` initialization exists.
- six default Markdown pages exist.
- selected Wiki pages are emitted and visible.
- proposal accept/discard exists.
- sensitive content is rejected.
- TypeScript and Rust protocol events match.

- [ ] **Step 2: Code quality review**

Reviewer must check:

- path traversal protection
- no accidental scan of ignored directories
- no hidden Wiki writes without proposal
- no UI overflow-prone text containers
- no stale session state leaks
- no unrelated refactors

- [ ] **Step 3: Final verification**

Run:

```bash
cargo test forge_wiki --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npm run build
npx playwright test e2e/frontend.spec.ts
git diff --check
```

Expected: all PASS.

- [ ] **Step 4: Final status**

Report:

- commits created
- tests run
- known limitations
- safe manual test prompts
- branch status

