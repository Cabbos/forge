# Forge Memory and Knowledge Retrieval Design

Date: 2026-07-18
Status: revised after architecture review; pending user re-review
Scope: Forge desktop local memory foundation and read-only Feishu knowledge retrieval

## Goal

Forge should continue work accurately across turns and sessions while grounding answers in authoritative local and remote documents. The product should feel as though it remembers the right things without exposing retrieval machinery in the primary work surface.

The implementation order is deliberate:

1. Establish one retrieval contract over the existing authoritative stores without moving write ownership.
2. Add a derived local knowledge index with Chinese retrieval, deduplication, budgeting, and auditability.
3. Add a read-only Feishu knowledge connector on top of that retrieval plane.
4. Use measured correctness and operating evidence to decide whether physical memory migration is worth a separate project.
5. Consider remote writeback only after read quality and permission isolation are proven.

The design is local-first, but not local-authoritative for remote documents. Feishu remains the source of truth for Feishu content; the local database is a bounded, revocable search index and cache.

## Product Decision

Forge will keep memory and knowledge as separate domains and unify them only at retrieval time.

- **Memory** is a durable claim about a user, project, decision, task state, or learned experience. Memory has acceptance, pin, archive, forget, confidence, and scope semantics.
- **Knowledge** is authoritative source material such as a local project document, Forge Wiki page, or Feishu page. Knowledge has source identity, revision, access, freshness, citation, and cache semantics.
- **Context candidates** are the common retrieval envelope. Memory records and knowledge chunks become candidates, pass through the same hard filters and token budget, and are then formatted into distinct hidden-context sections.

This avoids treating copied document text as user memory and avoids giving inferred memory the authority of a maintained document.

## Current State

Forge already has the required execution boundaries:

- Session snapshots, visible transcript history, previous summaries, and auto-compaction provide working memory.
- `WikiMemoryStore` persists accepted, pinned, archived, forgotten, and candidate memory records.
- `MemoryFactStore` persists user- and profile-scoped facts.
- Continuity persists project events and accepted or pinned experiences in SQLite with FTS.
- Forge Wiki persists project records in `.forge/wiki/*.md` and uses user-approved update proposals.
- `UnifiedMemoryRecord` projects wiki memory, memory facts, and continuity experiences into one recall view.
- `turn_prepared` exposes body-free recall decisions and context-budget evidence.
- Selected MCP resources and prompts can be injected as untrusted connector context.

The remaining structural gaps are:

- The unified memory model is a read projection over multiple physical stores, not a unified physical store.
- The current physical migration gate explicitly remains a dry run.
- Memory relevance uses whitespace tokenization and fixed category signals, which is weak for Chinese text without spaces.
- Forge Wiki selection is deterministic but limited to a small fixed set of page names and intent keywords.
- MCP connector resources are user-selected turn inputs, not a persistent searchable knowledge index.
- Recall budgets are split by existing call sites rather than allocated by one cross-source planner.
- Remote access identity, permission revocation, freshness, and citation are not represented in the current unified memory record.

## Alternatives Considered

### 1. Live MCP pass-through only

Every relevant turn would call a Feishu MCP server to search and fetch pages.

Advantages:

- Smallest initial implementation.
- No persistent remote content cache.
- Feishu permissions are evaluated on every request.

Disadvantages:

- Network and process startup latency affect every turn.
- No reliable offline behavior.
- Ranking, deduplication, and cross-source recall remain fragmented.
- Search quality and availability depend on the connector for every request.

This is suitable for a prototype probe, not the durable product path.

### 2. Full local mirror

Forge would replicate complete permitted Feishu spaces and treat the mirror as the primary retrieval source.

Advantages:

- Fast local search and broad offline coverage.
- Straightforward cross-document ranking.

Disadvantages:

- Highest permission, revocation, deletion, and sync complexity.
- Easy to retain content after the user loses access.
- Large initial sync and storage footprint.
- Encourages support for every Feishu object type before the core loop is proven.

This is explicitly rejected for the first implementation.

### 3. Local retrieval plane with bounded read-through cache

Forge indexes explicitly selected sources locally, retrieves candidates locally, and validates top remote hits before injection.

Advantages:

- Fast cross-source retrieval and bounded degraded behavior when the remote source is unavailable.
- Feishu remains authoritative.
- Permission and freshness checks happen before remote content is used.
- Existing recall audit and context budgeting can cover local and remote candidates.

Disadvantages:

- Requires a sync model, cache lifecycle, and per-user authorization namespace.
- More work than live pass-through.

This is the selected approach.

## Architecture

### Retrieval data plane

Forge will add an application-managed SQLite database under the Forge application data directory. The logical name in the implementation plan should be `context.db`; the final path must use the existing application-data path resolver rather than reading `HOME` directly.

The database owns derived retrieval state and knowledge cache state. Existing memory stores remain the write authority in this design. `context.db` may contain a shadow memory projection for retrieval, but actions such as accept, edit, archive, restore, pin, unpin, and forget continue to execute against the source named by the Memory Authority Map.

Physical memory write cutover is not part of the connected-knowledge implementation. It requires a separate design, migration evidence, rollback gate, and user approval after unified retrieval has produced enough operating evidence to justify the risk.

Required logical tables:

| Table | Purpose |
| --- | --- |
| `memory_records` | Rebuildable derived retrieval projection of current source-owned memories, populated only when its shadow adapter is enabled and never used as write authority |
| `knowledge_sources` | Enabled local or remote sources, identity namespace, configuration, and sync state |
| `knowledge_documents` | Document identity, type, title, canonical URI, revision, content hash, freshness, and accessibility |
| `knowledge_document_locations` | Provider locations for a document, including Feishu space, node, parent path, origin or shortcut type, and citation URL |
| `knowledge_source_documents` | Many-to-many membership between selected sources and canonical documents, including generation and deletion state |
| `knowledge_chunks` | Bounded text chunks with heading path, ordinal, token estimate, and content hash |
| `knowledge_chunks_fts` | Chinese-capable full-text candidate generation |
| `knowledge_embeddings` | Optional embedding vectors and model identity; absence must not disable retrieval |
| `source_acl_bindings` | Account or authorization namespace required to retrieve a source or document |
| `sync_cursors` | Per-source pagination, generation, last-success, and last-error state |
| `recall_events` | Body-free candidate decisions, score components, selected ids, budget, latency, and source freshness |

Existing memory stores remain authoritative throughout this design. Rebuilding the memory projection from those stores must be supported, and failure of the projection must fall back to direct source reads rather than changing memory action semantics.

The continuity event journal and experience status remain project-local in `.forge/continuity.db`. Only a rebuildable recall projection may appear in `memory_records`; raw execution and reflection events do not become memory records.

### Database ownership and concurrency

`context.db` has one logical writer owned by a `ContextStore` in application state. Desktop IPC, the embedded Gateway, schedulers, background tasks, and sync workers call the store service instead of opening independent writable connections or running migrations themselves.

The first implementation uses SQLite WAL with bounded transactions and a configured busy timeout. Reads may use pooled read connections, but schema migration, generation commit, cache deletion, and projection rebuild are serialized through the writer. Headless evaluation uses an isolated database path or a read-only snapshot; it must not compete with the user's live database writer.

Startup verifies schema version and integrity before enabling retrieval. A failed migration leaves the prior schema and source stores readable, disables the affected derived source, and emits typed diagnostics. Cache tables are rebuildable; memory source stores and project documents are never recovered from `context.db`.

### Stable identity

Memory identity remains `<source>:<source_id>` to preserve current action and audit behavior.

Canonical knowledge-document identity is provider-qualified:

```text
<provider>:<account_namespace>:<object_type>:<remote_or_local_object_id>
```

For Feishu, the document object id is `obj_token`; a Wiki `node_token` is a location, not the document identity. Location identity is:

```text
feishu:<account_namespace>:<space_id>:<node_token>
```

This permits one document to appear through origin nodes, shortcuts, or multiple selected spaces without duplicating its body. Removing one source removes only its membership and location rows; the canonical document and chunks are deleted only when no enabled source still references them.

Chunk identity is derived from document identity, normalized heading path, normalized content hash, and an occurrence index among identical chunks. Ordinal is mutable ordering metadata and is not part of stable identity. Inserting content before an unchanged chunk therefore does not rename the unchanged chunk.

Project scope initially continues to use the canonical project path for compatibility with existing memory and continuity records. Project relocation support is outside this design and must not be silently approximated from a Git remote.

### Provider-neutral connector boundary

The retrieval domain will depend on a `KnowledgeConnector` boundary with these responsibilities:

- list user-selectable sources;
- enumerate document metadata incrementally;
- fetch a supported document body;
- validate current access and revision for selected documents;
- return typed authentication, permission, rate-limit, unsupported-type, network, and parse failures.

The production Feishu implementation should consume Feishu OpenAPI behind this boundary. MCP can be used for early compatibility probes, but core sync state, retries, permission handling, and source identity must not depend on a turn-scoped MCP resource selection.

### Retrieval flow

For every prepared turn:

1. Build a `RecallQuery` from user text, canonical project path, active profile, selected connector sources, and the total hidden-context budget.
2. Generate local candidates from current memory records, Forge Wiki and local-document FTS, and enabled remote knowledge indexes.
3. Apply hard filters before ranking:
   - accepted or pinned memory status;
   - project and profile scope;
   - enabled source;
   - matching authorization namespace;
   - non-deleted and non-inaccessible document state;
   - user-selected connector scope where required.
4. Deduplicate exact source identities and normalized content hashes across memory, continuity, local Wiki, and remote chunks while retaining every contributing provenance and citation location.
5. Rank candidates with explicit score components:
   - lexical relevance;
   - optional semantic relevance;
   - project and profile match;
   - pin and user acceptance;
   - source authority;
   - recency and remote freshness;
   - confidence for inferred memories.
6. Form a provisional remote selection capped at three unique Feishu documents, then validate current access and revision through the per-account rate limiter.
7. Remove inaccessible or failed documents. If a revision changed, use refreshed content only when the complete fetch, parse, chunk, and rerank finishes inside the remote-validation deadline; otherwise exclude the stale body for this turn and enqueue a priority refresh.
8. Run at most one bounded refill pass for slots vacated by validation. A document that was not successfully validated in this turn cannot enter the final remote selection.
9. Allocate the hidden-context budget across memory, project records, local knowledge, remote knowledge, files, and retained transcript.
10. Inject bounded content with distinct trust labels and source citations.
11. Persist a body-free recall event containing decisions, scores, revisions, token estimates, validation outcomes, and latency.

The first internal slice gives remote validation a two-second turn deadline, enforced independently from the overall model request timeout. Deadline exhaustion excludes unchecked remote bodies and records a typed `validation_deadline_exceeded` outcome. These constants may change only through measured eval evidence and acceptance updates.

Remote validation failure must remove the affected candidate before final token allocation. The planner must not spend budget on a candidate it cannot inject.

### Chinese retrieval

The local retrieval path must not depend on whitespace tokenization for Chinese.

The first implementation will provide:

- normalized Unicode text;
- punctuation-aware segmentation;
- an application-generated lexical column containing space-separated Han-character bigrams, plus single-character terms for titles, headings, tags, and one-character queries;
- the same normalization and term-generation function for indexing and query construction;
- title, heading, tag, and body field weighting;
- exact phrase boosts;
- deterministic fallback when optional embeddings are unavailable.

Embeddings are an optional second retrieval channel. The stored embedding model id, dimensions, and content hash must be explicit so an index can be invalidated safely after model changes.

No external vector database is required for the first implementation. SQLite remains the retrieval authority until measured corpus size or latency proves it insufficient.

### Context budgeting

The planner will replace independent source limits with one total hidden-context allocation. Source buckets retain maximums, but unused budget can be reassigned after hard filtering.

The first policy is:

- always reserve output tokens before retrieval;
- preserve recent visible conversation and the latest compacted summary;
- give selected files and explicit user-selected connector material priority over automatic recall;
- apply the explicit authority order below when sources conflict;
- inject no more than three chunks from one document unless the user explicitly asks to summarize that document;
- keep body-free audit metadata outside the injected-content count but inside telemetry size limits.

The existing `turn_prepared.context_estimate` bucket model remains the public evidence contract. Implementation may add sub-bucket diagnostics internally, but the primary UI must not expose retrieval internals as product navigation.

For the same claim and matching project/profile/account scope, authority is ordered as follows:

1. the current visible user instruction;
2. an explicitly selected file, connector resource, or document for the current turn;
3. accepted or pinned user/project decisions and preferences;
4. maintained project records and access-validated connected knowledge;
5. accepted continuity experiences;
6. inferred or candidate memory.

Recency, revision, and scope still apply within one tier. A remote document is not automatically more authoritative than a current user instruction, and a stale or unvalidated remote body is never used to resolve a conflict. When deduplication merges equivalent content, the selected candidate retains all eligible provenance rather than discarding secondary citations.

## Feishu Read-Only Connector

### Source selection and identity

The user explicitly connects a Feishu account and chooses one or more knowledge spaces. Forge does not enumerate or mirror the entire tenant by default.

The connector stores:

- account namespace without access-token material;
- selected `space_id` values;
- node token, origin node token where present, node type, parent path, object token, and object type;
- canonical Feishu Wiki URL per location;
- document title and revision;
- last metadata sync, last successful content fetch, and last access validation;
- typed sync or permission status.

Tokens and refresh credentials remain reference-only in the system credential store, following the existing provider credential model. They must never be serialized into `context.db`, transcripts, recall events, diagnostics payloads, or connector content.

### Authentication and authorization

The default identity is a user access token so retrieval reflects the connected user's document access. Bot or tenant identity is allowed only for spaces explicitly shared with the application.

API scopes and resource permissions are separate gates. Passing the OAuth scope check does not prove access to a knowledge space or document.

All cached rows are namespaced by the connected account identity. The credential reference and account namespace are also bound to the Forge profile or desktop identity that created the connection. A candidate from one account namespace must never be visible to another profile, session, gateway owner, or desktop user merely because the remote object id is the same.

The embedded Gateway may request retrieval only through the local owner session and its active profile binding. It may not enumerate account namespaces, refresh credentials, start a new account connection, or reuse another profile's cached remote content. Headless evaluation uses fixture identities and fixture credentials only.

### Supported object types

The first production slice supports:

- Wiki spaces and node hierarchy metadata;
- `docx` page title, revision, block content, headings, paragraphs, lists, and code blocks;
- references back to the canonical Feishu page.

Sheets, Bitable, Mindnote, files, images, comments, and attachments are metadata-only in the first slice. Their presence is recorded as an unsupported content type, not silently flattened or omitted from diagnostics. Separate structured adapters can be designed after Docx retrieval quality passes acceptance.

### Incremental sync

The first slice uses scheduled incremental polling rather than requiring event webhooks.

- A sync run records a new generation and enumerates the selected space tree with pagination.
- Unchanged document revisions skip body fetching.
- A changed revision triggers a body fetch and content hash comparison.
- A changed revision with an unchanged content hash updates metadata without rebuilding chunks.
- Locations and source-document memberships absent from one source's complete successful generation are marked deleted only after that generation commits. The canonical document is deleted only when no enabled source still references it.
- A partial or failed generation never deletes the last known-good index.
- Rate limits use bounded exponential backoff with jitter and expose the next retry time.

Top-result read-through validation remains independent of the background sync schedule. The bounded provisional-selection procedure in the retrieval flow catches permission revocation and urgent edits before automatic recall uses cached text without turning one query into an unbounded sequence of remote calls.

### Cache lifecycle and local disclosure

Remote content is cached only for user-selected spaces. The Settings surface must disclose that selected content is indexed locally and provide:

- pause sync;
- reconnect account;
- remove a space;
- clear one source cache;
- clear all Feishu cached content;
- last success, last error, and stale status.

For the internal macOS slice, the database and containing directory must use owner-only filesystem permissions. The product must not claim encrypted-at-rest remote content until an encryption design and verification gate exist.

Cache use is bounded by explicit configuration: total remote-cache bytes, per-account bytes, maximum extracted text bytes per document, maximum chunks per document, and maximum retained disabled-source age. The internal implementation plan must choose conservative defaults and include corpus measurements that justify them before beta. Exceeding a document limit indexes metadata only with a typed `content_too_large` status. Exceeding an account or total quota evicts least-recently-retrieved rebuildable bodies and chunks while preserving source configuration and diagnostics.

Removing a source immediately disables its memberships and locations for retrieval, then deletes source-specific ACL bindings and sync cursors. A canonical document body, chunks, and embeddings are deleted only when no enabled source still references the document. Deletion failure remains visible in diagnostics and keeps the source disabled.

### Knowledge-content trust boundary

Local files, Forge Wiki pages, MCP resources, and Feishu bodies are data, not executable instructions. Every automatically retrieved knowledge section is delimited and labelled untrusted in model-facing context. Instructions found inside a document are never treated as instructions; a visible user request may ask the model to quote or analyze them only as data.

Document bodies, block text, credentials, and extracted secrets must not appear in recall events, diagnostics, error strings, telemetry, or migration reports. Before remote content is persisted, a policy scanner classifies likely credentials and other configured secret patterns. The first slice stores only redacted text for high-confidence secret matches, records body-free redaction evidence, and exposes the affected document status in Settings. The original remote document remains authoritative and unchanged.

Titles, URLs, headings, and citation labels are escaped before prompt formatting or UI rendering. Retrieval never interprets document text as a tool call, permission grant, account selector, source configuration, or writeback authorization.

### Citations

Every injected Feishu chunk carries a hidden citation identity containing document title, canonical URL, object id, revision, heading path, and last validation time.

When an answer materially relies on remote knowledge, the model-facing context instructs the model to cite the page by title and link. Forge must not imply that an unvalidated cached result is current.

The first Feishu slice fails closed for remote bodies while offline: local metadata may still explain which source is unavailable, but cached Feishu content is not injected. A future opt-in stale-content mode requires its own permission and acceptance design.

## Writeback Policy

Remote writeback is outside the read-only connector milestone.

When designed later, writeback will reuse the existing Forge Wiki proposal pattern:

1. generate a target document and bounded patch preview;
2. validate current revision and edit permission;
3. show the exact destination and change to the user;
4. require explicit confirmation;
5. apply an idempotent mutation against the validated revision;
6. record result evidence and the resulting remote revision.

Automatic memory-to-Feishu publishing, background document editing, silent conflict resolution, and autonomous creation of new spaces are not authorized by this design.

## Failure Handling

| Failure | Required behavior |
| --- | --- |
| Context database unavailable | Read memory directly from its source stores, disable indexed knowledge retrieval, emit diagnostics, and do not fabricate recall |
| Memory projection mismatch | Keep source stores authoritative, disable or rebuild the projection, and preserve source-owned actions |
| Feishu authentication expired | Attempt one typed refresh; then mark reconnect required without leaking token details |
| Feishu node or space permission denied | Disable the affected location or source membership and preserve a canonical document still reachable through another enabled source |
| Feishu document permission denied | Mark the canonical document inaccessible in that account namespace, exclude it immediately, and schedule body/chunk deletion |
| Feishu rate limited | Keep the last good generation, back off, expose next retry time |
| Network unavailable | Exclude Feishu body candidates, retain local and memory retrieval, and state that remote knowledge could not be verified |
| Unsupported object type | Index metadata only and report the type; do not inject empty content |
| Partial sync | Preserve the previous committed generation and resume from the recorded cursor |
| Chunk or parser failure | Exclude the failed document body, retain metadata, and continue other documents |
| Embedding backend unavailable | Continue with deterministic lexical retrieval |
| Source removed during a turn | Recheck enabled state before final injection and omit the source |
| Remote validation deadline exceeded | Exclude unchecked remote bodies, retain local retrieval, and record the typed deadline outcome |
| Remote cache quota exceeded | Evict rebuildable least-recently-retrieved bodies, preserve source configuration, and expose quota status |

Connector failures are non-fatal to the agent turn unless the user explicitly requested a remote-only operation whose result cannot be obtained. In that case Forge must state that the source could not be verified.

## Rollout Plan

### Phase 1A: Unified retrieval contract

- Add provider-neutral context-candidate, knowledge-source, document, chunk, and recall-query contracts.
- Route existing memory, continuity, Forge Wiki, and connector selections through one budget planner without changing physical authority.
- Preserve current stream and audit compatibility.
- Add cross-source deduplication and Chinese lexical retrieval tests.

### Phase 1B: Local knowledge data plane

- Create `context.db` behind a disabled-by-default capability flag and one `ContextStore` writer.
- Index Forge Wiki and explicitly selected local Markdown/text documents as knowledge, not memory.
- Add the canonical document, location, source-membership, FTS, chunking, citation, quota, and source-deletion model.
- Add a rebuildable shadow projection adapter for existing memory only if it improves measured retrieval latency or ranking composition.
- Keep every memory action and source write authoritative in the existing store.

### Phase 1C: Local retrieval acceptance

- Prove Chinese lexical recall, stable chunk identity, cross-source authority, body-free audit, and deterministic fallback without embeddings.
- Prove database corruption and rebuild behavior without changing memory source data.
- Establish the read-only `KnowledgeConnector` contract with a local fixture connector.

### Phase 2A: Feishu read-only internal slice

- Add user authentication and selected-space configuration.
- Sync Wiki node metadata and Docx content.
- Add per-account ACL namespace, incremental generations, read-through validation, and cache deletion.
- Preserve origin/shortcut locations and many-to-many source membership.
- Surface source health and cache controls in Settings, not the Work Panel.

### Phase 2B: Retrieval quality hardening

- Add curated Chinese and English Feishu recall cases.
- Tune lexical and optional semantic ranking from measured failures.
- Prove permission revocation, unvalidated-citation rejection, offline fail-closed behavior, and rate-limit recovery.
- Expand object-type support only when a real internal use case requires it.

### Phase 3A: Physical memory migration decision

Use retrieval latency, correctness, maintenance cost, corruption recovery, and source-action complexity from Phases 1 and 2 to decide whether physical memory unification has enough value to proceed. If it does, create and obtain approval for a separate design covering snapshots, dual reads, dual writes, action authority, cutover, rollback, and forgotten-record anti-relearning. No physical cutover is authorized by this design.

### Phase 3B: Explicit remote writeback proposals

This phase requires a separate user-approved design before implementation.

## Workstream Decomposition

This document is the umbrella architecture decision and is not a single implementation plan. Work proceeds through independently reviewable specifications in this order:

1. **Unified context retrieval**: candidate contracts, source adapters, authority, hard filters, deduplication, ranking, budget allocation, trust formatting, and recall evidence.
2. **Local knowledge index**: `ContextStore`, schema, FTS term generation, local document parsing, source membership, quotas, rebuild, and corruption recovery.
3. **Feishu read-only connector**: OAuth/profile binding, selected spaces, Wiki location graph, Docx parsing, generations, validation rate limiting, citations, cache controls, and permission revocation.
4. **Physical memory migration decision**: a later evidence-based design, created only if the first three workstreams demonstrate a concrete benefit that source projections cannot provide.

Each workstream receives its own design, implementation plan, verification evidence, and reversible capability flag. Completion of one workstream must not silently authorize the next workstream's credentials, remote access, data migration, or write permissions.

## Testing and Evidence

### Unit and property tests

- Stable record and chunk identity.
- Stable chunk identity when preceding chunks are inserted or removed.
- Project, profile, status, source, and ACL hard filters.
- Chinese bigram candidate generation and phrase boosts.
- Exact-id and normalized-content deduplication.
- Deterministic ranking without embeddings.
- Cross-source token allocation and per-document caps.
- No hidden body in recall audit or migration report.
- Untrusted-content formatting and instruction non-execution.
- Secret redaction evidence without original secret text.

### Projection and database tests

- Direct-source fallback when `context.db` is unavailable.
- Shadow read equivalence for current and archived memory records when projection is enabled.
- Candidate suppression and forgotten-record anti-relearning equivalence.
- Source-owned action equivalence for archive, restore, forget, pin, unpin, and fact edit.
- Crash recovery during projection rebuild and knowledge generation commit.
- Single-writer serialization, busy-timeout handling, and read concurrency.
- Schema migration rollback and derived-cache rebuild.
- Capability-flag rollback to direct memory reads and disabled knowledge retrieval.

### Connector integration tests

- Authentication refresh success and failure.
- Space and node pagination.
- Wiki node token to object token/type resolution.
- Origin and shortcut nodes resolving to one canonical document with multiple locations.
- Removing one source does not delete a document still referenced by another enabled source.
- Docx unchanged revision, changed revision, and unchanged content hash.
- Revision change excludes stale content unless refresh, parse, chunk, and rerank complete inside the deadline.
- HTTP 403 permission revocation and cache exclusion.
- HTTP 429 bounded backoff.
- Three-document validation cap, one refill pass, and validation-deadline behavior.
- Partial generation does not delete prior content.
- Removed source cannot be recalled while cache deletion is pending.
- Unsupported object types remain metadata-only.
- Document and account quota behavior, eviction, and rebuild.

### Product acceptance

- A user can connect one account, select one space, sync it, inspect status, pause it, and remove it.
- A question grounded in one Feishu page produces a correct citation.
- A same-title page in an unauthorized account namespace is never selected.
- A changed or revoked top result is revalidated before injection.
- A shortcut and origin node cite the selected Wiki location without duplicating document bodies.
- Offline turns do not inject cached Feishu bodies and clearly state that remote knowledge could not be verified when it was requested.
- A document containing prompt-like instructions remains quoted data and cannot trigger tools, permissions, source changes, or writeback.
- Work Panel remains free of memory and retrieval management concepts.
- Settings and History preserve existing diagnostics and session behavior.

### Evaluation metrics

The eval runner should extend existing memory evidence rather than create an unrelated scoring path. Required metrics are:

- relevant source selected at top-k;
- wrong-project, wrong-profile, wrong-account, and archived/forgotten injection count;
- duplicate-content injection count;
- unauthorized remote injection count;
- unvalidated or stale remote citation count;
- selected tokens versus budget;
- remote validation and total retrieval latency;
- lexical-only versus hybrid retrieval result quality.

Any unauthorized remote injection, hidden-body audit leak, or wrong-account result is a release-blocking failure.

## Documentation and Acceptance Impact

When implementation changes a user-visible runtime surface, update:

- `README.md`;
- `apps/desktop/README.md`;
- `CHANGELOG.md`;
- `apps/desktop/e2e/acceptance.spec.ts`;
- `scripts/acceptance.sh --dry-run` labels and advertised coverage.

The user-facing product language should describe “connected knowledge” or “connected sources,” not vector databases, embeddings, memory authority maps, continuity stores, or retrieval internals.

## Non-Goals

- Mirroring an entire Feishu tenant.
- Supporting every Feishu object type in the first slice.
- Making embeddings mandatory.
- Introducing an external vector database.
- Treating remote document text as memory facts.
- Automatically publishing memory to Feishu.
- Autonomous Feishu writes or space creation.
- Moving memory or connector management into the Work Panel.
- Claiming encrypted-at-rest remote content before that behavior is implemented and verified.
- Solving project relocation or identity merging in this design.

## Completion Standard

The design is complete when:

- one retrieval planner explains and budgets all automatic memory and knowledge candidates;
- existing source-owned memory participates in one retrieval planner without changing action authority;
- local Chinese retrieval no longer depends on whitespace-only matching;
- local project records and remote documents remain distinct from inferred memory;
- a selected Feishu Docx space can be indexed read-only with per-account isolation;
- top remote results are access- and revision-validated before injection;
- answers can cite the authoritative Feishu page;
- source removal and permission revocation prevent future recall and trigger bounded cache deletion;
- acceptance and eval evidence prove no wrong-account or unauthorized injection;
- the evidence required for a separate physical-memory-migration decision is recorded without authorizing that migration.

## References

- [Feishu Wiki API overview](https://open.feishu.cn/document/ukTMukTMukTM/uUDN04SN0QjL1QDN/wiki-v2)
- [Feishu Wiki API FAQ](https://open.feishu.cn/document/ukTMukTMukTM/uUDN04SN0QjL1QDN/wiki-v2/wiki-qa)
- [Feishu Wiki node origin and shortcut fields](https://open.feishu.cn/document/server-docs/docs/wiki-v2/space-node/create)
- [Feishu Docx document API](https://open.feishu.cn/document/server-docs/docs/docs/docx-v1/document/get?lang=zh-CN)
- [SQLite FTS5 tokenizer reference](https://www.sqlite.org/fts5.html)
- [SQLite write-ahead logging](https://www.sqlite.org/wal.html)
- `apps/desktop/src-tauri/src/memory/unified.rs`
- `apps/desktop/src-tauri/src/ipc/unified_memory.rs`
- `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
- `apps/desktop/src-tauri/src/ipc/mcp_context.rs`
- `apps/desktop/src-tauri/src/forge_wiki/storage.rs`
- `apps/desktop/src-tauri/src/continuity/store.rs`
- `scripts/memory-migration-dry-run.mjs`
