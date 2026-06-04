# Continuity / Experience System V0.4

Updated: 2026-06-03

## What Changed in V0.4

### 1. Episode-Level Experience Compiler
**问题**：V0.3 的 formation 仍然基于原始 tool failure 字符串，产生大量低结构化、难复用的经验。实库里的 candidate 很多是 "Tool `run_shell` failed: command=..." 这种执行级碎片，而不是工程级可复用知识。

**修复**：将 formation 从 "raw tool failure → candidate" 升级为 "episode-level experience compiler"。

- **Episode 结构** (`continuity/episode.rs`)：
  - 聚合单次用户输入的完整执行上下文：`project_path`, `session_id`, `user_goal_summary`, `changed_files`, `tool_count`, `failed_tools`, `file_changes`, `verification_status`, `notable_failures`, `final_result_summary`
  - `build_episode_from_turn(&AgentTurnState) -> Episode` 从 turn 状态直接构建
  - 复用 V0.3 的 shell 假失败过滤逻辑（`EXIT: 0`、只读检查命令等）

- **Experience Compiler** (`continuity/compiler.rs`)：
  - `ExperienceCompiler::compile(&Episode) -> Vec<ExperienceMemory>` 生成 0–3 个结构化 candidate
  - **结构化 body 格式**：`Problem: ... Cause: ... Fix: ... Verified by: ... Applies when: ... Evidence: ...`
  - 过滤规则：
    - 无文件变化 → 空（纯检查/聊天不产生可复用经验）
    - 取消 → 空
    - 调试-only → 空（`.forge/continuity.db` 变动、`console.log` 探针等）
  - 自动生成 tags：`ext:rs`, `verified`, `has-failure` 等
  - Confidence 动态调整：验证通过 +0.08，失败 -0.10，多文件变更 +0.03
  - 最多 3 个 candidate，自动去重

- **Formation 路径变更**：
  - `send_input_continuity.rs` 在每次 turn 结束时调用 `build_episode_from_turn()` 并将 `Episode` 附加到 `ReflectionEvent`
  - `form_experiences_from_reflection()` 检查 `reflection.episode`：
    - 如果有 Episode → 使用 `ExperienceCompiler::compile()`
    - 如果没有 → 回退到 V0.3 的 legacy lesson-string 路径（兼容旧 DB 数据）

### 2. V0.3 修复回顾（已在工作树中）

#### 2.1 Per-Project DB
- `ContinuityService::store_for_project()` 按需打开 `{project_path}/.forge/continuity.db`
- 每个项目有独立物理 DB，不再混用 Forge 主项目 DB
- `AppState::new()` 不再预创建单库，而是初始化空的管理器

#### 2.2 Formation 质量收敛
- **切断 prompt echo**：`send_input_continuity.rs` 不再把 `extract_candidates_from_user_message` 的 memory candidate 混入 reflection lessons
- **增强 rejection 过滤器**：`should_reject_experience_lesson` 新增 `is_too_short_for_lesson`（< 15 字符）、`looks_like_raw_user_prompt`、`is_low_value_continuation`
- **修复 shell 假失败检测**：识别 `EXIT: 0`、`TSC_EXIT: 0`、`TEST_EXIT: 0`、npm test 成功模式、只读检查命令
- **Review Event**：`ExperienceStatusChanged` 记录 `experience_id`, `old_status`, `new_status`, `session_id`, `project_path`, `timestamp_ms`

## 模块职责拆分

| 文件 | 行数 | 职责 |
|---|---|---|
| `continuity/mod.rs` | ~120 | 核心类型定义：`ContinuityEvent`, `ReflectionEvent`, `ExperienceMemory`, `ExperienceKind`, `ExperienceStatus`；formation 入口 `form_experiences_from_reflection()` |
| `continuity/episode.rs` | ~440 | `Episode` 结构体、`build_episode_from_turn()`、shell 假失败检测复用逻辑 |
| `continuity/compiler.rs` | ~680 | `ExperienceCompiler`、结构化 body 生成、episode 过滤、confidence 计算、tag 生成 |
| `continuity/service.rs` | ~150 | `ContinuityService`、per-project DB 管理、event 记录、experience formation/recall/search |
| `continuity/store.rs` | ~490 | `ContinuityStore`、SQLite 操作、migration、FTS5 索引、prune 逻辑 |
| `continuity/turn_adapters.rs` | ~500 | `continuity_events_from_turn()`、`continuity_lessons_from_turn()`、`build_send_input_reflection_event()` |
| `ipc/send_input_continuity.rs` | ~150 | `send_input` 结束时的 continuity 事件记录、episode 附加、experience formation 触发 |

## 测试覆盖

### 新增测试
- `episode_captures_changed_files_and_tool_counts` — Episode 正确捕获文件变更和工具统计
- `episode_skips_false_positive_failures` — EXIT: 0 假失败被排除在 notable_failures 外
- `episode_without_file_changes_has_empty_changed_files` — 无文件变更时 Episode 为空
- `compiler_returns_empty_for_no_file_changes` — 无文件变化不产生经验
- `compiler_returns_empty_for_cancelled_turn` — 取消不产生经验
- `compiler_returns_empty_for_debugging_only` — 调试-only 不产生经验
- `compiler_produces_structured_body_with_all_sections` — 结构化 body 包含全部 6 个 section
- `compiler_produces_failure_pattern_when_tools_fail` — 失败场景生成 BugPattern
- `compiler_produces_multiple_candidates_for_complex_episode` — 复杂 episode 生成多个 candidate
- `compiler_caps_at_three_candidates` — 上限为 3 个
- `episode_based_formation_produces_structured_experience` — 集成：有 episode 的 reflection 使用 compiler
- `episode_formation_skips_no_file_changes` — 集成：无文件变化的 episode 返回空
- `episode_formation_produces_bug_pattern_for_failed_tools` — 集成：失败工具生成 BugPattern
- `legacy_reflection_without_episode_still_forms_lessons` — 集成：旧 reflection 向后兼容

### 保留的 V0.3 测试
- `service_uses_per_project_db` — 每个项目有独立物理 DB
- `service_records_review_event_on_status_change` — status change 会写 review event
- `candidate_does_not_auto_inject` — candidate 不会进入 recall
- `accepted_experience_forms_hidden_context` — accepted/pinned 会进入 recall 并形成 hidden context
- `formation_rejects_raw_user_prompt_as_lesson` — prompt echo 被过滤
- `formation_rejects_short_low_value_continuation` — 短输入不形成经验
- `shell_false_failure_with_exit_zero_does_not_form_lesson` — EXIT: 0 不形成假失败

## 验证结果

```bash
# 完整后端检查（fmt + clippy -D warnings + test）
npm run check:backend

# 只跑 Continuity 相关测试
cargo test --manifest-path src-tauri/Cargo.toml -- continuity
cargo test --manifest-path src-tauri/Cargo.toml --test continuity_test

# 前端 build
npm run build
```

结果：
- `cargo fmt` 通过
- `cargo clippy --all-targets -- -D warnings` 通过
- `cargo test` **644 个测试全部通过**（559 lib + 26 continuity integration + 49 harness + 10 integration）
- `npm run build` 前端构建通过

## SQLite 当前状态

| 指标 | Forge 主项目 | 测试项目 |
|---|---|---|
| DB 路径 | `.forge/continuity.db` | `continuity-manual-test-app/.forge/continuity.db` |
| 大小 | 156K | 208K |
| events | 52 | 118 |
| experiences | 11 candidate (V0.2 旧数据) | 1 accepted, 4 archived, 3 candidate, 1 pinned |
| review events | 0 | 0 |

**关键发现**：
- per-project DB 已生效，测试项目数据写到了自己的 DB
- Forge 主库仍保留 11 条 V0.2 低质量 candidate（3 条 prompt echo，6 条 shell 假失败，2 条正常 lesson）
- 测试项目库里有 3 条 candidate 仍是以 `"Tool 'run_shell' failed: ..."` 形式存在的工具失败经验，是 V0.3 旧路径遗留
- 两个库均无 review event（前端尚未触发 accept/pin/forget 操作）

## 仍留到下一版的风险

1. **旧库数据未迁移**：Forge 主项目 DB 里的 11 条 V0.2 candidate 不会自动清理，长期可能误导搜索
2. **测试项目仍有旧路径经验**：3 条 sqlite3 假失败 candidate 是 V0.3/V0.4 切换前写入的，建议手动 archive/forget
3. **Review event 未在前端展示**：当前 review event 只存 DB，前端 Continuity 面板尚未展示审核历史
4. **per-project DB 连接无上限**：`stores: Mutex<HashMap>` 会无限缓存打开的 connection
5. **Episode 体积导致 ContinuityEvent 变大**：已加 `#[allow(clippy::large_enum_variant)]`，长期应考虑 boxing 或分离存储
6. **FTS 搜索仍用 project_path 列过滤**：虽然物理 DB 已隔离，但 schema 仍保留 `project_path` 作为冗余安全网
7. **Compiler 成功检测仍有漏网风险**：`is_debugging_only_episode` 是基于 heuristics 的，新型调试模式可能绕过
