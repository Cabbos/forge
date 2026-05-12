# Feature Prompts

## 1. 文件引用可点击跳转

### 需求

AI 输出中经常出现文件引用如 `src/agent/session.rs:123` 或 `` `/path/to/file.ts` ``，需要能点击直接打开。

### 要做的

**后端（Rust）：**
- 在工具结果里，文件路径用一种可识别的格式输出。已有的 `search_content` 返回 `file:line: content`，保持这个格式。
- 新增一个 Tauri IPC command `open_file(path: String, line: Option<u32>)`，用系统默认编辑器打开文件到指定行。
- macOS 用 `open -a "Visual Studio Code" --args -g {path}:{line}`，也可以配置编辑器。

**前端（React）：**
- 在 `TextBlock.tsx` 的 Markdown 渲染中，自动把 `file_path:line` 格式的文字变成可点击链接。
- 点击时调用 `openFile` IPC。
- 视觉：和普通链接区分开，可以用下划线 + 文件 icon。
- 同时匹配 `src/foo.rs:42` 和 `` `src/foo.rs:42` `` 两种写法。

---

## 2. Git Diff 工具

### 需求

Agent 修改文件后，用户需要直观看到改了什么，不用切终端跑 `git diff`。

### 要做的

**后端（Rust）：**
- 新增 tool `git_diff`，输出格式和 Claude Code 的 diff 类似：
  - 无参数：`git diff`（未 staged 改动）
  - `staged: true`：`git diff --cached`
  - `path: "src/foo.rs"`：只看某个文件
- 输出精简的 unified diff 格式，加号和减号分别用颜色标记。
- Agent 写文件后自动提示"可以用 git_diff 查看改动"。

**前端（React）：**
- 新增 `DiffCard.tsx` 组件渲染 diff 结果：
  - 行号列
  - `+` 行绿色背景，`-` 行红色背景
  - 单色字体（monospace）
  - 文件路径在顶部
- 在 `MessageList.tsx` / `BlockRenderer` 中注册 `diff_view` → `DiffCard`。
- 已有 `diff_view` StreamEvent 但没渲染器，直接复用这个事件类型。

---

## 3. 搜索结果可点击跳转

### 需求

`search_content` 返回 `file:line: content` 格式，需要每行都可点击跳转。

### 要做的

这个大部分被 #1 覆盖了。额外做：
- `search_content` 的结果在 `ToolCallCard` 里渲染时，每行自动转成可点击链接。
- 复用 #1 的 `open_file` IPC。

---

## 实现顺序

```
1. 后端 open_file IPC  → 前端文件链接点击
2. 前端 DiffCard  → 注册到 BlockRenderer
3. 后端 git_diff tool → Agent 可调用
4. 搜索结果链接化
```

## 验证方法

1. `cargo check` + `npx tsc --noEmit` 编译通过
2. 发送 prompt："帮我在 session.rs 里随便加一行注释"
3. Agent 编辑后能看到 diff 卡片
4. Agent 说 `src/agent/session.rs:123` 时，点击能打开文件
5. `search_content "AgentSession"` 的结果每行可点击跳转
