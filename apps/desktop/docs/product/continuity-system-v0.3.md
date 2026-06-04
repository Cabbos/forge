# Continuity / Experience System V0.3

Updated: 2026-06-03

## What Changed in V0.3

### 1. DB 物理位置改为项目自有
**问题**：之前所有 session 的 continuity 数据都写到 Forge 主项目的 `.forge/continuity.db`，用 `project_path` 列做逻辑隔离。测试项目自己的 `.forge/continuity.db` 是 0 字节，用户容易误以为"数据没存进去"。

**修复**：`ContinuityService` 改为 per-project DB manager。每个项目的数据库位于 `{project_path}/.forge/continuity.db`。
- `ContinuityService::store_for_project()` 按需打开/缓存各项目的 `ContinuityStore`
- `AppState::new()` 不再预创建单库，而是初始化空的管理器
- 前端 Continuity 面板新增一行 DB path 显示，避免误判

### 2. Experience Formation 质量收敛
**问题**：实库里出现大量低质量 candidate：
- 整段人工测试提示词被提成"用户偏好/项目事实"
- shell 输出明明 `TSC_EXIT: 0` / `TEST_EXIT: 0` 却仍被记成 Tool failed experience
- "继续/就行" 等短输入也能形成无意义经验

**修复**：
- **切断 prompt echo 源头**：`send_input_continuity.rs` 不再把 `extract_candidates_from_user_message` 产生的 memory candidate 混入 reflection lessons。 continuity lessons 只来自执行结果（tool failure、verification failure）。
- **增强 rejection 过滤器**：`should_reject_experience_lesson` 新增：
  - `is_too_short_for_lesson`：少于 15 字符直接丢弃
  - `looks_like_raw_user_prompt`：包含"我们现在在"/"请检查"/"人工测试"等原始 prompt 标记
  - `is_low_value_continuation`：纯"继续""就行"等 fragment
- **修复 shell 假失败检测**：`shell_failure_summary_looks_successful` 新增：
  - 识别 `TSC_EXIT: 0` / `TEST_EXIT: 0` / `EXIT: 0` 模式
  - 识别 npm test 成功输出（✅ + `=` 等）
  - 识别只读检查命令的正常 stdout（如 `sqlite3 .tables` / `SELECT`、`ls` / `file` / `wc -c`、`git status`），避免把 DB/文件检查结果提成失败经验
- **清理死代码**：移除不再使用的 `continuity_lessons_from_memory_candidates`

### 3. Review Event（人工确认事件）
**问题**：candidate -> accepted/pinned 只是 update status，没有留下用户反馈记录。

**修复**：
- 新增 `ContinuityEvent::ExperienceStatusChanged` 变体，字段：
  - `experience_id`, `old_status`, `new_status`
  - `session_id`, `project_path`, `timestamp_ms`
- `session_id` 表示执行 accept/pin/forget/archive 的当前审核 session；没有当前 session 时才回退到原始 experience 的 source session。
- `ContinuityService::update_experience_status` 在更新 status 后自动写入 review event
- 事件存入同一张 `continuity_events` 表（JSON 序列化，无需改 schema）

### 4. 闭环测试覆盖
新增/更新测试：
- `service_uses_per_project_db`：证明每个项目有独立物理 DB
- `service_records_review_event_on_status_change`：证明 status change 会写 review event
- `candidate_does_not_auto_inject`：证明 candidate 不会进入 recall
- `accepted_experience_forms_hidden_context`：证明 accepted/pinned 会进入 recall 并形成 hidden context
- `formation_rejects_raw_user_prompt_as_lesson`：证明 prompt echo 被过滤
- `formation_rejects_short_low_value_continuation`：证明短输入不形成经验
- `shell_false_failure_with_exit_zero_does_not_form_lesson`：证明 `EXIT: 0` 不形成假失败

## SQLite 验证结果

旧库（Forge 主项目）仍然保留 52 条 events + 11 条 candidate experiences：
- 所有 52 条 events 的 `project_path` 指向测试项目
- 11 条 experiences 中：3 条是 prompt echo，6 条是 shell 假失败，2 条是正常 lesson

新代码运行后：
- 新项目的数据将写入各自 `{project}/.forge/continuity.db`
- 不再产生 prompt-echo 和 shell 假失败经验
- 旧库数据不会自动迁移（设计选择：旧 candidate 质量不可信，建议在新项目重新积累）

## 测试命令

```bash
# Backend check（含 clippy + test）
npm run check:backend

# 只跑 Continuity 相关测试
cargo test --manifest-path src-tauri/Cargo.toml --test continuity_test

# 完整测试
cargo test --manifest-path src-tauri/Cargo.toml

# 前端 build
npm run build
```

## 仍留到下一版的风险

1. **旧库数据未迁移**：已有 11 条低质量 candidate 留在 Forge 主项目 DB。是否清理/迁移需产品决策。
2. **shell 成功检测仍可能漏网**：`shell_failure_summary_looks_successful` 是基于 heuristics 的，新型工具输出可能绕过检测。
3. ** reflection lesson 仍依赖模型输出质量**：formation 只能从 reflection 的 `lessons` 字段生成，如果模型本身输出低质量 lesson，过滤器无法完全挽救。
4. **Review event 未在前端展示**：当前 review event 只存 DB，前端 Continuity 面板尚未展示审核历史。
5. **per-project DB 无上限**：`stores: Mutex<HashMap>` 会无限缓存打开的 connection，长会话多项目场景下可能内存泄漏。
6. **FTS 搜索仍用 project_path 列过滤**：`ContinuityStore` 的 search/recall SQL 仍带 `project_path` 条件。虽然物理 DB 已隔离，但 schema 仍保留该列作为冗余安全网。
