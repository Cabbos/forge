# Frontend Redesign — Dark Immersive AI Workbench

## Context

当前前端 36 个组件，三栏固定布局（240px + 1fr + 320px），Visual 风格混杂 shadcn 默认 + huashu-design 定制，消息流是左对齐 avatar 格式。用户希望重新设计为"暗色沉浸的现代 AI 工作台"。

## Design Decisions

| 决策 | 选择 |
|------|------|
| 性格 | 暗色沉浸（Claude.ai / Arc Browser） |
| 布局 | 图标侧栏 48px（hover 展开 220px）+ Chat 全宽 + HubPanel 滑出覆盖 |
| 消息流 | 时间线对话流：用户右（琥珀色气泡），AI 左（暗色气泡），Tool/Sys 居中 |
| 配色 | `#0D0D0D` 底 + Amber `#D4A853` 主强调 + Steel Blue `#5B9BD5` 辅助 |
| 字体 | Geist Variable (UI) + Geist Mono (code) |
| 材质 | HubPanel backdrop-blur 磨砂，卡片 `#111` 微升高 |

## Layout Architecture

```
┌─────────────────────────────────────────────────┐
│ ║  Chat Area (full width, max-w-3xl centered)  ║ │
│ ║  ┌─ MessageList ───────────────────────────┐  ║ │
│ ║  │  · User bubbles (right, amber tint)     │  ║ │
│ ║  │  · AI bubbles (left, dark)              │  ║ │
│ ║  │  · Tool cards (center, system style)    │  ║ │
│ ║  └─────────────────────────────────────────┘  ║ │
│ ║  ┌─ InputBar ─────────────────────────────┐   ║ │
│ ║  │  [@ files] [/ commands]     🐋 V4 ▾ [↑]│   ║ │
│ ║  └─────────────────────────────────────────┘   ║ │
│ ╚═════════════════════════════════════════════════╝ │
│                                                    │
│  ▎ icons  ▎ ← 48px collapsed sidebar               │
│  ▎ █ █ █  ▎                                         │
└─────────────────────────────────────────────────────┘
```

### States

| State | Sidebar | HubPanel |
|-------|---------|----------|
| Default | 48px icons only | Hidden |
| Sidebar hover | 220px (session list, new btn) | Hidden |
| HubPanel open | 48px | 280px slide-out overlay (backdrop-blur) |
| Both open | 220px | 280px slide-out overlay |

## Visual System

### Color Tokens

```
--bg-base:        #0D0D0D    // 主背景
--bg-sidebar:     #0A0A0A    // 侧栏更深
--bg-card:        #111111    // 卡片浮层
--bg-input:       #0F0F0F    // 输入框
--border:         #1c1c1c    // 分割线
--border-hover:   #2a2a2a    // hover 边框
--text-primary:   #E4E4E4    // 主文字
--text-secondary: #999       // 次要文字
--text-muted:     #555       // 弱化文字
--accent:         #D4A853    // Amber 主强调
--accent-blue:    #5B9BD5    // 文件/链接
--accent-green:   #4A9E6B    // 成功状态
--accent-red:     #D47777    // 错误/危险
```

### Typography

- UI text: Geist Variable, 13px body, -0.01em tracking
- Monospace: Geist Mono, 12px, for code/file paths/token counts
- Line height: 1.55 body, 1.6 code
- Border radius: 8px cards, 14px inputs, 6px buttons, 50% avatars

### Materials

- HubPanel overlay: `background: rgba(10,10,10,0.85); backdrop-filter: blur(20px);`
- Cards: `#111` with 1px `#1c1c1c` border, lifted on hover
- Input: `#0F0F0F` with `#1c1c1c` border, `#D4A853` focus ring

## Component Design

### Message Bubbles (Timeline)

**User message (right-aligned):**
- `max-width: 70%`, `background: rgba(212,168,83,0.1)`
- Border radius: `14px 14px 4px 14px` (tail bottom-right)
- Color: `#ddd`, font-size: 14px
- Timestamp below: `font-size: 9px, color: #444`

**AI message (left-aligned):**
- `max-width: 80%`, `background: #111`
- Border radius: `14px 14px 14px 4px` (tail bottom-left)
- Color: `#ccc`, font-size: 14px
- Markdown rendered inline

**Tool execution (center-aligned, system style):**
- `display: inline-block; background: #111; border: 1px solid #1a1a1a; border-radius: 8px`
- Icon + tool name + status (Running/Done/Error)
- Expandable to show input/output
- Animated: Running = amber spinner, Done = green check, Error = red X

**Thinking block:**
- Collapsed by default: "▶ Thinking ···" dim text
- Expanded: border-left 2px amber, `color: #888`
- Streaming: auto-expand, dots animate

**Shell output:**
- `background: #0a0a0a; border: 1px solid #1a1a1a; border-radius: 8px`
- Green dot indicator for success, red for failure
- Monospace output, max-height 300px overflow

**Date separators:**
- Center-aligned: `── 2026-05-10 ──`
- `font-size: 9px, color: #444`

### InputBar

- Full-width, max-w-3xl centered
- `background: #0F0F0F; border: 1px solid #1c1c1c; border-radius: 14px`
- Textarea: minimal height 24px, auto-expand to 140px
- Right side: model selector chip + send button (amber circle)
- Below: `@ files` `/ commands` `⌘K palette` hint chips
- Enter sends, Shift+Enter newline, IME composition safe

### Scroll behavior

- Auto-scroll to bottom on new content
- User manually scrolls up → stop auto-follow
- Show floating "↓ Back to latest" button when scrolled up
- Button click → smooth scroll to bottom, resume auto-follow

### Sidebar (collapsed)

- 48px fixed width
- Top: DeepSeek whale icon (amber, 28px, rounded 8px)
- Middle: session dots (green=running, gray=stopped)
- Bottom: + new session button

### Sidebar (expanded, 220px)

- Hover trigger with 0.2s transition
- Session list: name + timestamp + provider badge
- Active session: `background: #111`
- Delete on right-click or X hover

### HubPanel (overlay)

- 280px width, slides from right
- `backdrop-filter: blur(20px)`, `background: rgba(10,10,10,0.85)`
- Tab bar: Skills / MCP / Hooks
- Real data from CapabilityRegistry IPC
- Search + Install/Uninstall/Toggle

### Global search (⌘K)

- Floating modal, `backdrop-filter: blur`
- Fuzzy search: sessions, commands, files, skills
- Keyboard navigation: ↑↓ Enter Esc

## Component Tree (Target)

```
AppShell
├── Sidebar (collapsed/expanded)
│   ├── DeepSeekIcon
│   ├── SessionDots
│   └── NewSessionButton
├── MainContent
│   ├── ChatView
│   │   ├── MessageList
│   │   │   ├── DateSeparator
│   │   │   ├── UserBubble
│   │   │   ├── AIBubble
│   │   │   ├── ToolCard
│   │   │   ├── ThinkingBlock
│   │   │   ├── ShellCard
│   │   │   └── ScrollToBottomButton
│   │   └── InputBar
│   │       ├── Textarea
│   │       ├── ModelSelector
│   │       ├── HintChips
│   │       └── SendButton
│   ├── HubPanel (overlay)
│   │   ├── TabBar
│   │   ├── SkillsTab
│   │   ├── MCPTab
│   │   └── HooksTab
│   ├── CommandPalette (modal)
│   └── SettingsDialog (modal)
```

## Files to Create/Modify

| Action | File | Description |
|--------|------|-------------|
| Rewrite | `src/styles/globals.css` | New design tokens, animation keyframes |
| Rewrite | `src/components/layout/AppShell.tsx` | Collapsible sidebar + overlay HubPanel |
| Rewrite | `src/components/layout/Sidebar.tsx` | Icon mode + expand on hover |
| Rewrite | `src/components/layout/HubPanel.tsx` | Slide-out overlay with backdrop-blur |
| Rewrite | `src/components/chat/ChatView.tsx` | Timeline wrapper |
| Rewrite | `src/components/chat/MessageList.tsx` | Timeline layout + date separators |
| Rewrite | `src/components/messages/UserMessage.tsx` | Right-aligned amber bubble |
| Rewrite | `src/components/messages/TextBlock.tsx` | Left-aligned dark bubble |
| Rewrite | `src/components/messages/ToolCallCard.tsx` | Center system card with status |
| Rewrite | `src/components/messages/ThinkingBlock.tsx` | Collapsible with dots animation |
| Rewrite | `src/components/messages/ShellCard.tsx` | Terminal style output |
| Rewrite | `src/components/session/InputBar.tsx` | Full-width, hint chips, model selector |
| Delete | `src/components/widgets/` | Deprecated |
| Delete | `src/components/layout/StatusBar.tsx` | Replaced by HubPanel session info |
| Delete | `src/components/plugin_manager/` | Replaced by HubPanel skills tab |

## Non-Goals

- Mobile/responsive layout (desktop only)
- Multi-language i18n
- Accessibility audit (WCAG)
- Animation framework other than CSS transitions
- Changing the existing Tauri IPC protocol
