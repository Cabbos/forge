# Forge V1 Internal Beta And V2 Direction

Updated: 2026-05-21

## Product Position

Forge is a local-first desktop agent for building and maintaining local projects. The user should feel three things:

1. I can open a project and describe what I want.
2. Forge understands the task, uses the right local context, and stays inside the project boundary.
3. I can trust the visible work evidence enough to continue, verify, or stop.

V1 should not ask users to understand Workflow Router, Context Activation, Memory, Wiki, MCP, Hooks, or Skills as product concepts. These are internal infrastructure. The product language remains:

- Current Task
- Project Archive
- Delivery

## V1 Internal Beta Standard

V1 is ready for internal beta when a non-expert user and a professional developer can both complete a small local project task without breaking workspace boundaries or needing to understand Forge internals.

Detailed repeatable beta prompts and pass/fail rules live in `docs/product/forge-v1-internal-beta-playbook.md`.

### Must Work

- Create or resume a conversation in the selected workspace.
- Make `@file` search and file context strictly use the current session workspace.
- Route `/` actions as structured intent, not as raw prompt text.
- Auto-include hidden context from selected files, saved background, project records, connectors, and capability state.
- Keep capability internals hidden behind product-safe labels.
- Show tool calls, diffs, shell output, verification, checkpoints, and delivery state clearly enough for review.
- Provide clear missing API key and stopped-session recovery messages.
- Keep the interface quiet, dark, readable, and close to Codex-like interaction density.

### Must Not Do

- Do not expose internal terms as user-facing product concepts.
- Do not silently fall back to editing or searching Forge itself when the active workspace is different.
- Do not require users to manually manage context engineering.
- Do not push users into complex file/data panels before the core agent loop is reliable.
- Do not promise enterprise gateway, collaboration, cloud sync, or remote execution in V1.

## V1 Engineering Tracks

### Track 1: Workspace Safety

Goal: The user always knows Forge is acting in the intended project.

Current state:

- Workspace selection exists.
- Forge source workspace can be detected as high risk.
- File search and file references are scoped to the current session workspace.

Remaining V1 work:

- Make session restore and app restart preserve workspace/session association more visibly.
- Add regression coverage for open-file, preview-file, checkpoint, runtime, and search all using the same workspace source.
- Improve failure copy when a path is blocked or outside the workspace.

### Track 2: Invisible Capability Layer

Goal: `/`, `@`, Skills, MCP, Hooks, and saved background should behave like one coherent agent brain.

Current state:

- `/` commands are sent as structured capabilities.
- `/` action intent is translated into hidden backend intent.
- `@file` is sent as structured file reference and rendered as a visible user-selected reference.
- Hidden capability snapshots include action, file references, selected connectors, matched skills, active hooks, and available connectors.

Remaining V1 work:

- Make slash action intent participate in Skill matching consistently.
- Make capability snapshot wording product-safe and stable.
- Keep capability evidence available for debugging without making it central in the UI.
- Ensure MCP failures are summarized as useful evidence rather than noisy raw errors.

### Track 3: Continuity

Goal: Long work should keep direction without making users understand context limits.

Current state:

- Auto compact exists.
- Context source snapshots exist.
- Saved background and project archive records exist.

Remaining V1 work:

- Make resume behavior predictable after app restart.
- Make "what did we discuss before" answer honestly from retained visible history and saved project background.
- Avoid duplicated or malformed summaries in Chinese output.
- Add internal test prompts for long-running continuation and memory boundaries.

### Track 4: Delivery Confidence

Goal: The user can tell whether the work is ready to try.

Current state:

- Delivery summary, runtime status, checkpoint status, verification traces, and diff cards exist.

Remaining V1 work:

- Tighten the hierarchy between final answer, verification evidence, and delivery status.
- Ensure failed verification produces a natural next action.
- Keep preview/checkpoint actions available without occupying the entire product experience.

### Track 5: Product Polish

Goal: The app feels like a mature local agent, not a demo shell.

Current state:

- Conversation reading experience and input bar have been heavily polished toward Codex-like density.
- Sidebar and empty state are cleaner.

Remaining V1 work:

- Continue reducing visual noise in secondary panels.
- Make all empty/error/loading states intentional.
- Keep Chinese copy primary and consistent.

## V2 Direction

V2 should not be "more panels." It should deepen Forge's moat: a local agent that quietly understands the user's projects over time.

### V2 Theme 1: Project-Native Intelligence

Forge should become better at each project the longer it is used:

- Project archive becomes a hidden operating memory, not a visible wiki product.
- Forge learns build commands, architecture, conventions, risky files, and user preferences.
- Context selection becomes more accurate and less dependent on manual `@file` usage.

### V2 Theme 2: Guided Creation For Non-Experts

Forge should help a non-coder turn a vague need into a working local tool:

- Ask only the minimum necessary questions.
- Propose the next concrete step.
- Build previewable increments.
- Explain tradeoffs in plain language.
- Avoid exposing implementation machinery unless the user asks.

### V2 Theme 3: Professional Control For Developers

Forge should remain fast and comfortable for professional developers:

- Richer `/` and `@` command surfaces.
- Better diff and verification workflows.
- Explicit review mode, fix mode, test mode, and refactor mode.
- Configurable local policies for tools, hooks, skills, and workspace safety.

### V2 Theme 4: Local Connectors

V2 can expand local integrations without turning into an enterprise platform:

- Local files and documents.
- Local browser automation.
- Local app automation where permission is clear.
- Optional Feishu/Obsidian/GitHub-like connectors as user-owned local capabilities.

Enterprise gateway, hosted collaboration, organization admin, billing, and cloud memory should remain out of scope until the local product loop is trusted.

## Internal Beta Test Prompts

Use a non-Forge workspace such as `forge-test-app` when testing write behavior.

### Beginner Creation

```text
我想做一个本地小工具，用来记录每天喝水次数。你先做一个能用的第一版，页面要能直接操作，不要改 Forge 本体。
```

### Workspace Boundary

```text
只在当前项目里搜索文件。引用 @src，然后告诉我你看到的是哪个项目的文件，不要读取 Forge 本体。
```

### Structured Action

```text
/fix
@src/App.tsx
这个页面上的按钮点击后没有反馈。请先定位原因，再做最小修复，并运行相关检查。
```

### Review Mode

```text
/code-review
请检查当前项目最近改动里最可能导致回归的问题。先列问题，不要直接改代码。
```

### Continuity

```text
我们之前在这个项目里决定了哪些设计方向？如果你不知道，请明确说不知道，并只基于当前项目记录和可见对话回答。
```

### Connector Context

```text
如果本轮带入了连接资料，请只把它当作背景使用。总结你会如何利用它，不要复述原文。
```

## Next Implementation Order

1. Finish invisible capability routing for slash intent, Skill matching, and product-safe context labels.
2. Strengthen workspace-bound IPC coverage across search, preview, open, runtime, and checkpoint.
3. Improve resume and continuity behavior after app restart.
4. Tighten delivery evidence hierarchy and failed-verification recovery.
5. Run one full internal beta script on a non-Forge demo project.
