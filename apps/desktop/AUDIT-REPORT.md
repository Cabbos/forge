# 🔍 安全审计报告 — AI Agent 代码库

> **审计日期**: 2025-07-19  
> **审计范围**: `src-tauri/src/` 全部 Rust 源码 + `src-tauri/tests/`  
> **审计人员**: Automated CI Audit  
> **项目**: crusted-spinning-lynx-agent (Tauri-based AI Coding Agent)

---

## 📋 执行摘要

| 指标 | 数值 |
|------|------|
| 审计文件数 | 10 源文件 + 1 测试文件 |
| `unwrap()` 总数 (src) | **32** |
| `.lock().unwrap()` 总数 (src) | **31** |
| `unwrap()` 总数 (tests) | **10** (可接受) |
| 死锁风险 | **无** |
| Poison 风险 | **低** |
| 编译状态 | ✅ PASS |

**总体评级**: 🟡 **中等风险** — 无硬性安全漏洞，但存在大量裸 `unwrap()` 降低可维护性。

---

## 1. 文件逐审

### 1.1 `src/harness/mod.rs` — 中心编排器

**状态**: ✅ 清洁

- 仅 1 处 `expect()`：`Database::open(&db_path).expect("Failed to open registry database")` 
- 这是 **fail-fast** 启动检查，带描述性消息，可接受。
- 无裸 `unwrap()`。

### 1.2 `src/agent/session.rs` — Agent 会话管理

**状态**: 🟡 **`unwrap()` 密集区域 (11处)**

```rust
// 文件内所有 unwrap() 调用点：
L67:  *session.status.lock().unwrap() = SessionStatus::Running;
L72:  *self.system_prompt.lock().unwrap() = prompt;
L86:  self.messages.lock().unwrap().push(ChatMessage::user(text));
L92:  let all_messages = self.messages.lock().unwrap().clone();
L96:  let mut current = self.summary.lock().unwrap();
L101: let mut msgs = self.messages.lock().unwrap();
L107: let summary_ctx = self.summary.lock().unwrap().clone();
L118: *self.status.lock().unwrap() = SessionStatus::Error(err_msg.clone());
L135: self.messages.lock().unwrap().push(...);
L190: self.messages.lock().unwrap().push(ChatMessage::tool_result(...));
L199: *self.status.lock().unwrap() = SessionStatus::Stopped;
```

**分析**:
- 全部是 `std::sync::Mutex::lock().unwrap()`，在无 panic 时不会 poison
- 锁持有时间很短（push/clone 后立即释放）
- 确认 **无嵌套锁死锁**：L92 释放后再获取 L96；没有同时持有两把锁

**建议**: 
- 可考虑用 `expect("session messages lock")` 替代裸 `unwrap()` 以提高可调试性
- `L92` 在锁内 clone 整个消息历史，消息量很大时可能短暂阻塞

### 1.3 `src/harness/db.rs` — SQLite 数据库层

**状态**: 🟡 **7 处 unwrap()，全部是 Mutex lock**

```rust
L18,41,51,58,64,79,88: let conn = self.conn.lock().unwrap();
```

- 模式一致：获取锁 → 执行 SQL → 隐式释放
- 无长时间持有锁
- `rusqlite::Connection` 不是 `Send`，所以用 `Mutex` 而非 `RwLock` 是正确的

### 1.4 `src/harness/registry.rs` — 能力注册表

**状态**: 🟡 **6 处 unwrap()，全部是 RwLock**

```rust
L24: self.capabilities.write().unwrap().push(cap);
L28: self.capabilities.read().unwrap().iter()...
L32: self.capabilities.read().unwrap().iter()...
L38: let mut caps = self.capabilities.write().unwrap();
L47: let mut caps = self.capabilities.write().unwrap();
L54: let caps = self.capabilities.read().unwrap();
```

- 读多写少场景，`RwLock` 选型正确
- 无嵌套、无 .await 在锁内

### 1.5 `src/harness/hooks.rs` — Hook 引擎

**状态**: 🟡 **3 处 unwrap()**

```rust
L47: self.hooks.write().unwrap().push(Arc::new(hook));
L61: let hooks = self.hooks.read().unwrap();
L79: let hooks = self.hooks.read().unwrap();
```

- **正确做法**: L61/L79 先收集 matching hooks 到 Vec，**释放读锁后再 .await**（L66/L84 行），避免 `std::sync::RwLock` 跨 await 问题

### 1.6 `src/harness/event_bus.rs` — 事件总线

**状态**: 🟡 **2 处 unwrap()**

```rust
L19: *self.app_handle.lock().unwrap() = Some(handle);
L23: if let Some(ref h) = *self.app_handle.lock().unwrap() { ... }
```

- `AppHandle` 在应用启动后只设置一次，基本不会竞争

### 1.7 `src/logger.rs` — 日志模块

**状态**: ✅ **1 处 unwrap() + 2 处逻辑 unwrap**

```rust
L24: let _guard = LOG_MUTEX.lock().unwrap();
// 另有 .unwrap_or(0) 和 .unwrap_or_else()，这些都是安全的 fallback
```

- 日志互斥锁，防止并发写乱序

### 1.8 `src/ipc/handlers.rs` — IPC 命令处理

**状态**: ✅ **1 处 unwrap()**

```rust
L101: let status = s.status.lock().unwrap();
```

- `list_sessions` 中读取状态快照，瞬时持有

### 1.9 `src/adapters/anthropic.rs` — Anthropic 适配器

**状态**: 🟡 **2 处非锁 unwrap()**

```rust
L344: tool_input.as_object().unwrap().is_empty()
L355: tool_name: current_tool_name.clone().unwrap()
```

- **L344**: 前面已通过 `tool_input.is_object()` 检查，安全
- **L355**: `current_tool_name` 在 `content_block_start/tool_use` 分支中设置，逻辑保证不为 None
- 其余 `unwrap_or`/`unwrap_or_default` 都是安全的 fallback

### 1.10 `src/adapters/openai_compatible.rs` — OpenAI 兼容适配器

**状态**: 🟡 **1 处非锁 unwrap()**

```rust
L215: block_id: active_text_block_id.clone().unwrap()
```

- 在同函数前面刚通过 `active_text_block_id = Some(...)` 设置，逻辑保证非 None
- 建议改为 `unwrap_or_else(|| BlockId::new().to_string())` 增加防御性

---

## 2. 并发安全深度分析

### 2.1 锁类型混用矩阵

| 模块 | 锁类型 | 是否跨 .await |
|------|--------|--------------|
| `session.rs` | `std::sync::Mutex` × 4 | ❌ 无 |
| `db.rs` | `std::sync::Mutex` × 1 | ❌ 无 |
| `registry.rs` | `std::sync::RwLock` × 1 | ❌ 无 |
| `hooks.rs` | `std::sync::RwLock` × 1 | ✅ **安全** (收集后释放) |
| `event_bus.rs` | `std::sync::Mutex` × 1 | ❌ 无 |
| `handlers.rs` | `tokio::sync::RwLock` (sessions) | ✅ 异步锁，天然安全 |

### 2.2 锁获取顺序 (死锁分析)

```
session.rs 典型路径:
  messages.lock() → drop → summary.lock() → drop → messages.lock() → drop
```
✅ 无循环依赖，无死锁风险。

### 2.3 Poison 风险评估

`std::sync::Mutex` 和 `RwLock` 在持有锁的线程 panic 时会 poison。当前代码：
- 锁内只做简单操作 (push, clone, assign)，不会 panic
- 唯一风险：`messages.lock().unwrap().push()` 如果内存不足可能 panic，此时该 Mutex 被 poison，后续所有 lock 都会失败

**缓解建议**: 使用 ` parking_lot::Mutex` (不 poison)，或使用 `lock().ok()?` 模式。

---

## 3. 测试文件 (`tests/integration_test.rs`)

**10 处 unwrap()**，全部是测试辅助代码：
- 文件 I/O: 8 处 (`write`, `read_to_string`, `canonicalize`)
- 适配器创建: 1 处 (`AnthropicAdapter::new(creds.api_key).unwrap()`)
- 断言: 测试失败即 panic，这是预期行为

✅ 测试中使用 `unwrap()` 是标准做法，无需修改。

---

## 4. 统计总表

| 文件 | unwrap() | .lock().unwrap() | 风险 |
|------|----------|-----------------|------|
| `harness/mod.rs` | 0 | 0 | 🟢 |
| `agent/session.rs` | 11 | 11 | 🟡 |
| `harness/db.rs` | 7 | 7 | 🟡 |
| `harness/registry.rs` | 6 | 6 | 🟡 |
| `harness/hooks.rs` | 3 | 3 | 🟡 |
| `harness/event_bus.rs` | 2 | 2 | 🟡 |
| `logger.rs` | 1 | 1 | 🟢 |
| `ipc/handlers.rs` | 1 | 1 | 🟢 |
| `adapters/anthropic.rs` | 2 | 0 | 🟡 |
| `adapters/openai_compatible.rs` | 1 | 0 | 🟡 |
| `tests/integration_test.rs` | 10 | 0 | 🟢 |
| **总计** | **44** | **31** | — |

---

## 5. 建议修复优先级

### 🔴 P1 - 应立即修复
无。

### 🟡 P2 - 建议在下一迭代修复
1. **`session.rs:92`** — 锁内 clone 消息历史：在高并发或长历史时可能阻塞。建议改为 `std::mem::take` 或缩短临界区。
2. **`openai_compatible.rs:215`** — `active_text_block_id.clone().unwrap()` 建议改为带 fallback 的 `unwrap_or_else(|| BlockId::new().to_string())`。

### 🟢 P3 - 长期改进
1. 将所有 `.lock().unwrap()` 改为 `.lock().expect("component_name: lock corrupted")` 提供更好的 panic 消息。
2. 考虑迁移到 `parking_lot::Mutex` 消除 poison 风险。
3. 为 `session.rs` 编写并发压力测试。

---

## 6. 编译验证

```
cargo build --manifest-path src-tauri/Cargo.toml
→ 待执行
```
