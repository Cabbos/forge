# Harness Architecture Refactor — Design Spec

## Context

DeepSeek Agent 当前 harness（`src-tauri/src/harness/`）已接入 Agent Loop 的工具执行链路，但整体设计是"单向接入"：Harness 只参与了 `execute_tool()` 的 Hooks + Permission 拦截，SkillLoader 扫描了 SKILL.md 但 system prompt 不走它，MCP 完全没接，permission 规则不持久化。HubPanel 前端显示的是硬编码假数据。

## Goal

将 harness 重构为完整的可插拔能力平台：Skills 可安装/可配置，Hooks 可编辑，MCP server 可接入，Permission 规则可持久化，HubPanel 显示真实数据。

## Architecture

### Core Abstractions

**EventBus** — 所有子系统通过事件通信。用 `tokio::broadcast` 实现，多消费者，慢消费者不阻塞。

**Capability trait** — 统一抽象 Skills / Hooks / MCP / Tools：

```rust
#[async_trait]
pub trait Capability: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> CapabilityKind;
    fn metadata(&self) -> CapabilityMetadata;
    fn install(&mut self) -> Result<(), Error>;
    fn uninstall(&mut self) -> Result<(), Error>;
    fn enable(&mut self);
    fn disable(&mut self);
    fn is_enabled(&self) -> bool;
    fn configure(&mut self, config: serde_json::Value) -> Result<(), Error>;
    fn subscribed_events(&self) -> Vec<EventType>;
    async fn on_event(&self, event: &Event, bus: &EventBus) -> Result<(), Error>;
}
```

**CapabilityRegistry** — 统一管理所有 Capability：本地扫描 + 远程发现 + 安装/卸载 + 启用/禁用 + 配置。数据持久化到 SQLite（rusqlite），替代当前的纯内存 + SKILL.md 文件扫描方案。

**Harness** — 变薄：初始化 EventBus + Registry，注册内置 Capability，提供 `start_session()` / `process_message()` 两个入口。

### Data Flow

```
1. Session Start
   Harness.start_session() → EventBus.emit(SessionStart)
   → Skills/MCPs subscribe → system_prompt assembled → AgentSession created

2. User Message
   AgentSession.send_message() → API stream → parse ToolCalls

3. Tool Execution (per ToolCall)
   EventBus.emit(PreTool { name, input })
   → HookEngine.on_event() → modify/block
   → PermissionGate.check() → confirm if needed
   → Tool.execute(input) → result
   → EventBus.emit(PostTool { name, result })
   → HookEngine.on_event() → audit/modify result

4. Runtime Capability Change
   HubPanel user action → IPC → Registry.toggle/enable/install
   → Registry updates SQLite → EventBus.emit(CapabilityChanged)
```

### Migration Plan

**Phase 1: Capability trait + Registry** (3-4 changesets)
- New: `harness/capability.rs` — trait + CapabilityKind enum
- New: `harness/registry.rs` — scan, install, toggle, persist to SQLite
- New: `harness/capabilities/` — builtin Capability implementations
- Modify: `harness/skills.rs`, `harness/hooks.rs` — implement Capability trait
- Modify: `executor/` tools — become Capability implementations
- Add: `rusqlite` to Cargo.toml for Registry persistence
- Verify: `cargo build` passes, Registry scans local skills

**Phase 2: EventBus replaces direct calls** (2-3 changesets)
- Rewrite: `harness/event_bus.rs` — tokio::broadcast, multi-consumer
- Modify: `agent/session.rs` — tool execution via EventBus::emit + wait
- Modify: `harness/mod.rs` — wire EventBus, subsystems subscribe
- Verify: existing behavior unchanged (Agent loop still works)

**Phase 3: HubPanel real data + IPC** (3-4 changesets)
- New IPC: `list_capabilities`, `install_capability`, `toggle_capability`, `configure_capability`
- Frontend: HubPanel tabs consume real IPC data
- Fix: InputBar Enter + scroll bugs
- Verify: `npm run tauri dev`, HubPanel shows real skills/hooks/MCP

## Key Decisions

- **SQLite over file-based**: Registry needs transactional update + query, SKILL.md alone can't represent installed/configured state
- **EventBus at tool-execution level only**: Don't over-engineer — session lifecycle events go through Bus, but streaming chunks stay in AgentSession's direct emit
- **Registry scan is additive**: First run scans local dirs, subsequent changes via IPC. No polling — HubPanel refreshes on capability-changed events
- **Old plugin_manager module**: Remove in Phase 1. Replaced by Registry.

## Non-Goals

- Remote MCP server management (stdio only for Phase 1-2)
- Skill marketplace UI in frontend (Registry scan first, discover later)
- Hook script editor (Phase 3 minimum)
- Concurrent multi-session event isolation (single session for now)
