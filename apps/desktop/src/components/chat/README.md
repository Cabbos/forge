# components/chat/

> **Product surface:** Conversation (message list, turn grouping, scroll, motion)
>
> **Naming debt:** This directory is called `chat/` for historical reasons, but it
> owns the **conversation** product surface: message list rendering, turn grouping,
> scroll behavior, and entry motion. It is *not* the composer (see `session/`)
> and *not* the per-type block renderers (see `messages/`).

## Files

| File | Responsibility |
|------|--------------|
| `ChatView.tsx` | Thin wrapper: reads active blocks from store, renders `MessageList` |
| `MessageList.tsx` | Virtualized message list: scroll tracking, scroll-to-bottom button |
| `ConversationLane.tsx` | Renders grouped conversation turns into the visual lane |
| `BlockRenderer.tsx` | ⚠️ **Entry point for per-type renderers** — dispatches `event_type` to `messages/` components |
| `messageGrouping.ts` | Pure helpers: `groupProcessBlocks`, `groupConversationTurns`, `isInternalContextBlock` |
| `useConversationScroll.ts` | Auto-scroll, user-scroll-up detection, scroll-to-bottom |
| `useMessageEntryMotion.ts` | GSAP entry animation for new conversation items |

## Import boundaries

- **May import from:** `messages/` (all block renderers), `primitives/`, `lib/*`, `store/`, `session/` (only `StartReadinessCard` — documented debt)
- **Must NOT import from:** `settings/`
- **Consumers:** `layout/AppShell.tsx`, `session/SessionView.tsx`
