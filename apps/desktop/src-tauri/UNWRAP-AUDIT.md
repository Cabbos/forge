# unwrap() 审计报告

> 生成日期: 2025-01-15
> 审计范围: `src-tauri/src/` 下所有 `.rs` 文件中包含 `unwrap()` 的行

---

## 一、项目依赖 (`src-tauri/Cargo.toml`)

| 依赖 | 版本/特性 |
|------|-----------|
| `tauri` | 2 (features: tray-icon) |
| `tauri-plugin-shell` | 2 |
| `serde` | 1 (features: derive) |
| `serde_json` | 1 |
| `portable-pty` | 0.8 |
| `vt100` | 0.15 |
| `tokio` | 1 (features: full) |
| `uuid` | 1 (features: v7) |
| `regex` | 1 |
| `log` | 0.4 |
| `env_logger` | 0.11 |
| `thiserror` | 2 |
| `reqwest` | 0.12 (features: stream, json) |
| `tokio-stream` | 0.1 |
| `futures` | 0.3 |
| `async-trait` | 0.1 |
| `rusqlite` | 0.31 (features: bundled) |

---

## 二、`unwrap()` 调用清单

### 2.1 `src/agent/session.rs` — 12 处

| 行号 | 代码 |
|------|------|
| 67 | `*session.status.lock().unwrap() = SessionStatus::Running;` |
| 72 | `*self.system_prompt.lock().unwrap() = prompt;` |
| 85 | `self.messages.lock().unwrap().len()` |
| 88 | `self.messages.lock().unwrap().push(ChatMessage::user(text));` |
| 94 | `let all_messages = self.messages.lock().unwrap().clone();` |
| 98 | `let mut current = self.summary.lock().unwrap();` |
| 103 | `let mut msgs = self.messages.lock().unwrap();` |
| 109 | `let summary_ctx = self.summary.lock().unwrap().clone();` |
| 133 | `self.messages.lock().unwrap().push(...)` |
| 194 | `self.messages.lock().unwrap().push(ChatMessage::tool(...))` |
| 203 | `let mut msgs = self.messages.lock().unwrap().clone();` |
| 218 | `*self.status.lock().unwrap() = SessionStatus::Stopped;` |

### 2.2 `src/harness/db.rs` — 7 处

| 行号 | 代码 |
|------|------|
| 18 | `let conn = self.conn.lock().unwrap();` |
| 41 | `let conn = self.conn.lock().unwrap();` |
| 51 | `let conn = self.conn.lock().unwrap();` |
| 58 | `let conn = self.conn.lock().unwrap();` |
| 64 | `let conn = self.conn.lock().unwrap();` |
| 79 | `let conn = self.conn.lock().unwrap();` |
| 88 | `let conn = self.conn.lock().unwrap();` |

### 2.3 `src/harness/registry.rs` — 6 处

| 行号 | 代码 |
|------|------|
| 24 | `self.capabilities.write().unwrap().push(cap);` |
| 28 | `self.capabilities.read().unwrap().iter()...` |
| 32 | `self.capabilities.read().unwrap().iter()...` |
| 38 | `let mut caps = self.capabilities.write().unwrap();` |
| 47 | `let mut caps = self.capabilities.write().unwrap();` |
| 54 | `let caps = self.capabilities.read().unwrap();` |

### 2.4 `src/harness/hooks.rs` — 3 处

| 行号 | 代码 |
|------|------|
| 47 | `self.hooks.write().unwrap().push(Arc::new(hook));` |
| 61 | `let hooks = self.hooks.read().unwrap();` |
| 79 | `let hooks = self.hooks.read().unwrap();` |

### 2.5 `src/harness/event_bus.rs` — 2 处

| 行号 | 代码 |
|------|------|
| 19 | `*self.app_handle.lock().unwrap() = Some(handle);` |
| 23 | `if let Some(ref h) = *self.app_handle.lock().unwrap() {` |

### 2.6 `src/adapters/anthropic.rs` — 2 处

| 行号 | 代码 |
|------|------|
| 344 | `&& !tool_input.as_object().unwrap().is_empty()` |
| 355 | `tool_name: current_tool_name.clone().unwrap(),` |

### 2.7 `src/adapters/openai_compatible.rs` — 1 处

| 行号 | 代码 |
|------|------|
| 221 | `block_id: active_text_block_id.clone().unwrap()` |

### 2.8 `src/ipc/handlers.rs` — 1 处

| 行号 | 代码 |
|------|------|
| 101 | `let status = s.status.lock().unwrap();` |

### 2.9 `src/executor/mod.rs` — 3 处

| 行号 | 代码 |
|------|------|
| 244 | `if let Some(cached) = CACHE.lock().unwrap().get(query) {` |
| 252 | `let _ = CACHE.lock().unwrap().insert(...)` |
| 257 | `let _ = CACHE.lock().unwrap().insert(...)` |

### 2.10 `src/logger.rs` — 1 处

| 行号 | 代码 |
|------|------|
| 24 | `let _guard = LOG_MUTEX.lock().unwrap();` |

---

## 三、汇总统计

| 文件 | `unwrap()` 次数 | 类型（Mutex/RwLock/其他） |
|------|:-------------:|--------------------------|
| `agent/session.rs` | 12 | 全部 `Mutex::lock().unwrap()` |
| `harness/db.rs` | 7 | 全部 `Mutex::lock().unwrap()` |
| `harness/registry.rs` | 6 | 全部 `RwLock::write/read().unwrap()` |
| `harness/hooks.rs` | 3 | 全部 `RwLock::write/read().unwrap()` |
| `executor/mod.rs` | 3 | `Mutex::lock().unwrap()` |
| `harness/event_bus.rs` | 2 | `Mutex::lock().unwrap()` |
| `adapters/anthropic.rs` | 2 | 非锁: `as_object().unwrap()`, `Option::unwrap()` |
| `adapters/openai_compatible.rs` | 1 | 非锁: `Option::unwrap()` |
| `ipc/handlers.rs` | 1 | `Mutex::lock().unwrap()` |
| `logger.rs` | 1 | `Mutex::lock().unwrap()` |
| **总计** | **38** | |

### 风险分布

- **Mutex/RwLock `lock().unwrap()`** — 共 35 处。std::sync 锁在被 poison 时才会 panic，通常情况下安全。建议加 `.expect("描述信息")` 提供诊断上下文。
- **`as_object().unwrap()` (anthropic.rs:344)** — 已前置 `is_object()` 检查，逻辑安全但写法冗余。
- **`Option::unwrap()` (anthropic.rs:355, openai_compatible.rs:221)** — 依赖事件顺序，SSE 乱序时可能 panic，风险较高。

---

## 四、建议

1. **高优先级**: `anthropic.rs:355` / `openai_compatible.rs:221` — 将 `.unwrap()` 改为 `unwrap_or("unknown")` 或 `unwrap_or_else(BlockId::new)` 防止乱序 panic。
2. **中优先级**: 全局所有 `lock().unwrap()` 替换为 `.expect("xxx mutex poisoned")` 提供 panic 上下文。
3. **低优先级**: `anthropic.rs:344` 合并 `is_object() + as_object().unwrap()` 为 `if let Some(obj) = tool_input.as_object()`。
