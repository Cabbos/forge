# Harness 模块审计文档

> 生成时间: 2025-07-18  
> 源码路径: `src-tauri/src/harness/`

---

## 1. 模块概述

`harness` 是 AI Agent 的**核心编排层**，负责将所有子系统串联起来，形成完整的工具执行管线。设计参考了 Claude Code 的 hooks/skills/permissions 模型，结合 Hermes 的 agent-centric 流式架构。

## 2. 文件结构

```
src-tauri/src/harness/
├── mod.rs              # 入口 + Harness 结构体（中心编排器）
├── capability.rs       # Capability trait + 元数据/事件类型定义
├── capabilities/       # 内建 Capability 实现
│   ├── mod.rs          # 子模块导出
│   ├── tools.rs        # FileToolCap, WriteFileToolCap, ShellToolCap, SearchToolCap
│   ├── skills.rs       # SkillLoaderCap（Skill 加载器封装）
│   └── hooks.rs        # [stub] 预留
├── registry.rs         # CapabilityRegistry（能力注册表 + DB 持久化）
├── db.rs               # Database（SQLite: capabilities 表 + permission_rules 表）
├── permissions.rs      # PermissionGate（权限门控：模式匹配 + 会话缓存 + DB）
├── hooks.rs            # HookEngine + Hook trait + 内建 Hook（LoggingHook, FileSystemAuditHook）
├── event_bus.rs        # EventBus（Tauri 事件发射封装）
└── skills.rs           # SkillLoader（SKILL.md 扫描/加载/管理）
```

## 3. 架构分层

```
┌─────────────────────────────────────────────────────┐
│                   Harness (mod.rs)                   │
│  中心编排器：组件装配 + 生命周期管理 + 工具执行管线     │
└──────────┬──────────────────────────────────────────┘
           │
   ┌───────┼───────────┬──────────────┬──────────────┐
   ▼       ▼           ▼              ▼              ▼
┌────┐ ┌───────┐ ┌──────────┐ ┌───────────┐ ┌────────────┐
│Hook│ │Skill  │ │Permission│ │Capability │ │ EventBus   │
│Eng │ │Loader │ │Gate      │ │Registry   │ │            │
│    │ │       │ │          │ │ + DB      │ │ Tauri emit │
└────┘ └───────┘ └──────────┘ └───────────┘ └────────────┘
```

### 3.1 工具执行管线 (`execute_tool`)

```
用户/AI 发起工具调用
        │
        ▼
┌──────────────────┐
│ 1. Pre-tool Hook │  → HookDecision::Proceed(modified_input)
│    (HookEngine)  │  → HookDecision::Block(reason) → 终止
└──────┬───────────┘
       ▼
┌──────────────────┐
│ 2. Permission    │  → 已批准 → 继续
│    Gate Check    │  → 未批准 → emit ConfirmAsk → 等待用户 120s
└──────┬───────────┘
       ▼
┌──────────────────┐
│ 3. Tool Executor │  → 实际执行 (文件/Shell/搜索/Web)
└──────┬───────────┘
       ▼
┌──────────────────┐
│ 4. Post-tool Hook│  → 修改/审计结果
└──────────────────┘
```

## 4. 各模块详情

### 4.1 `capability.rs` — 能力抽象层
- **`Capability` trait**: 异步 trait，定义 `id`, `metadata`, `enabled`, `subscribed_events`, `on_event`
- **`CapabilityKind` 枚举**: `Skill | Hook | McpServer | Tool`
- **`Event` 枚举**: `SessionStart`, `SessionStop`, `PreTool`, `PostTool`, `CapabilityChanged`
- 所有能力通过事件驱动方式与系统交互

### 4.2 `capabilities/tools.rs` — 内建工具能力
注册了 4 个核心工具能力：
| 能力 | ID | 说明 |
|------|----|------|
| `FileToolCap` | `read_file` | 文件读取 |
| `WriteFileToolCap` | `write_to_file` | 文件创建/覆写 |
| `ShellToolCap` | `run_shell` | Shell 命令执行 |
| `SearchToolCap` | `search_files` | 文件搜索 |

### 4.3 `capabilities/skills.rs` — Skill 加载能力
- `SkillLoaderCap`: 将 `SkillLoader` 包装为 `Capability`
- 订阅 `SessionStart` 事件，会话开始时自动扫描 SKILL.md

### 4.4 `registry.rs` — 能力注册表
- **`CapabilityRegistry`**: 内存 `Vec<Box<dyn Capability>>` + `RwLock` 保护
- 注册时自动持久化到 SQLite `capabilities` 表
- 支持 CRUD + toggle + 事件分发 (`dispatch_event`)

### 4.5 `db.rs` — SQLite 数据库
- **两张表**:
  - `capabilities`: id, name, description, version, source, kind, enabled, config_json
  - `permission_rules`: id, tool_name, approved, created_at
- 使用 `rusqlite` bundled 模式，数据文件位于 `{working_dir}/.ai-studio/registry.db`
- 所有操作通过 `Mutex<Connection>` 同步访问

### 4.6 `permissions.rs` — 权限门控
- **`PermissionGate`**: 三层权限检查
  1. **DB 持久化规则** — `permission_rules` 表中 approved=1 的记录
  2. **全局模式白名单** — 预批准: `read_file`, `list_directory`, `search_files`, `search_content`, `web_search`, `web_fetch`
  3. **会话缓存** — 用户在当前会话中批准的临时许可
- 关键区分：`write_to_file`/`edit_file`/`run_shell` **不在**预批准列表中，必须弹窗确认
- 超时机制：等待用户确认最多 120 秒

### 4.7 `hooks.rs` — Hook 引擎
- **`Hook` trait**: `on_pre_tool` 返回 `HookDecision::Proceed | Block`
- **`HookEngine`**: `RwLock<Vec<Arc<dyn Hook>>>` 存储
- 内建两个 Hook:
  - `LoggingHook`: 记录 PreTool/PostTool 日志
  - `FileSystemAuditHook`: 审计文件写入和 Shell 执行（仅 PostTool）
- Pre-tool hooks 在锁外 await，避免死锁

### 4.8 `event_bus.rs` — 事件总线
- 封装 Tauri 的 `emit("session-output", event)`
- 提供类型安全的事件构造方法：`thinking_start/chunk/end`, `text_*`, `tool_*`, `shell_*`, `confirm_ask`, `session_*`, `error`, `usage`
- 使用 `Arc<Mutex<Option<AppHandle>>>` 懒绑定 AppHandle

### 4.9 `skills.rs` — Skill 加载器
- 扫描目录: `~/.ai-studio/skills/` 和可执行文件旁的 `skills/`
- 解析 SKILL.md 文件的 description + tools 元数据
- 支持 GitHub 来源的 Skill（`SkillSource::GitHub` 枚举已定义但尚未实现加载逻辑）

### 4.10 `mod.rs` — 中心编排器
- **`Harness` 结构体**：组装所有子系统
- 构造函数按顺序：
  1. 创建 `HookEngine`, `SkillLoader`, `EventBus`, `ToolExecutor`
  2. 打开/迁移 SQLite 数据库
  3. 创建 `PermissionGate` + `CapabilityRegistry`
  4. 注册 5 个内建能力 + 2 个内建 Hook
- 核心方法：
  - `build_system_prompt()`: 拼接 base prompt + 启用的 skills 指令
  - `execute_tool()`: 完整的 Hook → Permission → Execute → Hook 管线

## 5. 值得关注的设计点

### 5.1 并发安全
- `PermissionGate` 使用 `tokio::sync::RwLock`（异步锁）
- `HookEngine`, `CapabilityRegistry`, `Database` 使用 `std::sync::RwLock/Mutex`（同步锁）
- 混用两种锁类型需要注意：**不能在持有 std 锁时调用 .await**

### 5.2 unwrap 使用
模块内存在约 **30 处** 纯 `unwrap()`，主要集中在：
- Mutex/RwLock 的 `lock().unwrap()` — 相对安全（std::sync 锁不会 poison unless panic）
- `Database::open().expect()` — 启动时失败即 panic，属于 fail-fast 策略

### 5.3 权限模型
- 安全工具（只读）默认放行
- 危险工具（写入/Shell）必须用户确认
- 支持"永久批准"（写入 DB）和"会话批准"（内存缓存）
- 120 秒超时防止无限等待

## 6. 待完善

- `capabilities/hooks.rs` 目前是 stub，尚未实现 Hook 作为 Capability 的注册
- `SkillSource::GitHub` 分支已定义但未实现远程加载
- `SkillLoader` 的 tools.json 解析未实现（`parse_skill_metadata` 返回空 Vec）
- 没有单元测试覆盖
