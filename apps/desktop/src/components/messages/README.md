# components/messages/

> **Product surface:** Artifacts / Evidence (per-type block renderers)
>
> **Naming debt:** This directory is called `messages/` for historical reasons, but
> it contains **artifact/evidence renderers** for every backend `StreamEvent` type.
> Each file renders one kind of evidence: text, thinking, shell output, diffs,
> confirmations, delivery summaries, tool calls, errors, etc.

## Files

### Core renderers (imported by `BlockRenderer`)

| File | Renders `event_type` |
|------|---------------------|
| `TextBlock.tsx` | `text` (assistant text) |
| `ThinkingBlock.tsx` | `thinking` |
| `UserMessage.tsx` | `user_message` |
| `ShellCard.tsx` + `ShellCardHeader.tsx` + `ShellCardDetail.tsx` + `ShellOutputSections.tsx` | `shell` |
| `DiffCard.tsx` + `DiffBody.tsx` + `DiffHeaderActions.tsx` | `diff_view` |
| `ConfirmCard.tsx` + `ConfirmActions.tsx` + `ConfirmBoundaryViews.tsx` + `ConfirmViews.tsx` | `confirm_ask` |
| `DeliverySummaryCard.tsx` + `DeliverySummaryViews.tsx` | `delivery_summary` |
| `ToolCallCard.tsx` + `ToolActivityGroup.tsx` + `ToolActivitySummary.tsx` + `ToolActivityDetails.tsx` | `tool_call`, `tool_call_result` |
| `ProviderUsageCard.tsx` | `provider_usage` |
| `CodeBlock.tsx` | Inline / fenced code inside markdown |
| `DiagramBlock.tsx` | ASCII / Mermaid diagrams |
| `FilePreviewSheet.tsx` + `FilePreviewBody.tsx` + `FilePreviewActions.tsx` | File preview overlay |
| `ContextCompactCard.tsx` | `context_compact_start`, `context_compacted`, `context_compact_skipped` |
| `ErrorCard.tsx` | `error` (generic) |
| `MissingApiKeyCard.tsx` | `error` with `missing_api_key` code |
| `PendingBlock.tsx` | `pending` (loading indicator) |
| `SubAgentTrace.tsx` | Sub-agent execution trace |

### Presentation / logic helpers

| File | Responsibility |
|------|--------------|
| `codeBlockPresentation.ts` | Code block language detection, copy action state |
| `confirmPresentation.ts` | Confirm card decision labels, button states |
| `deliverySummaryPresentation.ts` | Delivery summary compact vs expanded view model |
| `diagramPresentation.ts` | Diagram type detection, containment rules |
| `diffPresentation.ts` | Diff stats, file path formatting |
| `filePreviewPresentation.ts` | File preview open/close state |
| `filePreviewTypes.ts` | File preview type definitions |
| `markdownFileRefs.tsx` | File reference parsing inside markdown |
| `processActivity.ts` | Tool/shell activity grouping logic |
| `processShellPresentation.ts` | Shell card presentation helpers |
| `processToolPresentation.ts` | Tool call presentation helpers |

### Shared helpers

| File | Responsibility |
|------|--------------|
| `MarkdownRenderer.tsx` | Shared markdown rendering (used by `TextBlock`) |
| `MessageCopyAction.tsx` | Copy-to-clipboard action for message blocks |
| `MessagePanel.tsx` | Shared panel/chrome for message containers |
| `ReaderCaptionAction.tsx` | Reader-mode caption toggle |
| `ProcessStatusDots.tsx` | Animated status dots for running processes |

## Import boundaries

- **May import from:** `primitives/`, `lib/*`, `store/`
- **Must NOT import from:** `session/`, `settings/`, `chat/` (except `messageGrouping.ts` pure helpers)
- **Consumers:** `chat/BlockRenderer.tsx` (single entry point)
