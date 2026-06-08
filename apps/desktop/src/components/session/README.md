# components/session/

> **Product surface:** Composer (input / send / resume / model menu)
>
> **Naming debt:** This directory is called `session/` for historical reasons, but
> nearly every file here belongs to the **composer** product surface.
> The only exception is `SessionView.tsx` (conversation shell).
> If you are looking for session *state* logic, see `src/store/`.

## Files

| File | Responsibility |
|------|--------------|
| `SessionView.tsx` | Conversation shell: mounts `ChatView` + `InputBar` for the active session |
| `InputBar.tsx` | Composer orchestrator: wires all composer sub-surfaces together |
| `ComposerSurface.tsx` | Textarea + send/stop/compact/resume controls |
| `ComposerTextarea.tsx` | Auto-resizing textarea with IME support |
| `ComposerToolbar.tsx` | `@`, `/`, stop/send/resume action buttons |
| `ComposerChipTray.tsx` | File/command chips with remove/overflow |
| `ComposerMenuLayer.tsx` | Floating menu container (suggestions + model menu) |
| `ComposerModelMenu.tsx` | Provider/model selector dropdown |
| `ComposerSuggestionMenu.tsx` | `/` and `@` suggestion list |
| `ComposerResumeError.tsx` | Resume-failure inline notice |
| `useComposerController.ts` | Top-level composer hook: composes all sub-hooks |
| `useComposerActions.ts` | Send, stop, compact, resume, keyboard handling |
| `useComposerChips.ts` | Chip add/remove/keyboard navigation |
| `useComposerDraft.ts` | Textarea value, auto-height, IME composition |
| `useComposerKeyboard.ts` | Keyboard shortcuts (Enter, Escape, Arrow keys) |
| `useComposerMenuDismissal.ts` | Click-outside / Escape dismissal for floating menus |
| `useComposerModelMenu.ts` | Model/provider selection state |
| `useComposerPresentation.ts` | Props assembly for the visual surface |
| `useComposerResume.ts` | Session resume state |
| `useComposerSessionState.ts` | Reads streaming/running state from the Zustand store |
| `useComposerSubmit.ts` | Submit validation and IPC call |
| `useComposerSuggestions.ts` | `/` and `@` trigger detection + file search |
| `composerCommands.ts` | Static slash-command definitions |
| `composerControllerView.ts` | View-model helpers for controller output |
| `composerTurnState.ts` | Turn-in-flight / running / paused state machine |
| `composerTypes.ts` | Shared composer type aliases |
| `contextUsageView.ts` | Context-window usage indicator view model |

## Import boundaries

- **May import from:** `chat/` (only `messageGrouping` helpers for block classification), `primitives/`, `lib/*`, `store/`
- **Must NOT import from:** `messages/`, `settings/`
- **Consumers:** `layout/AppShell.tsx`
