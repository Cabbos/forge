# Forge Memory and Knowledge Retrieval Design

Date: 2026-07-18
Status: pending user review
Scope: Forge desktop local memory foundation and read-only Feishu knowledge retrieval

## Goal

Forge should continue work accurately across turns and sessions while grounding answers in authoritative local and remote documents. The product should feel as though it remembers the right things without exposing retrieval machinery in the primary work surface.

The implementation order is deliberate:

1. Establish one local retrieval contract and a safe physical migration path for existing memory.
2. Improve local Chinese retrieval, deduplication, ranking, budgeting, and auditability.
3. Add a read-only Feishu knowledge connector on top of that retrieval plane.
4. Consider remote writeback only after read quality and permission isolation are proven.

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

The database owns derived retrieval state and the migrated memory view. It does not replace authoritative project Markdown or Feishu documents.

Required logical tables:

| Table | Purpose |
| --- | --- |
| `memory_records` | Unified physical form of wiki memory, memory facts, and recallable continuity experiences after migration approval, including candidate and forgotten suppression state |
| `knowledge_sources` | Enabled local or remote sources, identity namespace, configuration, and sync state |
| `knowledge_documents` | Document identity, type, title, canonical URI, revision, content hash, freshness, and accessibility |
| `knowledge_chunks` | Bounded text chunks with heading path, ordinal, token estimate, and content hash |
| `knowledge_chunks_fts` | Chinese-capable full-text candidate generation |
| `knowledge_embeddings` | Optional embedding vectors and model identity; absence must not disable retrieval |
| `source_acl_bindings` | Account or authorization namespace required to retrieve a source or document |
| `sync_cursors` | Per-source pagination, generation, last-success, and last-error state |
| `recall_events` | Body-free candidate decisions, score components, selected ids, budget, latency, and source freshness |

Existing stores remain authoritative during shadow and dual-read phases. The new database must not become the only readable copy until migration invariants pass.

The continuity event journal remains project-local in `.forge/continuity.db`. Only its recallable experience projection moves into `memory_records`; raw execution and reflection events do not become memory records.

### Stable identity

Memory identity remains `<source>:<source_id>` to preserve current action and audit behavior.

Knowledge identity is provider-qualified:

```text
<provider>:<account_namespace>:<remote_or_local_object_id>
```

Chunks use a deterministic identity derived from document identity, heading path, ordinal, and normalized content hash. A changed chunk receives a changed hash, while unchanged chunks retain identity across sync runs.

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
4. Deduplicate exact source identities and normalized content hashes across memory, continuity, local Wiki, and remote chunks.
5. Rank candidates with explicit score components:
   - lexical relevance;
   - optional semantic relevance;
   - project and profile match;
   - pin and user acceptance;
   - source authority;
   - recency and remote freshness;
   - confidence for inferred memories.
6. When online, validate every unique Feishu document that could enter the final selection against current access and revision. Validation may be coalesced within one turn but may not be skipped because a background sync was recent.
7. Allocate the hidden-context budget across memory, project records, local knowledge, remote knowledge, files, and retained transcript.
8. Inject bounded content with distinct section labels and source citations.
9. Persist a body-free recall event containing decisions, scores, revisions, token estimates, and latency.

Remote validation failure must remove the affected candidate before final token allocation. The planner must not spend budget on a candidate it cannot inject.

### Chinese retrieval

The local retrieval path must not depend on whitespace tokenization for Chinese.

The first implementation will provide:

- normalized Unicode text;
- punctuation-aware segmentation;
- Chinese character bigram candidate terms for FTS;
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
- cap inferred memory below authoritative knowledge when both answer the same question;
- inject no more than three chunks from one document unless the user explicitly asks to summarize that document;
- keep body-free audit metadata outside the injected-content count but inside telemetry size limits.

The existing `turn_prepared.context_estimate` bucket model remains the public evidence contract. Implementation may add sub-bucket diagnostics internally, but the primary UI must not expose retrieval internals as product navigation.

## Feishu Read-Only Connector

### Source selection and identity

The user explicitly connects a Feishu account and chooses one or more knowledge spaces. Forge does not enumerate or mirror the entire tenant by default.

The connector stores:

- account namespace without access-token material;
- selected `space_id` values;
- node token, object token, and object type;
- canonical Feishu URL;
- document title and revision;
- last metadata sync, last successful content fetch, and last access validation;
- typed sync or permission status.

Tokens and refresh credentials remain reference-only in the system credential store, following the existing provider credential model. They must never be serialized into `context.db`, transcripts, recall events, diagnostics payloads, or connector content.

### Authentication and authorization

The default identity is a user access token so retrieval reflects the connected user's document access. Bot or tenant identity is allowed only for spaces explicitly shared with the application.

API scopes and resource permissions are separate gates. Passing the OAuth scope check does not prove access to a knowledge space or document.

All cached rows are namespaced by the connected account identity. A candidate from one account namespace must never be visible to another profile, session, gateway owner, or desktop user merely because the remote object id is the same.

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
- Documents absent from a complete successful generation are marked deleted only after the generation commits.
- A partial or failed generation never deletes the last known-good index.
- Rate limits use bounded exponential backoff with jitter and expose the next retry time.

Top-result read-through validation remains independent of the background sync schedule. Every online turn validates each unique Feishu document that could be injected, catching permission revocation and urgent edits before automatic recall uses cached text.

### Cache lifecycle and local disclosure

Remote content is cached only for user-selected spaces. The Settings surface must disclose that selected content is indexed locally and provide:

- pause sync;
- reconnect account;
- remove a space;
- clear one source cache;
- clear all Feishu cached content;
- last success, last error, and stale status.

For the internal macOS slice, the database and containing directory must use owner-only filesystem permissions. The product must not claim encrypted-at-rest remote content until an encryption design and verification gate exist.

Removing a source immediately disables it for retrieval and then deletes its cached documents, chunks, embeddings, ACL bindings, and sync cursors. Deletion failure remains visible in diagnostics and keeps the source disabled.

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
| Unified database unavailable | Fall back to existing readable stores during migration phases; emit diagnostics; do not fabricate recall |
| Migration comparison mismatch | Keep legacy stores authoritative and disable the new read path |
| Feishu authentication expired | Attempt one typed refresh; then mark reconnect required without leaking token details |
| Feishu permission denied | Mark the document inaccessible, exclude it immediately, and schedule cache deletion |
| Feishu rate limited | Keep the last good generation, back off, expose next retry time |
| Network unavailable | Exclude Feishu body candidates, retain local and memory retrieval, and state that remote knowledge could not be verified |
| Unsupported object type | Index metadata only and report the type; do not inject empty content |
| Partial sync | Preserve the previous committed generation and resume from the recorded cursor |
| Chunk or parser failure | Exclude the failed document body, retain metadata, and continue other documents |
| Embedding backend unavailable | Continue with deterministic lexical retrieval |
| Source removed during a turn | Recheck enabled state before final injection and omit the source |

Connector failures are non-fatal to the agent turn unless the user explicitly requested a remote-only operation whose result cannot be obtained. In that case Forge must state that the source could not be verified.

## Rollout Plan

### Phase 1A: Unified retrieval contract

- Add provider-neutral context-candidate, knowledge-source, document, chunk, and recall-query contracts.
- Route existing memory, continuity, Forge Wiki, and connector selections through one budget planner without changing physical authority.
- Preserve current stream and audit compatibility.
- Add cross-source deduplication and Chinese lexical retrieval tests.

### Phase 1B: Shadow SQLite data plane

- Create `context.db` behind a disabled-by-default capability flag.
- Snapshot legacy stores before any migration write.
- Backfill stable ids, candidate records, forgotten suppression tombstones, and authority metadata.
- Dual-read and compare status, actions, recall eligibility, selected ids, and redaction.
- Keep all legacy writes authoritative.

### Phase 1C: Physical memory cutover

- Enable dual writes only after shadow comparisons pass.
- Prove archive, restore, forget, edit, pin, and unpin semantics for each source.
- Keep raw continuity events in the existing project-local event journal while dual-writing the recallable experience projection.
- Cut reads to the new store behind a reversible flag.
- Retain a documented rollback path until at least one release cycle passes acceptance.

### Phase 1D: Local knowledge indexing

- Index Forge Wiki and explicitly selected local Markdown/text documents as knowledge, not memory.
- Add FTS, chunking, citations, freshness, and source deletion.
- Establish the read-only `KnowledgeConnector` contract with a local fixture connector.

### Phase 2A: Feishu read-only internal slice

- Add user authentication and selected-space configuration.
- Sync Wiki node metadata and Docx content.
- Add per-account ACL namespace, incremental generations, read-through validation, and cache deletion.
- Surface source health and cache controls in Settings, not the Work Panel.

### Phase 2B: Retrieval quality hardening

- Add curated Chinese and English Feishu recall cases.
- Tune lexical and optional semantic ranking from measured failures.
- Prove permission revocation, unvalidated-citation rejection, offline fail-closed behavior, and rate-limit recovery.
- Expand object-type support only when a real internal use case requires it.

### Phase 3: Explicit remote writeback proposals

This phase requires a separate user-approved design before implementation.

## Testing and Evidence

### Unit and property tests

- Stable record and chunk identity.
- Project, profile, status, source, and ACL hard filters.
- Chinese bigram candidate generation and phrase boosts.
- Exact-id and normalized-content deduplication.
- Deterministic ranking without embeddings.
- Cross-source token allocation and per-document caps.
- No hidden body in recall audit or migration report.

### Migration tests

- Legacy snapshot creation and restoration.
- Shadow read equivalence for current and archived records.
- Candidate suppression and forgotten-record anti-relearning equivalence.
- Action equivalence for archive, restore, forget, pin, unpin, and fact edit.
- Crash recovery during backfill and dual write.
- Capability-flag rollback to legacy reads.

### Connector integration tests

- Authentication refresh success and failure.
- Space and node pagination.
- Wiki node token to object token/type resolution.
- Docx unchanged revision, changed revision, and unchanged content hash.
- HTTP 403 permission revocation and cache exclusion.
- HTTP 429 bounded backoff.
- Partial generation does not delete prior content.
- Removed source cannot be recalled while cache deletion is pending.
- Unsupported object types remain metadata-only.

### Product acceptance

- A user can connect one account, select one space, sync it, inspect status, pause it, and remove it.
- A question grounded in one Feishu page produces a correct citation.
- A same-title page in an unauthorized account namespace is never selected.
- A changed or revoked top result is revalidated before injection.
- Offline turns do not inject cached Feishu bodies and clearly state that remote knowledge could not be verified when it was requested.
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
- existing memory can migrate through snapshot, shadow, dual-read, dual-write, cutover, and rollback gates;
- local Chinese retrieval no longer depends on whitespace-only matching;
- local project records and remote documents remain distinct from inferred memory;
- a selected Feishu Docx space can be indexed read-only with per-account isolation;
- top remote results are access- and revision-validated before injection;
- answers can cite the authoritative Feishu page;
- source removal and permission revocation prevent future recall and trigger bounded cache deletion;
- acceptance and eval evidence prove no wrong-account or unauthorized injection.

## References

- [Feishu Wiki API overview](https://open.feishu.cn/document/ukTMukTMukTM/uUDN04SN0QjL1QDN/wiki-v2)
- [Feishu Wiki API FAQ](https://open.feishu.cn/document/ukTMukTMukTM/uUDN04SN0QjL1QDN/wiki-v2/wiki-qa)
- [Feishu Docx document API](https://open.feishu.cn/document/server-docs/docs/docs/docx-v1/document/get?lang=zh-CN)
- `apps/desktop/src-tauri/src/memory/unified.rs`
- `apps/desktop/src-tauri/src/ipc/unified_memory.rs`
- `apps/desktop/src-tauri/src/ipc/send_input_context.rs`
- `apps/desktop/src-tauri/src/ipc/mcp_context.rs`
- `apps/desktop/src-tauri/src/forge_wiki/storage.rs`
- `apps/desktop/src-tauri/src/continuity/store.rs`
- `scripts/memory-migration-dry-run.mjs`
