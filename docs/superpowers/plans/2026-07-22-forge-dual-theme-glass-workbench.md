# Forge 双主题玻璃工作台实施计划

> **供智能执行者使用：** 实施本计划时必须使用 `subagent-driven-development`（推荐）或 `executing-plans`，严格按任务顺序执行，并使用文中的复选框跟踪进度。

**目标（Goal）：** 将 Forge 桌面端升级为结构一致的石墨深色与纯白玻璃双主题，重构结果优先的对话体验，并消除流式 Markdown 反复重建造成的可见闪烁。

**架构（Architecture）：** 保留现有 React、Base UI、shadcn 和 CSS 变量体系，先建立 v6 语义材质层，再从共享控件向 conversation、composer、应用骨架、工作面板和设置页迁移。流式稳定性通过“稳定 Markdown 片段 + 单一活动尾段”实现，不在首轮引入新的渲染库；只有现有链路无法通过稳定性门槛时才另开替换计划评估 Streamdown 或 json-render。

**技术栈（Tech Stack）：** React 18、TypeScript、Vite、Tauri 2、Zustand、Base UI、shadcn/ui、原生 CSS 变量、ReactMarkdown、GSAP、Playwright、Node test、GitNexus。

---

## 实施前规则

1. 开始实施前创建独立工作树，不能在当前有用户未提交修改的工作树里直接改代码。
2. 修改任何函数、类或方法前，必须先调用 GitNexus `impact`，方向为 `upstream`，并向用户报告直接调用者、受影响流程和风险等级。
3. GitNexus 返回 HIGH 或 CRITICAL 时必须暂停并先告知用户。
4. 每个任务先写失败测试，再写最小实现，再运行局部测试，最后单独提交。
5. 每次提交前运行 GitNexus `detect_changes({ scope: "staged" })`。
6. 不修改后端协议、权限语义、工作面板对象模型或设置数据模型。
7. 不安装 Streamdown、json-render 或第二套组件系统。本计划的替换判定只形成证据，不直接换库。

## 文件职责映射

### 主题与共享材质

- `apps/desktop/src/styles/tokens.css`：v6 双主题语义变量、材质层级、圆角、阴影、排版与尺寸。
- `apps/desktop/src/styles/globals.css`：主题作用域、应用主画布、减少透明度降级和样式导入顺序。
- `apps/desktop/src/styles/primitives.css`：共享表面、按钮、图标按钮、列表行的完整交互状态。
- `apps/desktop/src/components/layout/AppShell.tsx`：设计版本标记与主题根作用域。
- `apps/desktop/scripts/check-conversation-style.mjs`：v6 静态样式契约。
- `apps/desktop/e2e/guardrails.spec.ts`：组件边界与禁用旧设计标记的静态守卫。

### 对话与流式稳定性

- 新建 `apps/desktop/src/components/messages/streamingMarkdownSegments.ts`：把流式 Markdown 划分为不可变稳定片段和一个活动尾段。
- 新建 `apps/desktop/src/components/messages/streamingMarkdownSegments.test.ts`：围栏代码、列表、表格、普通段落和尾段稳定性单元测试。
- 新建 `apps/desktop/src/components/messages/StreamingMarkdownRenderer.tsx`：稳定片段的渲染边界。
- 修改 `apps/desktop/src/components/messages/MarkdownRenderer.tsx`：保持完整回答渲染和文件引用能力，不再承担流式修复职责。
- 修改 `apps/desktop/src/components/messages/TextBlock.tsx`：移除助手头像与名称，接入稳定流式渲染。
- 修改 `apps/desktop/src/components/chat/useMessageEntryMotion.ts`：只允许新消息外壳首次入场。
- 修改 `apps/desktop/src/components/chat/ConversationTurn.tsx`、`ConversationProcessDisclosure.tsx`：结果优先结构和完成摘要。
- 修改 `apps/desktop/src/styles/messages.css`、`conversation-turn.css`、`markdown.css`：对话排版、用户气泡、过程摘要和正文节奏。
- 新建 `apps/desktop/e2e/streaming-stability.spec.ts`：60 秒流式稳定性与节点身份、滚动、焦点回归测试。

### Composer、应用骨架与产品表面

- `apps/desktop/src/styles/composer.css`：双主题玻璃输入框和完整按钮状态。
- `apps/desktop/src/styles/sidebar.css`、`titlebar.css`、`layout.css`：蓝灰深色侧栏、冷白玻璃浅色侧栏和主工作区层级。
- `apps/desktop/src/styles/work-panel.css`：与主工作台连续的分栏材质。
- `apps/desktop/src/styles/settings.css`：左侧导航与右侧分组设置行。
- `apps/desktop/src/styles/menu.css`、`command.css`、`confirm.css`、`delivery.css`：统一浮层、权限和交付材质。
- `apps/desktop/src/components/layout/Sidebar.tsx`、`AppTitlebar.tsx`：仅补充必要的语义属性，不改变导航数据。
- `apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`、`WorkPanelShell.tsx`：仅补充布局与主题验证所需属性，不改变对象状态模型。
- `apps/desktop/src/components/settings/SettingsDialog.tsx`、`SettingsCenterShell.tsx`：分组表面与可定位设置行语义。
- 新建 `apps/desktop/e2e/dual-theme-glass.spec.ts`：双主题、共享控件、应用骨架、设置和工作面板视觉契约。

### 文档与验收

- `apps/desktop/docs/product/forge-design-language.md`：将 Console Craft 更新为双主题玻璃工作台语言。
- `README.md`、`apps/desktop/README.md`、`CHANGELOG.md`：用户可见变更说明。
- `apps/desktop/e2e/acceptance.spec.ts`：产品级双主题与 conversation smoke。
- `scripts/acceptance.sh`：公布新增验收范围。

---

### 任务 1：建立 v6 双主题语义变量和静态契约

**文件：**

- 修改：`apps/desktop/src/styles/tokens.css`
- 修改：`apps/desktop/src/styles/globals.css`
- 修改：`apps/desktop/src/components/layout/AppShell.tsx:19-117`
- 修改：`apps/desktop/scripts/check-conversation-style.mjs`
- 修改：`apps/desktop/e2e/guardrails.spec.ts`

- [ ] **步骤 1：执行修改前影响分析**

调用：

```text
impact({ target: "AppShell", file_path: "apps/desktop/src/components/layout/AppShell.tsx", direction: "upstream", repo: "/Users/cabbos/project/forge" })
```

预期：列出 App、会话恢复和桌面壳层相关调用者。若风险高于 MEDIUM，先停止并报告。

- [ ] **步骤 2：先更新静态测试，使其要求 v6 设计标记和玻璃变量**

在 `check-conversation-style.mjs` 中把旧断言替换为：

```js
assertIncludes(files.appShell, 'data-design-version="v6-glass-workbench"', "app shell design version");
assertIncludes(files.tokens, "--forge-radius-shell: 18px;", "shell radius token");
assertIncludes(files.tokens, "--forge-radius-surface: 16px;", "surface radius token");
assertIncludes(files.tokens, "--forge-radius-control: 10px;", "control radius token");
assertIncludes(files.tokens, "--forge-material-inner-highlight:", "glass inner highlight token");
assertIncludes(files.globals, "@media (prefers-reduced-transparency: reduce)", "reduced transparency fallback");
assertNotIncludes(files.appShell, "v5-console-craft", "retired design version");
```

在 `guardrails.spec.ts` 增加：

```ts
test("desktop shell declares the v6 dual-theme glass contract", () => {
  const shell = read("src/components/layout/AppShell.tsx");
  const tokens = read("src/styles/tokens.css");
  expect(shell).toContain('data-design-version="v6-glass-workbench"');
  expect(tokens).toContain("--forge-material-inner-highlight:");
  expect(tokens).toContain("--forge-radius-shell: 18px;");
  expect(shell).not.toContain("v5-console-craft");
});
```

- [ ] **步骤 3：运行测试，确认当前实现失败**

运行：

```bash
npm --prefix apps/desktop run check:conversation-style
npm --prefix apps/desktop run test:e2e -- e2e/guardrails.spec.ts --grep "v6 dual-theme glass"
```

预期：FAIL，原因分别为缺少 v6 标记和新语义变量。

- [ ] **步骤 4：写入最小双主题变量**

在 `tokens.css` 的根主题中建立以下变量，并让现有 `--forge-material-*` 别名引用它们：

```css
--forge-radius-shell: 18px;
--forge-radius-surface: 16px;
--forge-radius-control: 10px;
--forge-material-canvas: #171717;
--forge-material-navigation: #273847;
--forge-material-group: rgba(36, 36, 36, 0.82);
--forge-material-control: rgba(47, 47, 47, 0.88);
--forge-material-floating: rgba(42, 42, 42, 0.92);
--forge-material-modal: rgba(35, 35, 35, 0.96);
--forge-material-border: rgba(255, 255, 255, 0.10);
--forge-material-inner-highlight: inset 0 1px 0 rgba(255, 255, 255, 0.10);
--forge-material-shadow: 0 18px 48px rgba(0, 0, 0, 0.28);
--forge-control-transition: 160ms cubic-bezier(0.16, 1, 0.3, 1);
```

在 `globals.css` 的浅色主题作用域中覆盖：

```css
--forge-material-canvas: #f7f9fb;
--forge-material-navigation: rgba(242, 247, 251, 0.78);
--forge-material-group: rgba(255, 255, 255, 0.68);
--forge-material-control: rgba(255, 255, 255, 0.76);
--forge-material-floating: rgba(255, 255, 255, 0.86);
--forge-material-modal: rgba(255, 255, 255, 0.94);
--forge-material-border: rgba(79, 98, 116, 0.16);
--forge-material-inner-highlight: inset 0 1px 0 rgba(255, 255, 255, 0.92);
--forge-material-shadow: 0 20px 52px rgba(64, 84, 104, 0.14);
```

加入实体降级：

```css
@media (prefers-reduced-transparency: reduce) {
  .forge-app-shell,
  .forge-composer,
  .forge-settings-dialog,
  .forge-command-surface,
  .forge-menu-surface {
    backdrop-filter: none !important;
    -webkit-backdrop-filter: none !important;
    background: var(--forge-bg-raised) !important;
  }
}
```

将 `AppShell.tsx` 标记改为：

```tsx
data-design-version="v6-glass-workbench"
```

- [ ] **步骤 5：运行局部检查并确认通过**

```bash
npm --prefix apps/desktop run check:conversation-style
npm --prefix apps/desktop run test:e2e -- e2e/guardrails.spec.ts --grep "v6 dual-theme glass"
```

预期：PASS。

- [ ] **步骤 6：检查影响并提交**

```bash
git add apps/desktop/src/styles/tokens.css apps/desktop/src/styles/globals.css apps/desktop/src/components/layout/AppShell.tsx apps/desktop/scripts/check-conversation-style.mjs apps/desktop/e2e/guardrails.spec.ts
git diff --cached --check
```

调用 `detect_changes({ scope: "staged", repo: "/Users/cabbos/project/forge" })`，确认只影响主题与桌面壳层后提交：

```bash
git commit -m "feat(desktop): establish dual-theme glass tokens"
```

---

### 任务 2：完善共享控件的完整交互状态

**文件：**

- 修改：`apps/desktop/src/styles/primitives.css`
- 修改：`apps/desktop/e2e/guardrails.spec.ts`
- 新建：`apps/desktop/e2e/dual-theme-glass.spec.ts`

- [ ] **步骤 1：确认共享控件仍由 Base UI 原语承载**

只读检查 `ForgeActionButton` 和 `ForgeIconButton`，确认二者继续透传 Base UI 的 `disabled`、`aria-*` 和 `data-*` 属性。本任务不修改这两个组件符号，只统一它们已有类名的 CSS 状态。

- [ ] **步骤 2：先写控件状态测试**

在 `dual-theme-glass.spec.ts` 写入：

```ts
import { expect, test } from "@playwright/test";
import { setup } from "./fixtures/app";

test("shared controls expose tactile hover active focus and disabled states", async ({ page }) => {
  await setup(page);
  await page.goto("http://localhost:1420");
  const button = page.getByRole("button", { name: "新对话", exact: true });
  await expect(button).toBeVisible();
  await button.focus();
  await expect(button).toBeFocused();
  const transition = await button.evaluate((node) => getComputedStyle(node).transitionDuration);
  expect(transition).not.toBe("0s");
  await button.hover();
  await expect(button).toHaveCSS("cursor", "pointer");
});
```

并在 `guardrails.spec.ts` 断言共享样式包含 `:active`、`:focus-visible`、`:disabled` 和 `[data-loading="true"]`。

- [ ] **步骤 3：运行测试确认失败**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/guardrails.spec.ts --grep "shared control state contract"
npm --prefix apps/desktop run test:e2e -- e2e/dual-theme-glass.spec.ts --grep "shared controls"
```

预期：静态守卫因缺少完整状态失败。

- [ ] **步骤 4：实现共享状态**

在 `primitives.css` 为 `.forge-action`、`.forge-icon-button` 和 `.forge-control-surface` 使用同一状态规则：

```css
.forge-action,
.forge-icon-button,
.forge-control-surface {
  border-color: var(--forge-material-border);
  background: var(--forge-material-control);
  box-shadow: var(--forge-material-inner-highlight);
  transition: transform var(--forge-control-transition), background-color var(--forge-control-transition), border-color var(--forge-control-transition), color var(--forge-control-transition), box-shadow var(--forge-control-transition);
}

.forge-action:hover,
.forge-icon-button:hover,
.forge-control-surface:hover {
  border-color: var(--forge-material-border-hover);
  background: color-mix(in srgb, var(--forge-material-control) 88%, var(--forge-text-primary) 12%);
}

.forge-action:active,
.forge-icon-button:active,
.forge-control-surface:active {
  transform: translateY(1px) scale(0.98);
}

.forge-action:focus-visible,
.forge-icon-button:focus-visible,
.forge-control-surface:focus-visible {
  outline: none;
  box-shadow: var(--forge-material-inner-highlight), 0 0 0 2px var(--forge-focus-ring);
}

.forge-action:disabled,
.forge-icon-button:disabled,
.forge-control-surface:disabled,
.forge-action[data-loading="true"] {
  cursor: default;
  opacity: 0.46;
  transform: none;
}
```

现有 `action.tsx` 和 `icon-button.tsx` 已通过 `{...props}` 透传 `data-loading`、`disabled` 和 Base UI 原生属性，不增加本地状态。

- [ ] **步骤 5：验证并提交**

运行任务 2 的两个测试和 `npm run build:desktop`。通过后运行 staged `detect_changes` 并提交：

```bash
git commit -m "feat(desktop): add tactile shared control states"
```

---

### 任务 3：建立稳定的流式 Markdown 分片器

**文件：**

- 新建：`apps/desktop/src/components/messages/streamingMarkdownSegments.ts`
- 新建：`apps/desktop/src/components/messages/streamingMarkdownSegments.test.ts`

- [ ] **步骤 1：先写分片器失败测试**

```ts
import assert from "node:assert/strict";
import test from "node:test";
import { segmentStreamingMarkdown } from "./streamingMarkdownSegments.ts";

test("freezes completed paragraphs and leaves one active tail", () => {
  assert.deepEqual(segmentStreamingMarkdown("第一段。\n\n第二段正在生成", false), [
    { id: "segment-0", content: "第一段。", stable: true },
    { id: "segment-1", content: "第二段正在生成", stable: false },
  ]);
});

test("keeps an open code fence in the active tail", () => {
  const result = segmentStreamingMarkdown("说明。\n\n```ts\nconst value = 1", false);
  assert.equal(result[0].stable, true);
  assert.equal(result[1].stable, false);
  assert.match(result[1].content, /```ts/);
});

test("promotes a closed fence after its blank boundary", () => {
  const result = segmentStreamingMarkdown("```ts\nconst value = 1\n```\n\n下一段", false);
  assert.equal(result[0].stable, true);
  assert.equal(result[1].stable, false);
});

test("marks the final tail stable when the stream completes", () => {
  const result = segmentStreamingMarkdown("只有一段最终内容", true);
  assert.deepEqual(result, [{ id: "segment-0", content: "只有一段最终内容", stable: true }]);
});
```

- [ ] **步骤 2：运行单元测试确认失败**

```bash
node --test apps/desktop/src/components/messages/streamingMarkdownSegments.test.ts
```

预期：FAIL，模块不存在。

- [ ] **步骤 3：实现纯函数分片器**

```ts
export interface StreamingMarkdownSegment {
  id: string;
  content: string;
  stable: boolean;
}

export function segmentStreamingMarkdown(
  content: string,
  complete: boolean,
): StreamingMarkdownSegment[] {
  if (!content) return [];
  const lines = content.split("\n");
  const segments: StreamingMarkdownSegment[] = [];
  let start = 0;
  let openFence: "```" | "~~~" | null = null;

  for (let index = 0; index < lines.length; index += 1) {
    const marker = lines[index].match(/^\s*(```|~~~)/)?.[1] as "```" | "~~~" | undefined;
    if (marker) openFence = openFence === marker ? null : marker;
    const isBoundary = lines[index].trim() === "" && openFence === null;
    if (!isBoundary) continue;
    const value = lines.slice(start, index + 1).join("\n").trim();
    if (value) segments.push({ id: `segment-${segments.length}`, content: value, stable: true });
    start = index + 1;
  }

  const tail = lines.slice(start).join("\n").trim();
  if (tail) {
    segments.push({
      id: `segment-${segments.length}`,
      content: tail,
      stable: complete,
    });
  }
  return segments;
}
```

- [ ] **步骤 4：运行单元测试并提交**

```bash
node --test apps/desktop/src/components/messages/streamingMarkdownSegments.test.ts
```

预期：4 个测试全部 PASS。暂存两个新文件，运行 `detect_changes`，然后提交：

```bash
git commit -m "feat(desktop): segment streaming markdown safely"
```

---

### 任务 4：接入稳定渲染边界并消除流式闪烁

**文件：**

- 新建：`apps/desktop/src/components/messages/StreamingMarkdownRenderer.tsx`
- 修改：`apps/desktop/src/components/messages/MarkdownRenderer.tsx:12-172`
- 修改：`apps/desktop/src/components/messages/TextBlock.tsx:1-83`
- 修改：`apps/desktop/src/styles/messages.css`
- 修改：`apps/desktop/e2e/messages.spec.ts`
- 新建：`apps/desktop/e2e/streaming-stability.spec.ts`

- [ ] **步骤 1：执行符号影响分析**

分别对 `MarkdownRenderer` 和 `TextBlock` 调用 upstream `impact`。预期直接影响用户消息、助手消息和写文件预览。若任一项为 HIGH 或 CRITICAL，暂停并报告。`useMessageEntryMotion` 已使用 `data-forge-motion-seen` 防止同一消息重复入场，本任务用 E2E 验证该行为，不修改此符号。

- [ ] **步骤 2：先写节点稳定性测试**

在 `streaming-stability.spec.ts` 使用现有 `setup` 和 `simulateStream`，加入：

```ts
test("streaming markdown keeps one assistant root and stable completed segments", async ({ page }) => {
  const sessionId = "streaming-stable-root";
  await setup(page);
  await page.addInitScript((id) => {
    // @ts-expect-error Forge E2E fixture session override
    window.__mockSessionId = id;
  }, sessionId);
  await page.goto("http://localhost:1420");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.locator("textarea").fill("输出包含段落和代码的说明");
  await page.locator("textarea").press("Enter");
  await simulateStream(page, sessionId, [
    { event_type: "text_start", session_id: sessionId, block_id: "answer" },
    { event_type: "text_chunk", session_id: sessionId, block_id: "answer", content: "第一段。\n\n" },
  ], 1);
  const answer = page.getByTestId("assistant-message");
  await answer.evaluate((node) => ((node as HTMLElement).dataset.identityProbe = "kept"));
  await simulateStream(page, sessionId, [
    { event_type: "text_chunk", session_id: sessionId, block_id: "answer", content: "```ts\nconst value = 1;\n" },
    { event_type: "text_chunk", session_id: sessionId, block_id: "answer", content: "```\n\n结尾" },
    { event_type: "text_end", session_id: sessionId, block_id: "answer" },
  ], 20);
  await expect(answer).toHaveAttribute("data-identity-probe", "kept");
  await expect(answer.getByText("第一段。")).toHaveCount(1);
});
```

- [ ] **步骤 3：运行测试确认失败**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/streaming-stability.spec.ts --grep "keeps one assistant root"
```

预期：FAIL，稳定片段渲染器尚不存在或根节点身份丢失。

- [ ] **步骤 4：实现稳定渲染组件**

`StreamingMarkdownRenderer.tsx` 使用固定片段键，稳定片段交给 MarkdownRenderer，活动尾段只显示文本：

```tsx
import { memo, useMemo } from "react";
import type { FileRef } from "./filePreviewTypes";
import { MarkdownRenderer } from "./MarkdownRenderer";
import { segmentStreamingMarkdown } from "./streamingMarkdownSegments";

const StableSegment = memo(function StableSegment({
  content,
  onOpenFileRef,
}: {
  content: string;
  onOpenFileRef: (fileRef: FileRef) => void;
}) {
  return <MarkdownRenderer content={content} onOpenFileRef={onOpenFileRef} />;
});

export function StreamingMarkdownRenderer({
  content,
  complete,
  onOpenFileRef,
}: {
  content: string;
  complete: boolean;
  onOpenFileRef: (fileRef: FileRef) => void;
}) {
  const segments = useMemo(
    () => segmentStreamingMarkdown(content, complete),
    [content, complete],
  );
  return (
    <div data-testid="streaming-markdown" data-complete={complete ? "true" : "false"}>
      {segments.map((segment) => segment.stable ? (
        <StableSegment key={segment.id} content={segment.content} onOpenFileRef={onOpenFileRef} />
      ) : (
        <span key={segment.id} className="forge-streaming-markdown-tail">{segment.content}</span>
      ))}
    </div>
  );
}
```

在 `messages.css` 加入活动尾段的稳定文本规则：

```css
.forge-streaming-markdown-tail {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
```

`TextBlock.tsx` 保留同一个 `assistant-message` 根节点，并在其内部使用 `StreamingMarkdownRenderer`。删除旧的 96ms 本地节流状态、助手头像和助手名称。流事件本身已经由 store 更新，渲染稳定性由片段边界负责。

`MarkdownRenderer.tsx` 删除 `stabilizeStreamingMarkdown` 和 `streaming` 参数。`CodeBlock` 在完整 Markdown 片段中始终使用 `streaming={false}`。

`useMessageEntryMotion.ts` 保持不变。它继续只监听 `blockCount`，并依靠 `data-forge-motion-seen` 保证同一个消息外壳只入场一次。

- [ ] **步骤 5：运行单元、E2E 和构建检查**

```bash
node --test apps/desktop/src/components/messages/streamingMarkdownSegments.test.ts
npm --prefix apps/desktop run test:e2e -- e2e/streaming-stability.spec.ts
npm --prefix apps/desktop run test:e2e -- e2e/messages.spec.ts --grep "streaming output|streaming markdown"
npm run build:desktop
```

预期：全部 PASS，流式回答根节点不被替换。

- [ ] **步骤 6：检查影响并提交**

运行 staged `detect_changes`，确认影响集中在 BlockRenderer 到 MarkdownRenderer 的消息流程后提交：

```bash
git commit -m "fix(desktop): stabilize streaming markdown rendering"
```

---

### 任务 5：重构结果优先的 Conversation 视觉结构

**文件：**

- 修改：`apps/desktop/src/components/messages/TextBlock.tsx`
- 修改：`apps/desktop/src/components/messages/UserMessage.tsx`
- 修改：`apps/desktop/src/components/chat/ConversationTurn.tsx`
- 修改：`apps/desktop/src/components/chat/ConversationProcessDisclosure.tsx`
- 修改：`apps/desktop/src/styles/messages.css`
- 修改：`apps/desktop/src/styles/conversation-turn.css`
- 修改：`apps/desktop/src/styles/markdown.css`
- 修改：`apps/desktop/scripts/check-conversation-style.mjs`
- 修改：`apps/desktop/e2e/conversation-result-first.spec.ts`
- 修改：`apps/desktop/e2e/messages.spec.ts`

- [ ] **步骤 1：影响分析**

对 `TextBlock`、`UserMessage`、`ConversationTurn` 和 `ConversationProcessDisclosure` 分别执行 upstream `impact`，重点检查消息渲染、结果优先流程、权限中断和交付摘要。

- [ ] **步骤 2：先改测试契约**

静态守卫要求：

```ts
expect(textBlock).not.toContain("forge-assistant-avatar");
expect(textBlock).not.toContain("forge-assistant-name");
expect(textBlock).toContain('data-message-role="assistant"');
expect(messages).toContain(".assistant-paper");
expect(messages).toContain("max-width: min(920px, 100%);");
```

E2E 要求助手正文无卡片背景、用户气泡右对齐、过程完成行可展开：

```ts
await expect(page.getByTestId("assistant-message")).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
await expect(page.getByTestId("user-message")).toHaveCSS("border-radius", "16px");
await expect(page.getByTestId("conversation-process-trigger")).toContainText("已处理");
```

- [ ] **步骤 3：运行目标测试确认失败**

```bash
npm --prefix apps/desktop run check:conversation-style
npm --prefix apps/desktop run test:e2e -- e2e/conversation-result-first.spec.ts e2e/messages.spec.ts --grep "result-first|conversation area|assistant prose|user bubbles"
```

- [ ] **步骤 4：实现 conversation 排版**

核心 CSS 使用以下完整规则作为基线：

```css
.forge-conversation-lane {
  width: 100%;
  max-width: min(920px, 100%);
  margin-inline: auto;
}

.assistant-paper {
  position: relative;
  display: block;
  min-width: 0;
  border: 0;
  background: transparent;
  box-shadow: none;
  color: var(--forge-text-primary);
  font-size: 15px;
  line-height: 1.78;
}

.forge-user-message-row {
  display: flex;
  justify-content: flex-end;
}

.forge-user-message {
  max-width: min(680px, 78%);
  border: 1px solid var(--forge-material-border);
  border-radius: 16px;
  background: var(--forge-material-group);
  box-shadow: var(--forge-material-inner-highlight);
  padding: 0.625rem 0.875rem;
}

.forge-process-disclosure-trigger {
  width: 100%;
  min-height: 32px;
  border-bottom: 1px solid var(--forge-border-subtle);
  border-radius: 0;
  padding-inline: 0;
}
```

完成摘要继续使用已有 `terminal.durationMs` 和 `terminal.operationCount`，不修改业务投影。风险、权限和失败块仍通过 `PrimaryBlock role="artifact"` 立即显示。

- [ ] **步骤 5：验证并提交**

运行任务 5 的静态检查、两个 E2E 文件和 `npm run build:desktop`。运行 staged `detect_changes` 后提交：

```bash
git commit -m "feat(desktop): align conversation with result-first layout"
```

---

### 任务 6：重做 Composer 和权限交互材质

**文件：**

- 修改：`apps/desktop/src/styles/composer.css`
- 修改：`apps/desktop/src/styles/confirm.css`
- 修改：`apps/desktop/src/components/session/ComposerSurface.tsx`
- 修改：`apps/desktop/src/components/messages/ConfirmCard.tsx`
- 修改：`apps/desktop/e2e/composer.spec.ts`
- 修改：`apps/desktop/e2e/acceptance.spec.ts`

- [ ] **步骤 1：对 `ComposerSurface` 和 `ConfirmCard` 执行 upstream impact**

重点记录输入提交、暂停、权限模式和确认响应流程。不得修改提交或确认业务逻辑。

- [ ] **步骤 2：先写双主题和按压状态测试**

测试要求：输入框两种主题都为 16px 圆角；focus-within 出现内高光和焦点环；发送按钮 active 时具备 `matrix` 或 `translate` 变换；权限确认始终可见且不被过程折叠。

- [ ] **步骤 3：运行目标测试确认失败**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/composer.spec.ts e2e/acceptance.spec.ts --grep "composer glass|permission remains visible"
```

- [ ] **步骤 4：实现材质与状态，不改业务行为**

```css
.forge-composer {
  border: 1px solid var(--forge-material-border);
  border-radius: var(--forge-radius-surface);
  background: var(--forge-material-floating);
  box-shadow: var(--forge-material-inner-highlight), var(--forge-material-shadow);
  backdrop-filter: blur(24px) saturate(150%);
  -webkit-backdrop-filter: blur(24px) saturate(150%);
}

.forge-composer:focus-within {
  border-color: var(--forge-material-border-focus);
  box-shadow: var(--forge-material-inner-highlight), var(--forge-material-shadow), 0 0 0 2px var(--forge-focus-ring);
}

.forge-composer-send:active {
  transform: translateY(1px) scale(0.98);
}
```

确认卡使用 `--forge-material-modal`，危险操作继续使用 `--forge-danger`，不把危险色改成普通强调蓝。

- [ ] **步骤 5：验证并提交**

运行 composer、permission 相关验收、conversation style check 和 desktop build。运行 staged `detect_changes` 后提交：

```bash
git commit -m "feat(desktop): refine glass composer and permission states"
```

---

### 任务 7：统一侧栏、标题栏和工作面板

**文件：**

- 修改：`apps/desktop/src/styles/layout.css`
- 修改：`apps/desktop/src/styles/sidebar.css`
- 修改：`apps/desktop/src/styles/titlebar.css`
- 修改：`apps/desktop/src/styles/work-panel.css`
- 修改：`apps/desktop/src/components/layout/Sidebar.tsx`
- 修改：`apps/desktop/src/components/layout/AppTitlebar.tsx`
- 修改：`apps/desktop/src/components/workpanel/WorkPanelLayout.tsx`
- 修改：`apps/desktop/src/components/workpanel/WorkPanelShell.tsx`
- 修改：`apps/desktop/e2e/chrome.spec.ts`
- 修改：`apps/desktop/e2e/acceptance.spec.ts`
- 修改：`apps/desktop/e2e/dual-theme-glass.spec.ts`

- [ ] **步骤 1：执行影响分析**

对 `Sidebar`、`AppTitlebar`、`WorkPanelLayout` 和 `WorkPanelShell` 执行 upstream `impact`。重点确认工作面板恢复、窄窗口覆盖模式和标题栏拖拽区。

- [ ] **步骤 2：先写布局和主题失败测试**

断言深色侧栏使用导航材质，浅色侧栏使用半透明白色；工作面板与主工作区之间只有一条边缘分隔；窄窗口仍可关闭工作面板；切换主题不改变面板宽度和当前标签。

- [ ] **步骤 3：运行测试确认失败**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/chrome.spec.ts e2e/dual-theme-glass.spec.ts e2e/acceptance.spec.ts --grep "sidebar material|work panel.*theme|theme.*panel"
```

- [ ] **步骤 4：实现结构连续的三段式工作区**

```css
.forge-sidebar {
  background: var(--forge-material-navigation);
  border-right: 1px solid var(--forge-material-border);
}

.forge-main-workbench {
  background: var(--forge-material-canvas);
  box-shadow: none;
}

.forge-work-panel {
  border-left: 1px solid var(--forge-material-border);
  background: var(--forge-material-canvas);
  box-shadow: none;
}

.forge-work-panel[data-layout="overlay"] {
  background: var(--forge-material-modal);
  box-shadow: var(--forge-material-inner-highlight), var(--forge-material-shadow);
  backdrop-filter: blur(24px) saturate(150%);
}
```

只添加 `data-layout`、`data-theme-surface` 等展示属性，不修改 `workPanelState.ts`、`workPanelPersistence.ts` 或 tab id。

- [ ] **步骤 5：验证并提交**

运行任务 7 的三个 E2E 文件和 desktop build。运行 staged `detect_changes`，确认工作面板状态流程未改变后提交：

```bash
git commit -m "feat(desktop): unify shell sidebar and work panel materials"
```

---

### 任务 8：统一设置页、菜单、命令面板和弹窗

**文件：**

- 修改：`apps/desktop/src/styles/settings.css`
- 修改：`apps/desktop/src/styles/menu.css`
- 修改：`apps/desktop/src/styles/command.css`
- 修改：`apps/desktop/src/styles/delivery.css`
- 修改：`apps/desktop/src/components/settings/SettingsDialog.tsx`
- 修改：`apps/desktop/src/components/settings/SettingsCenterShell.tsx`
- 修改：`apps/desktop/src/components/CommandPaletteContent.tsx`
- 修改：`apps/desktop/e2e/acceptance.spec.ts`
- 修改：`apps/desktop/e2e/dual-theme-glass.spec.ts`

- [ ] **步骤 1：执行影响分析**

对 `SettingsDialog`、`SettingsCenterShell` 和 `CommandPaletteContent` 执行 upstream `impact`。不得改变设置分区 id、字段顺序、命令动作或分析事件名称。

- [ ] **步骤 2：先写设置和浮层测试**

测试要求：设置导航保持左侧固定；设置内容使用分组表面而非每行卡片；搜索可以定位具体设置行；命令面板和菜单使用同一浮层变量；浅色和深色均通过对比检查。

- [ ] **步骤 3：运行测试确认失败**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/dual-theme-glass.spec.ts e2e/acceptance.spec.ts --grep "settings grouped surface|shared floating material"
```

- [ ] **步骤 4：实现设置分组和统一浮层**

```css
.forge-settings-dialog,
.forge-command-surface,
.forge-menu-surface {
  border: 1px solid var(--forge-material-border);
  border-radius: var(--forge-radius-shell);
  background: var(--forge-material-modal);
  box-shadow: var(--forge-material-inner-highlight), var(--forge-material-shadow);
  backdrop-filter: blur(24px) saturate(150%);
  -webkit-backdrop-filter: blur(24px) saturate(150%);
}

.forge-settings-group {
  border: 1px solid var(--forge-material-border);
  border-radius: var(--forge-radius-surface);
  background: var(--forge-material-group);
  box-shadow: var(--forge-material-inner-highlight);
}

.forge-settings-row + .forge-settings-row {
  border-top: 1px solid var(--forge-border-subtle);
}
```

为可搜索设置行增加稳定的 `data-setting-key`，其值使用现有 section id 与字段 id，不创建新状态源。

- [ ] **步骤 5：验证并提交**

运行设置相关 acceptance、dual-theme-glass、command 相关 smoke 和 desktop build。运行 staged `detect_changes` 后提交：

```bash
git commit -m "feat(desktop): align settings and floating surfaces"
```

---

### 任务 9：完成双主题、可访问性和 60 秒稳定性验收

**文件：**

- 修改：`apps/desktop/e2e/dual-theme-glass.spec.ts`
- 修改：`apps/desktop/e2e/streaming-stability.spec.ts`
- 修改：`apps/desktop/e2e/messages.spec.ts`
- 修改：`apps/desktop/e2e/acceptance.spec.ts`

- [ ] **步骤 1：补齐 60 秒长流测试**

测试用 120 个 500ms 间隔的片段组成 60 秒流，交替加入段落、列表、代码围栏和表格。记录以下条件：

```ts
await expect.poll(async () => page.getByTestId("assistant-message").count()).toBe(1);
await expect(page.locator("textarea")).toBeFocused();
await expect(page.getByTestId("assistant-message")).toHaveAttribute("data-identity-probe", "kept");
```

用户主动向上滚动后记录 `scrollTop`，后续片段到达时允许内容高度增长，但 `scrollTop` 不得被强制改回底部。

- [ ] **步骤 2：补齐减少动态和减少透明度检查**

Playwright 使用 `reducedMotion: "reduce"` 验证所有消息入场动画为 `none` 或 `0s`。减少透明度在浏览器不支持媒体模拟时，通过测试专用根属性触发与媒体规则相同的实体降级类，并断言 `backdrop-filter: none`。

- [ ] **步骤 3：补齐双主题截图矩阵**

为以下状态各保存 light 与 dark 两张截图：空白工作台、长回答、流式过程、展开证据、输入框聚焦、权限确认、工作面板、设置、命令面板。截图输出到 Playwright test artifacts，不提交运行噪声。

- [ ] **步骤 4：运行视觉与稳定性矩阵**

```bash
npm --prefix apps/desktop run test:e2e -- e2e/dual-theme-glass.spec.ts e2e/streaming-stability.spec.ts e2e/messages.spec.ts e2e/conversation-result-first.spec.ts
```

预期：全部 PASS，且没有整段回答根节点替换、重复入场、焦点丢失或强制滚动。

- [ ] **步骤 5：做替换判定并提交测试**

如果现有稳定分片方案通过全部门槛，记录“无需替换渲染库”。如果仍失败，停止实施并另写 Streamdown 技术验证设计，不能在本任务中直接安装依赖。

运行 staged `detect_changes` 后提交：

```bash
git commit -m "test(desktop): cover dual-theme and streaming stability"
```

---

### 任务 10：同步产品文档和 acceptance 宣告

**文件：**

- 修改：`apps/desktop/docs/product/forge-design-language.md`
- 修改：`README.md`
- 修改：`apps/desktop/README.md`
- 修改：`CHANGELOG.md`
- 修改：`scripts/acceptance.sh`
- 修改：`apps/desktop/e2e/acceptance.spec.ts`

- [ ] **步骤 1：更新设计语言文档**

将现有 v5 Console Craft 定位替换为 v6 双主题玻璃工作台，明确：双主题结构一致、conversation 结果优先、玻璃只用于真实浮层、控件完整状态、稳定流式片段、减少透明度实体降级。

- [ ] **步骤 2：更新用户可见说明**

在 README 和 CHANGELOG 使用以下统一口径：

```markdown
- 桌面工作台现在提供结构一致的石墨深色与纯白玻璃双主题。
- 对话区采用结果优先排版，普通工具过程默认收束为可展开摘要。
- 流式 Markdown 使用稳定片段渲染，避免长回复中的整块闪烁与滚动跳动。
```

- [ ] **步骤 3：更新 acceptance dry-run 项目**

在 `scripts/acceptance.sh` 新增一个 `desktop-ui` 域 gate，命令固定为：

```bash
npm --prefix apps/desktop run test:e2e -- e2e/dual-theme-glass.spec.ts e2e/streaming-stability.spec.ts
```

对应标签使用 `desktop dual-theme and streaming stability`，确保 `--dry-run` 能列出它。

- [ ] **步骤 4：验证文档与 dry-run**

```bash
scripts/acceptance.sh --dry-run
rg -n "v6|双主题|稳定片段" apps/desktop/docs/product/forge-design-language.md README.md apps/desktop/README.md CHANGELOG.md
```

预期：dry-run 包含新 gate，四份文档使用一致口径。

- [ ] **步骤 5：检查影响并提交**

运行 staged `detect_changes`，确认仅影响文档和 acceptance 宣告后提交：

```bash
git commit -m "docs: document dual-theme glass workbench"
```

---

### 任务 11：执行最终验证和范围审计

**文件：**

- 不新增文件
- 只在发现失败时回到对应任务的原文件修复

- [ ] **步骤 1：运行静态与构建检查**

```bash
npm --prefix apps/desktop run check:conversation-style
npm run build:desktop
```

预期：PASS。

- [ ] **步骤 2：运行核心 E2E**

```bash
npm --prefix apps/desktop run test:e2e -- \
  e2e/dual-theme-glass.spec.ts \
  e2e/streaming-stability.spec.ts \
  e2e/conversation-result-first.spec.ts \
  e2e/messages.spec.ts \
  e2e/composer.spec.ts \
  e2e/chrome.spec.ts \
  e2e/acceptance.spec.ts
```

预期：全部 PASS。

- [ ] **步骤 3：运行项目级验收预检**

```bash
scripts/acceptance.sh --dry-run
```

预期：退出码 0，并公布双主题与流式稳定性 gate。

- [ ] **步骤 4：执行 GitNexus 最终范围检查**

调用：

```text
detect_changes({ scope: "compare", base_ref: "main", repo: "/Users/cabbos/project/forge" })
```

预期：变化集中在桌面端展示、流式 Markdown 渲染、相关 E2E 和文档；不得出现后端协议、权限决策、凭据、调度或网关流程变化。

- [ ] **步骤 5：做提交与工作区审计**

```bash
git status --short
git log --oneline --decorate -12
```

确认没有误提交用户原有改动，没有生成物或截图噪声残留。若最后验证修复产生新改动，单独运行测试、`detect_changes` 并提交：

```bash
git commit -m "fix(desktop): close dual-theme verification gaps"
```

## 计划自检结果

- 设计方案中的双主题、材质层级、conversation、composer、侧栏、工作面板、设置、控件状态、减少动态、减少透明度、Generate UI 闪烁和文档同步均有对应任务。
- Streamdown 与 json-render 只作为失败后的独立决策门，不会在当前计划中未经验证引入。
- 所有新增 TypeScript 类型与函数名称在后续任务中保持一致：`StreamingMarkdownSegment`、`segmentStreamingMarkdown`、`StreamingMarkdownRenderer`。
- 没有待定占位符，也没有要求执行者自行猜测测试命令或验收标准。
