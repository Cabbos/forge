# Forge Unified Context Retrieval Design

Date: 2026-07-18
Status: approved for implementation planning
Parent: `docs/superpowers/specs/2026-07-18-forge-memory-knowledge-retrieval-design.md`
Workstream: 1 of 4 — unified context retrieval

## Goal

Forge should make one deterministic, inspectable decision about which memory and knowledge material enters a model turn. Source adapters may discover and parse material, but they may not independently decide final injection limits, consume unrelated source budgets, or serialize hidden bodies into evidence.

This workstream establishes the retrieval contract used later by the local knowledge index and the Feishu connector. It does not create `context.db`, sync a remote source, or move memory write authority.

## Scope

The first implementation covers the context sources already present in the desktop send path:

- unified memory records, including accepted continuity experiences already projected through unified memory;
- Forge Wiki project records;
- explicitly selected workspace files;
- explicitly selected MCP resources and prompts;
- source diagnostics needed to explain a failed explicit selection.

The planner also accounts for, but does not rank or remove, mandatory turn material:

- system prompt and safety rules;
- visible user input;
- previous summary and retained transcript;
- capability snapshot and recovery trace;
- reserved output tokens and the existing safety buffer.

Desktop IPC and accepted Gateway session input must use the same planner and evidence contracts.

## Non-Goals

- Creating or migrating a persistent retrieval database.
- Physically unifying memory stores or changing memory action authority.
- Crawling arbitrary local directories.
- Adding Feishu authentication, Wiki enumeration, Docx parsing, or remote validation.
- Adding embeddings or an external vector database.
- Replacing transcript compaction.
- Exposing retrieval internals as a new primary product surface.
- Changing Forge Wiki writeback proposals.

## Current-State Evidence

The current send path has useful source-specific safety controls but no cross-source planner:

| Surface | Current behavior | Gap addressed here |
| --- | --- | --- |
| `select_send_input_contexts` | Selects project records, memory, and MCP context sequentially | Sources cannot compete under one budget or deduplicate across boundaries |
| Unified memory | Chooses at most 8 records under a private 2,048-token budget | Its budget is unaware of files, project records, transcript, and connectors |
| Forge Wiki | Chooses at most 4 fixed pages and includes at most 8,000 content characters | It formats bodies before global selection and cannot yield budget to other sources deliberately |
| Selected files | Reads at most 6 files, 80,000 bytes each, and 120,000 characters total | Safety caps are mistaken for an injection budget and can dominate a turn |
| MCP context | Reads at most 8 selections and truncates each text item at 12,000 characters | Results are formatted before ranking; failures can become model context instead of typed evidence |
| `PreparedTurn` | Estimates already-built hidden contexts and reserves output | It is an after-the-fact report, not the decision maker |
| `ContextBuilder` | Injects every non-empty hidden block | `omitted_sources` is always empty and `budget_tokens` is not enforced |
| Eval runner | Scores memory evidence and several context buckets | It does not normalize the existing connector bucket or grade a generic retrieval plan |

Continuity has one obsolete direct selection function, but production selection already includes accepted and pinned experiences through unified memory. The direct path must not become a second adapter.

## Product Decision

Forge will use an adapter-first candidate pipeline.

Each adapter returns ephemeral `ContextCandidate` values and typed diagnostics. A provider-neutral `ContextRetrievalPlanner` applies hard filters, content deduplication, authority ordering, relevance ranking, and an initial retrieval ceiling. After transcript compaction, the context assembler performs a final capacity check before marking candidates injected.

Formatting happens only after planning. Public evidence contains identities, decisions, score components, budgets, and token estimates, but never candidate bodies.

## Alternatives Considered

### 1. Adapter-first candidates and one planner

Advantages:

- Enables candidate-level deduplication, provenance, authority, and future citations.
- Gives local knowledge and Feishu stable integration points without changing their storage.
- Separates “selected for assembly” from “actually injected.”
- Preserves source-specific parsers and access checks.

Disadvantages:

- Requires compatibility projections for existing memory and prepared-turn evidence.
- Requires the session assembler to understand ranked retrieval blocks.

This is the selected approach.

### 2. Budget whole `HiddenContextPart` blocks

This would keep current source formatting and drop or retain complete blocks.

It is rejected because a 120,000-character file block, a four-page Wiki block, and one memory record are not comparable units. Whole-block budgeting cannot preserve citations or deduplicate equivalent content.

### 3. Index every source in `context.db` first

This would provide one query surface but would make physical storage a prerequisite for a retrieval contract.

It is rejected for this workstream because it reintroduces migration risk, delays value, and conflicts with the parent design's source-authority decision.

## Architecture

### Logical components

The implementation introduces five focused boundaries:

1. **Candidate adapters** discover source material and return candidates plus diagnostics.
2. **Lexical analysis** produces deterministic Chinese and Latin relevance terms shared by adapters and the planner.
3. **Context retrieval planner** filters, deduplicates, ranks, and allocates the initial retrieval budget.
4. **Retrieval formatter** turns selected candidates into source-labelled, policy-labelled planned blocks.
5. **Final context assembler** recomputes capacity after transcript compaction, injects fitting blocks, and records omissions.

The planner is a pure deterministic component over a query, candidates, and a budget policy. Source I/O stays in adapters. Final message construction stays in the agent session layer.

### Query contract

`ContextRetrievalQuery` contains:

- session id and canonical project path;
- active profile id when present;
- visible user query and normalized lexical terms;
- explicitly selected file and connector identities;
- provider context-window size or the existing 128,000-token default;
- estimated mandatory-context tokens before retrieval;
- source-specific authorization namespace when a future connector provides one;
- the retrieval capability flag and shadow/default rollout mode.

The query is created from the session snapshot after the visible user message has been recorded. It contains no credentials and is not persisted with raw visible text. Body-free audit may store a query hash and normalized intent labels.

### Candidate contract

`ContextCandidate` is ephemeral and contains:

| Field | Meaning |
| --- | --- |
| `candidate_id` | Stable provider-qualified id for this candidate or segment |
| `source_kind` | Memory, project record, selected file, selected connector, or source diagnostic |
| `source_id` | Source-owned record, page, path, resource, or prompt id |
| `title` | Bounded human-readable label |
| `body` | Ephemeral text available only to planning and formatting |
| `scope` | Project, profile, account, session, or document constraints |
| `authority` | Accepted/pinned decision or preference, maintained project record, explicitly selected source material, accepted continuity, inferred memory, or diagnostic |
| `explicit_selection` | Whether the user selected this source for the current turn |
| `relevance` | Normalized lexical/source relevance from 0 to 1 |
| `confidence` | Optional source confidence from 0 to 1 |
| `freshness` | Optional source timestamp or revision evidence |
| `status` | Source-owned eligibility status used by hard filters |
| `content_hash` | Hash of normalized body for cross-source deduplication |
| `provenance` | One or more body-free source and citation identities |
| `estimated_tokens` | Estimate produced by the shared token estimator |
| `truncation_policy` | Atomic, bounded prefix, or bounded sections |
| `instruction_policy` | `data_only` by default; only an explicitly selected MCP prompt may record visible user delegation |

Candidate adapters may retain byte, file-count, response-size, and parse-depth safety caps. Those caps protect local and remote I/O; they are not final injection budgets.

### Decision and plan contracts

`ContextCandidateDecision` is the serializable, body-free pre-assembly audit form. It contains candidate and source ids, source kind, authority, score components, scope-match results, estimated and allocated tokens, provenance ids, truncation status, rank, and one decision:

- `selected_for_assembly`;
- `excluded_status`;
- `excluded_scope`;
- `excluded_authorization`;
- `no_relevance_signal`;
- `duplicate_merged`;
- `source_cap_exceeded`;
- `retrieval_budget_exceeded`;
- `source_unavailable`;
- `cancelled`.

`ContextAssemblyDecision` is the body-free final form recorded in the turn-context snapshot. It references a candidate id and records `injected`, `injected_truncated`, `final_context_budget_exceeded`, or `cancelled_before_model`.

`ContextRetrievalPlan` contains the policy version, query hash, total and per-bucket budgets, candidate decisions, selected candidate ids, merged provenance, source diagnostics, fallback mode, and planner latency. It does not contain body, block text, file content, prompt messages, credentials, or raw error payloads.

An ephemeral `PlannedContextBlock` carries the selected body into the session layer. It contains candidate ids, source kind, label, priority, estimated tokens, truncation policy, instruction policy, and formatted content. It is never serialized into `turn_prepared` or diagnostics.

## Candidate Adapters

### Unified memory adapter

The adapter reads the existing Wiki memory, profile facts, and continuity experience sources through their current authority boundaries. It emits one candidate per eligible or auditable record and preserves current status, project, and profile semantics.

The generic planner replaces the memory-only final limit and token allocation. The existing `RecallPlan` remains a compatibility projection of the generic decisions during rollout; it is not a second selection pass.

### Project record adapter

The adapter returns one candidate per considered Forge Wiki page. The first slice may preserve the current page-intent heuristic for candidate generation, but the fixed four-page injection limit moves to the planner. Page content remains bounded during I/O and is labelled data-only.

Metadata-only fallback after a page read failure is a distinct candidate with a source diagnostic; it does not masquerade as successfully loaded page content.

### Selected file adapter

The adapter preserves current canonical-workspace containment, normal-file, UTF-8, null-byte, byte-size, and file-count checks. It returns one bounded candidate per selected file. Large candidates use bounded-prefix truncation at formatting time and record omitted characters without placing omitted content in evidence.

An unresolved, outside-workspace, binary, or unreadable file produces a typed diagnostic and no body candidate.

### Selected MCP adapter

The adapter preserves the user's exact server/resource or server/prompt identity and current per-item response safety cap. Successful text becomes one candidate per explicit selection; multiple returned text blocks become bounded sections of that candidate. Resources remain data-only. A prompt remains untrusted by default and may be treated as delegated task guidance only when both the prompt was explicitly selected and the visible user request explicitly asks Forge to apply it. Delegation never grants tools, permissions, account access, or writeback authority.

Connection, permission, parse, and empty-content failures become typed diagnostics. Raw connector errors and prompt bodies never enter public evidence. A compact sanitized availability note may be selected for assembly only when the failed connector was explicitly selected for the turn.

### Continuity compatibility

Accepted and pinned continuity experiences enter only through the unified memory adapter. The unused direct continuity selector and `continuity_context` request field are removed after shadow evidence proves no production caller depends on them.

## Lexical Analysis

The first workstream replaces whitespace-only memory terms with one deterministic helper:

- Unicode NFKC normalization;
- lowercase normalization for scripts with case;
- punctuation-aware Latin and numeric tokens;
- contiguous Han-character bigrams;
- single Han characters for one-character queries, titles, headings, and tags;
- exact normalized substring and exact phrase signals;
- stable sorted unique output.

The same helper processes queries and candidate fields. Selected files and connectors remain eligible because they are explicit even when lexical relevance is zero. Automatic memory and project records require a relevance signal.

This helper is intentionally independent of SQLite. The local knowledge workstream will reuse its normalized terms when building FTS columns.

## Filtering, Deduplication, and Ranking

### Hard filters

The planner applies hard filters before scoring:

- memory status is accepted or pinned;
- project and profile scope match;
- source is enabled and available;
- candidate belongs to the explicit selection when explicit scope is required;
- authorization namespace matches when present;
- body is non-empty unless this is a sanitized source diagnostic;
- candidate has not been cancelled or invalidated during selection.

Hard-filter failures remain body-free audit decisions and cannot be restored by a high relevance score.

### Deduplication

The planner first deduplicates exact `candidate_id`, then normalized `content_hash`. A content duplicate becomes one candidate using the highest authority representation while retaining every eligible provenance and citation identity.

Deduplication never merges candidates across mismatched project, profile, or account scopes. It never discards a forgotten-memory suppression identity. Source diagnostics are deduplicated by source and failure class, not by message text.

### Ranking

Ranking is lexicographic rather than one opaque weighted number:

1. explicit selection;
2. authority class;
3. normalized lexical relevance;
4. pinned/accepted state;
5. scope specificity;
6. confidence and freshness when present;
7. stable `candidate_id` tie-breaker.

The visible user request is not a candidate and always outranks retrieved material. Authority affects factual conflict resolution, not instruction trust. Every automatic candidate and every selected file or resource remains data-only. Only an explicitly selected MCP prompt may use the bounded delegation policy defined above.

## Budget Policy

### Shared window policy

The existing context-window constants become one shared `ContextWindowPolicy` used by auto-compaction, prepared-turn evidence, retrieval planning, and final assembly:

- default context window: 128,000 tokens;
- minimum effective window: 16,000 tokens;
- reserved output: `min(20,000, window / 4)`;
- safety buffer: `min(13,000, window / 10)`;
- initial retrieval ceiling: `min(24,000, window / 4)`.

The planner's available retrieval budget is the initial ceiling further limited by the estimated space remaining after mandatory context, reserved output, and safety buffer. The visible user message, which is already present in session history when adapters run, is counted once. A zero remainder selects no body candidates.

### Initial allocation

The first pass uses soft shares of the available retrieval budget:

- 50% for explicitly selected files and connectors;
- 25% for maintained project records;
- 25% for memory, including continuity experiences.

Unused tokens return to one shared pool. The second pass allocates the pool in global rank order, so a single non-empty bucket can use the remaining budget. Soft shares prevent one abundant source from starving all others without wasting capacity.

Memory records are atomic. Project pages, selected files, and connector resources may use their declared bounded truncation policy. A truncated candidate retains source metadata, an explicit truncation marker in model context, and body-free allocated-versus-estimated token evidence.

### Final assembly enforcement

Candidate I/O and the initial plan occur before model-driven transcript compaction. Therefore `selected_for_assembly` is not the same as `injected`.

After compaction, the final assembler recomputes the full model-input estimate using the compacted transcript, summary, system prompt, mandatory hidden blocks, and planned retrieval blocks. If the total exceeds `window - reserved_output - safety_buffer`, it removes or truncates retrieval blocks from lowest to highest rank until the envelope fits.

The assembler never removes the system prompt, visible user message, safety/recovery context, or retained transcript. Transcript reduction remains the responsibility of auto-compaction and overflow retry. If mandatory material alone exceeds the envelope, every retrieval block is omitted and the existing overflow path remains authoritative.

Final `ContextBundle.sources` contains injected blocks. `ContextBundle.omitted_sources` contains planned blocks omitted by final enforcement with body-free reasons. Their source metadata is extended with body-free `candidate_ids` and the corresponding `ContextAssemblyDecision`, so generic plans and final evidence can be correlated without serializing content. The final turn-context snapshot, not `turn_prepared`, is authoritative for whether content reached the model.

## Trust and Formatting

Retrieval formatting follows these invariants:

- every candidate body is inside a source-labelled, delimited block;
- model-facing text says that memory claims, documents, files, resources, and connector content are untrusted data rather than permission or tool instructions;
- explicitly delegated MCP prompt guidance is labelled separately, retains the untrusted-source boundary, and cannot expand authority;
- file fences, titles, paths, labels, and URLs are escaped or JSON-quoted as appropriate;
- a candidate body cannot create a tool request, permission grant, source selection, account binding, or writeback approval;
- source failures use sanitized typed summaries rather than raw external error text;
- no candidate body is written to application logs, stream evidence, eval artifacts, or diagnostics;
- candidate bodies live only for the turn and are dropped after final assembly.

Memory remains lower-trust than visible conversation when claims conflict. Maintained project records may have higher factual authority than inferred memory but remain instruction-untrusted.

## Prepared-Turn and Eval Compatibility

`TurnPrepared` gains an optional `context_retrieval_plan`. During rollout Forge keeps:

- `selected_memory_ids`;
- `selected_memory_audit`;
- `memory_recall_plan`;
- `selected_project_record_ids`;
- existing context-estimate buckets.

These fields are projected from the generic plan and must not run independent selection logic. They remain until the desktop, Gateway ownership checks, eval runner, and stored fixtures consume the generic plan for one accepted release cycle.

`turn_prepared.context_estimate` and `ContextCandidateDecision` represent pre-assembly selection and reserved budgets. `turn_context.sources`, `turn_context.omitted_sources`, and `ContextAssemblyDecision` represent final assembly. The eval runner correlates them by candidate id and treats a preselected candidate as injected only when final context evidence confirms it.

The eval runner adds and recognizes these buckets without dropping existing ones:

- visible input;
- hidden system;
- memory;
- files;
- project records;
- connector context;
- compacted transcript;
- reserved output.

Future local and remote knowledge buckets may be added without changing candidate decision semantics.

## Failure Handling

| Failure | Required behavior |
| --- | --- |
| One automatic adapter fails | Continue other adapters, emit a typed diagnostic, and do not fabricate a candidate |
| Explicit file or connector fails | Continue the turn, expose source status, and allow one bounded sanitized availability note |
| Planner fails in shadow or compatibility rollout | Use existing bounded source behavior through final envelope enforcement, mark `fallback_legacy`, and exclude future-only sources |
| Planner returns a scope-invalid selection | Treat as a release-blocking invariant failure and exclude it |
| Token estimate changes after formatting | Re-estimate the formatted block before final assembly |
| Final context exceeds the envelope | Omit or truncate lowest-ranked retrieval blocks and record `final_context_budget_exceeded` |
| Mandatory context alone exceeds the envelope | Omit all retrieval blocks and use existing compaction/overflow handling |
| User cancels during adapter I/O | Stop remaining adapter work, mark unfinished candidates cancelled, and do not start the model turn |
| Audit serialization fails | Continue only with body-free minimal source ids and emit diagnostics; never fall back to serializing bodies |

Adapter failures are non-fatal unless the user requested a source-only operation whose result cannot be obtained. In that case the response must state that the selected source was unavailable.

## Rollout

### Stage A: Contracts and shadow plan

- Add candidate, query, decision, plan, diagnostic, budget-policy, and planned-block contracts.
- Adapt current sources without changing injected context.
- Produce a body-free shadow plan and compare selected identities, scope exclusions, and token estimates with legacy behavior.
- Add generic eval parsing while retaining legacy metrics.

### Stage B: Automatic-source cutover

- Let the generic planner control memory and Forge Wiki selection behind a reversible capability flag.
- Project legacy memory and project-record evidence from the generic plan.
- Prove project/profile filtering, continuity deduplication, Chinese recall, and no hidden-body audit leak.

### Stage C: Explicit-source and final-assembly cutover

- Route selected files and MCP resources/prompts through candidate adapters.
- Enable final context-envelope enforcement and populate omitted-source evidence.
- Prove Desktop and Gateway parity, cancellation, connector failures, and overflow behavior.

### Stage D: Default path and cleanup

- Make the planner default only after shadow and cutover acceptance pass.
- Remove the unused direct continuity selector and independent source injection limits that duplicate planner policy.
- Retain I/O safety caps and compatibility evidence for the documented release window.
- Keep a reversible legacy fallback until one release cycle passes acceptance.

## Testing and Evidence

### Unit and property tests

- Stable candidate identity and deterministic ordering independent of adapter completion order.
- Project, profile, account, status, enabled-source, and explicit-selection hard filters.
- Exact-id and normalized-content deduplication with retained multi-source provenance.
- Unicode normalization, Chinese bigrams, one-character queries, Latin tokens, and exact phrase signals.
- Soft-share allocation, unused-budget redistribution, atomic memory, and bounded truncation.
- Preselection versus final injection decisions.
- Final envelope enforcement and deterministic omitted-source order.
- No body, prompt text, file text, credential, or raw external error in serialized evidence.

### Adapter integration tests

- Unified memory preserves accepted/pinned, project/profile, candidate, archived, and forgotten semantics.
- Continuity experience appears once through unified memory and never through a second selector.
- Forge Wiki produces per-page candidates and metadata-only failure evidence.
- File resolution rejects outside-workspace, symlink escape, binary, missing, and oversized input safely.
- MCP resource and prompt success, empty result, process failure, cancellation, and truncation.
- Adapter completion order does not change the plan.

### Send-path tests

- Desktop and Gateway inputs produce the same plan for the same session state.
- `turn_prepared` stays body-free and retains legacy fields.
- `turn_context` identifies final injected and omitted sources.
- Capability snapshot and recovery trace remain mandatory and outside retrieval ranking.
- Auto-compaction followed by final assembly remains within the shared envelope when mandatory context fits.
- Overflow retry reuses the same candidate plan without duplicating retrieval blocks.

### Eval cases

- Chinese memory recall without whitespace.
- Duplicate memory and project-record content selects one body with two provenance identities.
- Large selected file does not starve relevant project memory.
- Empty automatic buckets allow an explicit source to reuse the remaining budget.
- Wrong-project, wrong-profile, archived, forgotten, or unauthorized candidate is never injected.
- Connector bucket is recognized and validated.
- Candidate selected before compaction but omitted at final assembly is not scored as injected.
- Hidden-body, raw-error, and duplicate-source evidence remain release-blocking failures.

## Acceptance Gates

The workstream is complete only when evidence proves:

- every in-scope retrieval source enters the model only through the generic plan and final assembler;
- no adapter retains an independent final injection budget;
- source I/O safety caps remain enforced;
- final retrieval bodies fit the shared context envelope whenever mandatory context fits;
- explicit sources receive priority without permanently starving automatic sources;
- Chinese queries recall relevant memory without whitespace;
- cross-source duplicates inject once and retain all eligible provenance;
- Desktop and Gateway paths produce equivalent plans;
- final injection and omission evidence is body-free and matches actual model context;
- existing stream, History, ownership, and eval consumers remain compatible;
- the capability flag can restore the legacy path during the documented rollback window.

Any wrong-project, wrong-profile, wrong-account, archived, forgotten, unauthorized, duplicate-body, hidden-body-audit, or final-overbudget injection is release-blocking.

## Documentation and Acceptance Impact

Implementation must keep these surfaces aligned when their behavior changes:

- `README.md`;
- `apps/desktop/README.md`;
- `CHANGELOG.md`;
- `apps/desktop/e2e/acceptance.spec.ts`;
- `scripts/acceptance.sh --dry-run`;
- relevant eval-runner cases and scoring fixtures.

User-facing language remains “context,” “project records,” “saved background,” and “connected sources.” Candidate ids, authority classes, score components, token shares, and planner modes remain diagnostic evidence rather than primary navigation.

## References

- `docs/superpowers/specs/2026-07-18-forge-memory-knowledge-retrieval-design.md`
- `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
- `apps/desktop/src-tauri/src/ipc/unified_memory.rs`
- `apps/desktop/src-tauri/src/ipc/project_records.rs`
- `apps/desktop/src-tauri/src/ipc/file_references.rs`
- `apps/desktop/src-tauri/src/ipc/mcp_context.rs`
- `apps/desktop/src-tauri/src/memory/unified.rs`
- `apps/desktop/src-tauri/src/forge_wiki/storage.rs`
- `apps/desktop/src-tauri/src/agent/prepared_turn.rs`
- `apps/desktop/src-tauri/src/agent/context_builder.rs`
- `apps/desktop/src-tauri/src/agent/auto_compact.rs`
- `apps/eval-runner/app/scoring.py`
