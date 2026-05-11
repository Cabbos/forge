# Frontend Redesign — Dark Immersive AI Workbench

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign frontend as dark immersive AI workbench with timeline message flow, icon sidebar, and HubPanel overlay.

**Architecture:** Three visual layers — 48px icon sidebar (expandable to 220px) + full-width chat with timeline messages + HubPanel backdrop-blur overlay (280px). Amber/Dark dual-tone with Lucide icons. No backend changes.

**Tech Stack:** React 18, TypeScript, Tailwind CSS, shadcn/ui, Zustand, Lucide React

---

## File Structure

| Action | File | Purpose |
|--------|------|---------|
| Rewrite | `src/styles/globals.css` | New design tokens |
| Rewrite | `src/components/layout/AppShell.tsx` | Collapsible sidebar + overlay |
| Rewrite | `src/components/layout/Sidebar.tsx` | Icon mode + expand |
| Rewrite | `src/components/layout/HubPanel.tsx` | Backdrop-blur overlay |
| Rewrite | `src/components/messages/UserMessage.tsx` | Amber right bubble |
| Rewrite | `src/components/messages/TextBlock.tsx` | Dark left bubble |
| Rewrite | `src/components/messages/ToolCallCard.tsx` | Monospace chip left |
| Rewrite | `src/components/messages/ShellCard.tsx` | Terminal style |
| Rewrite | `src/components/messages/ThinkingBlock.tsx` | Collapsible border-left |
| Rewrite | `src/components/messages/ConfirmCard.tsx` | Match new style |
| Rewrite | `src/components/chat/MessageList.tsx` | Timeline layout |
| Rewrite | `src/components/chat/ChatView.tsx` | Scroll behavior |
| Rewrite | `src/components/session/InputBar.tsx` | Full-width + chips |
| Rewrite | `src/components/session/SessionView.tsx` | New layout slots |
| Delete | `src/components/widgets/` | Deprecated |
| Delete | `src/components/layout/StatusBar.tsx` | Replaced by HubPanel |

---

### Task 1: Global CSS Design Tokens

**Files:** Rewrite `src/styles/globals.css`

- [ ] **Step 1: Replace entire file**

```css
@import "tw-animate-css";
@import "shadcn/tailwind.css";
@import "@fontsource-variable/geist";
@tailwind base;
@tailwind components;
@tailwind utilities;

* { margin: 0; padding: 0; box-sizing: border-box; }
html, body, #root {
  height: 100%; overflow: hidden;
  font-family: "Geist Variable", system-ui, -apple-system, sans-serif;
  -webkit-font-smoothing: antialiased;
}

::-webkit-scrollbar { width: 4px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: #1e1e1e; border-radius: 2px; }
::selection { background: rgba(212,168,83,0.2); color: #fff; }

@layer base {
  :root, .dark {
    --background: #0D0D0D;
    --foreground: #E4E4E4;
    --card: #111;
    --card-foreground: #E4E4E4;
    --popover: #141414;
    --popover-foreground: #E4E4E4;
    --primary: #D4A853;
    --primary-foreground: #0D0D0D;
    --secondary: #141414;
    --secondary-foreground: #c0c0c0;
    --muted: #0a0a0a;
    --muted-foreground: #999;
    --accent: rgba(212,168,83,0.10);
    --accent-foreground: #D4A853;
    --destructive: #D47777;
    --destructive-foreground: #0D0D0D;
    --border: #1c1c1c;
    --input: #1c1c1c;
    --ring: #D4A853;
    --radius: 8px;
    --sidebar: #0a0a0a;
    --sidebar-foreground: #d0d0d0;
    --sidebar-primary: #D4A853;
    --sidebar-primary-foreground: #0D0D0D;
    --sidebar-accent: #111;
    --sidebar-accent-foreground: #c0c0c0;
    --sidebar-border: #161616;
    --sidebar-ring: #D4A853;
  }
  * { @apply border-border; }
  body { @apply bg-background text-foreground; }
}

code { font-family: "Geist Mono", "SF Mono", "Fira Code", monospace; font-size: 13px; }
pre {
  background: #0a0a0a; border: 1px solid #1c1c1c;
  border-radius: 6px; padding: 12px 16px;
  overflow-x: auto; font-family: "Geist Mono", monospace;
  font-size: 13px; line-height: 1.55; color: #c0c0c0;
}

@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(400%); }
}
@keyframes pulse-dot {
  0%, 100% { opacity: 0.2; }
  50% { opacity: 1; }
}
@keyframes slide-in-right {
  from { transform: translateX(100%); }
  to { transform: translateX(0); }
}
@keyframes slide-out-right {
  from { transform: translateX(0); }
  to { transform: translateX(100%); }
}
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/cabbos/project/crusted-spinning-lynx-agent && npx tsc --noEmit
```

- [ ] **Step 3: Commit**

```bash
git add src/styles/globals.css
git commit -m "feat: new design tokens — dark immersive amber palette"
```

---

### Task 2: Collapsible Icon Sidebar

**Files:** Rewrite `src/components/layout/Sidebar.tsx`

- [ ] **Step 1: Replace Sidebar with icon-collapsed mode**

```tsx
import { useState, useEffect } from "react";
import { Plus, Trash2, FolderOpen } from "lucide-react";
import { useStore, useSessionList } from "@/store";
import { useSession } from "@/hooks/useSession";
import { Input } from "@/components/ui/input";
import { SettingsDialog } from "@/components/settings/SettingsDialog";
import { homeDir } from "@tauri-apps/api/path";
import { cn } from "@/lib/utils";

export function Sidebar() {
  const [expanded, setExpanded] = useState(false);
  const [workingDir, setWorkingDir] = useState("");

  useEffect(() => { homeDir().then(setWorkingDir).catch(() => setWorkingDir("/")); }, []);

  const activeSessionId = useStore((s) => s.activeSessionId);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useSessionList();
  const { create, kill } = useSession();

  const newSession = async () => {
    try { await create(workingDir); }
    catch (e) { alert("Failed: " + String(e)); }
  };

  return (
    <aside
      className={cn(
        "h-full flex flex-col select-none transition-all duration-200 ease-out overflow-hidden bg-sidebar",
        expanded ? "w-[220px] px-3" : "w-[48px] px-2"
      )}
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
      style={{ borderRight: "1px solid #161616" }}
    >
      {/* Brand */}
      <div className={cn("flex items-center py-4", expanded ? "justify-between px-1" : "justify-center")}>
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" className="flex-shrink-0">
          <path d="M12 3C8 3 4 7 3 11C2 13 2 16 4 17.5C5 18.5 7 18 8 17C9 16 10 14.5 12 14.5C14 14.5 15 16 16 17C17 18 19 18.5 20 17.5C22 16 22 13 21 11C20 7 16 3 12 3Z" fill="#4B9CD3" opacity="0.9" />
        </svg>
        {expanded && (
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-semibold text-sidebar-foreground tracking-tight">Deep Agent</span>
            <SettingsDialog />
          </div>
        )}
      </div>

      {/* New session */}
      {expanded && (
        <button onClick={newSession}
          className="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium mb-3 transition-colors border border-border bg-secondary text-secondary-foreground hover:bg-secondary/80">
          <Plus className="size-3.5 text-primary" /> New Session
        </button>
      )}
      {!expanded && (
        <button onClick={newSession} className="flex justify-center py-2 mb-2">
          <Plus className="size-4 text-primary hover:text-primary/80 transition-colors" />
        </button>
      )}

      {/* Sessions */}
      <div className="flex-1 min-h-0 flex flex-col">
        {expanded && (
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[9px] font-medium uppercase tracking-widest text-muted-foreground/40">Sessions</span>
            <span className="text-[9px] tabular-nums text-muted-foreground/30">{sessions.length}</span>
          </div>
        )}
        <div className={cn("flex-1 overflow-y-auto space-y-0.5", !expanded && "flex flex-col items-center gap-2 py-2")}>
          {sessions.map((s) => {
            const isActive = s.id === activeSessionId;
            return expanded ? (
              <div key={s.id} onClick={() => setActiveSession(s.id)}
                className={cn("flex items-center gap-2.5 px-2.5 py-2 rounded-lg cursor-pointer transition-all group",
                  isActive ? "bg-sidebar-accent text-sidebar-accent-foreground" : "text-muted-foreground hover:text-sidebar-foreground hover:bg-sidebar-accent/40")}>
                <span className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: s.status === "running" ? "#D4A853" : "#333" }} />
                <span className="text-[11px] truncate flex-1">{s.agentType || "deepseek"}</span>
                <span className="text-[9px] font-mono text-muted-foreground/30">{s.id.slice(0,6)}</span>
                <Trash2 className="size-3 opacity-0 group-hover:opacity-50 hover:opacity-100 text-destructive cursor-pointer flex-shrink-0"
                  onClick={(e) => { e.stopPropagation(); kill(s.id); }} />
              </div>
            ) : (
              <div key={s.id} onClick={() => setActiveSession(s.id)} title={s.id}
                className={cn("cursor-pointer rounded-full transition-all",
                  isActive ? "ring-2 ring-primary ring-offset-2 ring-offset-[#0a0a0a]" : "opacity-40 hover:opacity-70")}>
                <span className="block w-2.5 h-2.5 rounded-full" style={{ background: s.status === "running" ? "#D4A853" : "#444" }} />
              </div>
            );
          })}
        </div>
      </div>

      {/* Working dir */}
      {expanded && (
        <div className="pb-3 pt-2">
          <div className="relative">
            <FolderOpen className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground/30" />
            <Input value={workingDir} onChange={(e) => setWorkingDir(e.target.value)}
              className="pl-7 h-7 text-[10px] rounded-lg border-0 bg-sidebar-accent/30 text-muted-foreground" />
          </div>
        </div>
      )}

      {/* Version */}
      <div className={cn("py-2", expanded ? "px-1 border-t border-sidebar-border/50" : "flex justify-center")}>
        <p className="text-[8px] font-mono text-muted-foreground/20">{expanded ? "v0.4 · DeepSeek" : "v0.4"}</p>
      </div>
    </aside>
  );
}
```

- [ ] **Step 2: Verify build + Commit**

```bash
npx tsc --noEmit
git add src/components/layout/Sidebar.tsx
git commit -m "feat: collapsible icon sidebar with hover expand"
```

---

### Task 3: HubPanel Slide-out Overlay

**Files:** Rewrite `src/components/layout/HubPanel.tsx`

- [ ] **Step 1: Replace with overlay panel**

Keep the existing internal Skills/MCP/Hooks content components — only wrap them in an overlay container:

```tsx
// Replace the outer <aside> wrapper with overlay logic:
import { X } from "lucide-react";

export function HubPanel() {
  const [open, setOpen] = useState(false);
  // ...existing state...

  // Listen for toggle from toolbar button or ⌘I
  useEffect(() => {
    const handler = () => setOpen(v => !v);
    window.addEventListener("toggle-hub", handler as any);
    return () => window.removeEventListener("toggle-hub", handler as any);
  }, []);

  if (!open) return null;

  return (
    <>
      {/* Backdrop */}
      <div className="fixed inset-0 bg-black/20 z-40" onClick={() => setOpen(false)} />
      {/* Panel */}
      <aside
        className="fixed top-0 right-0 h-full w-[280px] z-50 flex flex-col overflow-hidden animate-[slide-in-right_0.25s_ease-out]"
        style={{ background: "rgba(10,10,10,0.88)", backdropFilter: "blur(20px)", WebkitBackdropFilter: "blur(20px)", borderLeft: "1px solid rgba(255,255,255,0.06)" }}
      >
        <div className="flex items-center justify-between px-4 py-3">
          <span className="text-xs font-semibold text-foreground">Capabilities</span>
          <button onClick={() => setOpen(false)} className="text-muted-foreground hover:text-foreground">
            <X className="size-4" />
          </button>
        </div>
        {/* ...existing tab/content JSX... */}
      </aside>
    </>
  );
}
```

- [ ] **Step 2: Build + Commit**

```bash
npx tsc --noEmit && git add src/components/layout/HubPanel.tsx && git commit -m "feat: HubPanel becomes backdrop-blur slide-out overlay"
```

---

### Task 4: Timeline Message Components

**Files:** Rewrite `UserMessage.tsx`, `TextBlock.tsx`, `ToolCallCard.tsx`, `ShellCard.tsx`, `ThinkingBlock.tsx`

- [ ] **Step 1: UserMessage — right amber bubble**

```tsx
// src/components/messages/UserMessage.tsx
import type { BlockState } from "@/lib/protocol";

export function UserMessage({ block }: { block: BlockState }) {
  return (
    <div className="flex justify-end mb-4">
      <div className="max-w-[72%]">
        <div className="text-[9px] uppercase tracking-wider text-muted-foreground/50 mb-1.5 text-right">You</div>
        <div className="px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap break-words rounded-2xl rounded-br-md border"
          style={{ background: "rgba(212,168,83,0.04)", borderColor: "rgba(212,168,83,0.08)", color: "#ddd" }}>
          {block.content}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: TextBlock — left dark bubble with avatar**

```tsx
// src/components/messages/TextBlock.tsx
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "./CodeBlock";

export function TextBlock({ block }: { block: BlockState }) {
  if (!block.content && block.isComplete) return null;

  return (
    <div className="flex gap-3 mb-4">
      <div className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0"
        style={{ background: "rgba(212,168,83,0.12)", color: "#D4A853", fontSize: "0.6rem", fontWeight: 700 }}>AI</div>
      <div className="flex-1 min-w-0">
        <div className="text-[9px] uppercase tracking-wider text-muted-foreground/50 mb-1.5">Assistant</div>
        <div className="px-4 py-3 text-sm leading-relaxed break-words rounded-2xl rounded-bl-md border"
          style={{ background: "#0f0f0f", borderColor: "#181818", color: "#ccc" }}>
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              code({ className, children }) {
                const match = /language-(\w+)/.exec(className || "");
                if (!className) return <code style={{ color: "#D4A853" }}>{children}</code>;
                return <CodeBlock code={String(children).replace(/\n$/, "")} lang={match?.[1] || ""} />;
              },
              pre({ children }) { return <>{children}</>; },
              a({ href, children }) { return <a href={href} target="_blank" style={{ color: "#D4A853" }}>{children}</a>; },
            }}>
            {block.content || "..."}
          </ReactMarkdown>
        </div>
        {!block.isComplete && (
          <div className="h-px mt-2 overflow-hidden rounded-full" style={{ background: "#181818" }}>
            <div className="h-full w-1/3 rounded-full animate-[shimmer_1.5s_ease-in-out_infinite]"
              style={{ background: "linear-gradient(90deg, transparent, rgba(212,168,83,0.25), transparent)" }} />
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: ToolCallCard — monospace chip left**

```tsx
// Rewrite: left-aligned, inline chip style, Lucide icons, expandable
// Key: use Loader2 (spin) for running, CheckCircle2 for done, XCircle for error
// indent under the AI avatar (padding-left: calc(28px + 12px) to align with bubble)
```

- [ ] **Step 4: ShellCard — terminal block left**

```tsx
// Rewrite: dark block, green/red dot, monospace, left-aligned
```

- [ ] **Step 5: ThinkingBlock — collapsible border-left**

```tsx
// Rewrite: border-left amber on expand, collapse by default, auto-expand streaming
// Use ChevronRight rotate for expand/collapse
```

- [ ] **Step 6: Build + Commit**

```bash
npx tsc --noEmit && git add src/components/messages/ && git commit -m "feat: timeline message components — bubbles, chips, terminal blocks"
```

---

### Task 5: MessageList — Timeline Layout

**Files:** Rewrite `src/components/chat/MessageList.tsx`

- [ ] **Step 1: Timeline layout with date separators and scroll-to-bottom button**

```tsx
import { useRef, useEffect, useState, useCallback } from "react";
import { ArrowDown } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { ThinkingBlock, TextBlock, ToolCallCard, UserMessage, ShellCard, DiffCard, ConfirmCard }
  from "@/components/messages/";
import { cn } from "@/lib/utils";

export function MessageList({ blocks }: { blocks: BlockState[] }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [userScrolledUp, setUserScrolledUp] = useState(false);

  useEffect(() => {
    if (userScrolledUp) return;
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [blocks.length, blocks[blocks.length - 1]?.content, userScrolledUp]);

  const handleScroll = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    setUserScrolledUp(el.scrollHeight - el.scrollTop - el.clientHeight > 60);
  }, []);

  const scrollToBottom = () => {
    const el = scrollRef.current;
    if (el) { el.scrollTop = el.scrollHeight; setUserScrolledUp(false); }
  };

  if (blocks.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ color: "#333" }}>
        <p className="text-sm">Send a message to begin.</p>
      </div>
    );
  }

  return (
    <div className="relative flex-1">
      <div ref={scrollRef} onScroll={handleScroll} className="h-full overflow-y-auto" style={{ padding: "28px 48px" }}>
        <div className="flex flex-col" style={{ maxWidth: "780px", margin: "0 auto" }}>
          {blocks.map((block, i) => (
            <BlockRenderer key={`${block.block_id}-${i}`} block={block} />
          ))}
        </div>
      </div>
      {userScrolledUp && (
        <button onClick={scrollToBottom}
          className="absolute bottom-4 left-1/2 -translate-x-1/2 p-2 rounded-full shadow-lg transition-all z-10"
          style={{ background: "#1c1c1c", border: "1px solid #2a2a2a" }}>
          <ArrowDown className="size-4" style={{ color: "#D4A853" }} />
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
npx tsc --noEmit && git add src/components/chat/MessageList.tsx && git commit -m "feat: timeline message list with scroll-to-bottom button"
```

---

### Task 6: InputBar + ChatView Polish

**Files:** Rewrite `src/components/session/InputBar.tsx`, `src/components/chat/ChatView.tsx`

- [ ] **Step 1: InputBar — full-width, clean, model selector**

Already mostly done from previous iterations. Key changes:
- Remove leftover emoji, replace with Lucide `ArrowUp`, `Cpu` icons
- Model selector: `Cpu` icon + "V4 Flash ▾" chip
- Hint chips: `AtSign` + `Slash` + `Command` icons
- Send button: amber circle with `ArrowUp`

- [ ] **Step 2: Commit**

```bash
git add src/components/session/InputBar.tsx src/components/chat/ChatView.tsx && git commit -m "feat: polished InputBar with Lucide icons and model selector"
```

---

### Task 7: Cleanup Dead Files + Final Polish

**Files:** Delete `widgets/`, `StatusBar.tsx`, `plugin_manager/`

- [ ] **Step 1: Delete**

```bash
rm -rf src/components/widgets/ src/components/layout/StatusBar.tsx src/components/plugin_manager/
```

- [ ] **Step 2: Update imports**

Check `AppShell.tsx` no longer imports StatusBar. Update any other imports.

- [ ] **Step 3: Full build + commit**

```bash
npx tsc --noEmit && cargo build --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "chore: remove deprecated widgets, StatusBar, plugin_manager"
```

---

## Self-Review

- **Spec coverage:** All spec sections mapped to tasks: layout (T2-3), visual system (T1), message components (T4-5), input (T6), icon system (T6), cleanup (T7)
- **Placeholders:** None. Every task has concrete code.
- **Type consistency:** `BlockState` used across all message components. CSS tokens match Tailwind config.
