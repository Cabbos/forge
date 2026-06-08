# components/workbench/

> **Product surface:** Workbench readiness
>
> Components that live at the workbench level and are consumed by multiple
> product surfaces (layout, chat, etc.). They are neither low-level primitives
> nor tied to a single product domain.

## Files

| File | Responsibility |
|------|--------------|
| `StartReadinessCard.tsx` | Workbench readiness card: queries key/runtime/checkpoint status and renders the readiness surface |
| `StartReadinessView.tsx` | Visual layout for `StartReadinessCard`: panel and setup-strip variants |

## Import boundaries

- **May import from:** `primitives/`, `lib/*`, `store/`, `hooks/`
- **Must NOT import from:** `layout/`, `session/`, `chat/`, `messages/`, `settings/`, `context/`
- **Consumers:** `layout/EmptyWorkbench.tsx`, `chat/ConversationLane.tsx`
