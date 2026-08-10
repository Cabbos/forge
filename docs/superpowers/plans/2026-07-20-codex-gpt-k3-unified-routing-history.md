# Codex GPT–K3 Unified Routing and History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend CCSwitch so one stable Codex provider identity exposes the official GPT catalog plus Kimi K3, routes each request by model to the correct upstream, and preserves a single visible task history.

**Architecture:** A small routing configuration on the active Codex provider names the OpenAI Official target and exact third-party routes. A new resolver merges the official Codex model cache with exact K3 entries, selects the upstream before forwarding, and fails closed for unknown models. Existing CCSwitch unified-history migration moves OpenAI sessions into the shared `custom` bucket; a route-pin table prevents an existing task from crossing provider families.

**Tech Stack:** Rust, Tauri 2, SQLite, React/TypeScript, Vitest, Cargo tests, CCSwitch 3.17-compatible provider/proxy architecture, Codex Responses API, Kimi Chat Completions conversion

---

## Repository and Runtime Map

- Design authority: `/Users/cabbos/project/forge/docs/superpowers/specs/2026-07-20-codex-gpt-k3-unified-routing-history-design.md`
- CCSwitch primary clone: `/Users/cabbos/project/cc-switch`
- CCSwitch implementation worktree: `/Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing`
- CCSwitch source base: `https://github.com/farion1231/cc-switch.git`, commit `613fef70bc7d5e35299b4131935f738c85765b35`
- Side-by-side app: `/Applications/CC Switch Unified Test.app`
- Production app: `/Applications/CC Switch.app`
- Production database: `/Users/cabbos/.cc-switch/cc-switch.db`
- Codex state database: `/Users/cabbos/.codex/state_5.sqlite`
- Codex sessions: `/Users/cabbos/.codex/sessions/` and `/Users/cabbos/.codex/archived_sessions/`
- OpenAI model cache: `/Users/cabbos/.codex/models_cache.json`
- Generated merged catalog: `/Users/cabbos/.codex/cc-switch-model-catalog.json`

No Forge application source file changes during implementation. Only this plan and its design live in Forge.

## Fixed Production Identities

The current CCSwitch database contains these Codex provider IDs:

```text
Kimi Bridge:     a15a786a-9670-46f2-9081-208d74cb103f
OpenAI Official: codex-official
Default:         default
```

The production routing metadata is:

```json
{
  "openaiProviderId": "codex-official",
  "exactRoutes": {
    "k3": "a15a786a-9670-46f2-9081-208d74cb103f"
  }
}
```

### Task 1: Create an Isolated CCSwitch Worktree and Establish the Baseline

**Files:**
- Create repository: `/Users/cabbos/project/cc-switch`
- Create worktree: `/Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing`
- Read: `/Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing/package.json`
- Read: `/Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing/src-tauri/Cargo.toml`

- [ ] **Step 1: Clone the pinned source without modifying the installed app**

```bash
git clone https://github.com/farion1231/cc-switch.git /Users/cabbos/project/cc-switch
git -C /Users/cabbos/project/cc-switch fetch origin
git -C /Users/cabbos/project/cc-switch checkout 613fef70bc7d5e35299b4131935f738c85765b35
test "$(git -C /Users/cabbos/project/cc-switch rev-parse HEAD)" = "613fef70bc7d5e35299b4131935f738c85765b35"
```

Expected: the final assertion exits `0`.

- [ ] **Step 2: Create the feature worktree and branch**

```bash
git -C /Users/cabbos/project/cc-switch worktree add \
  -b codex/codex-unified-model-routing \
  /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing \
  613fef70bc7d5e35299b4131935f738c85765b35
```

Expected: a named branch exists in the isolated worktree.

- [ ] **Step 3: Install the exact JavaScript dependencies**

```bash
cd /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing
corepack pnpm install --frozen-lockfile
```

Expected: dependency installation exits `0` without changing `pnpm-lock.yaml`.

- [ ] **Step 4: Run the relevant baseline suites**

```bash
cd /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing
corepack pnpm typecheck
corepack pnpm test:unit
cargo test --manifest-path src-tauri/Cargo.toml proxy::provider_router --lib
cargo test --manifest-path src-tauri/Cargo.toml codex_history_migration --lib
cargo test --manifest-path src-tauri/Cargo.toml codex_config --lib
```

Expected: all commands exit `0`. Stop before implementation if the pinned baseline is not green.

### Task 2: Add Typed Model-Routing Configuration

**Files:**
- Modify: `src-tauri/src/provider.rs`
- Modify: `src/types.ts`
- Test: `src-tauri/src/provider.rs`

- [ ] **Step 1: Write backend serialization tests**

Add these tests to the existing `provider.rs` test module:

```rust
#[test]
fn provider_meta_roundtrips_codex_model_routing() {
    let meta = ProviderMeta {
        codex_model_routing: Some(CodexModelRoutingConfig {
            openai_provider_id: "codex-official".to_string(),
            exact_routes: HashMap::from([(
                "k3".to_string(),
                "kimi-provider".to_string(),
            )]),
        }),
        ..ProviderMeta::default()
    };
    let value = serde_json::to_value(&meta).expect("serialize routing meta");
    assert_eq!(
        value["codexModelRouting"]["openaiProviderId"],
        "codex-official"
    );
    assert_eq!(
        value["codexModelRouting"]["exactRoutes"]["k3"],
        "kimi-provider"
    );
    let decoded: ProviderMeta = serde_json::from_value(value).expect("decode routing meta");
    assert_eq!(decoded, meta);
}

#[test]
fn provider_meta_omits_absent_codex_model_routing() {
    let value = serde_json::to_value(ProviderMeta::default()).expect("serialize meta");
    assert!(value.get("codexModelRouting").is_none());
}
```

- [ ] **Step 2: Run the tests and confirm the missing types fail compilation**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  provider_meta_roundtrips_codex_model_routing --lib
```

Expected: compilation fails because `CodexModelRoutingConfig` and `codex_model_routing` do not exist.

- [ ] **Step 3: Add the backend types**

Add before `ProviderMeta`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexModelRoutingConfig {
    #[serde(rename = "openaiProviderId")]
    pub openai_provider_id: String,
    #[serde(default, rename = "exactRoutes")]
    pub exact_routes: HashMap<String, String>,
}
```

Add inside `ProviderMeta`:

```rust
#[serde(rename = "codexModelRouting", skip_serializing_if = "Option::is_none")]
pub codex_model_routing: Option<CodexModelRoutingConfig>,
```

- [ ] **Step 4: Add the matching frontend type**

Add to `src/types.ts`:

```ts
export interface CodexModelRoutingConfig {
  openaiProviderId: string;
  exactRoutes: Record<string, string>;
}
```

Add inside `ProviderMeta`:

```ts
codexModelRouting?: CodexModelRoutingConfig;
```

- [ ] **Step 5: Run focused and type tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  provider_meta_roundtrips_codex_model_routing --lib
cargo test --manifest-path src-tauri/Cargo.toml \
  provider_meta_omits_absent_codex_model_routing --lib
corepack pnpm typecheck
```

Expected: all commands exit `0`.

- [ ] **Step 6: Commit the typed configuration**

```bash
git add src-tauri/src/provider.rs src/types.ts
git commit -m "feat(codex): add model routing metadata"
```

### Task 3: Build the Last-Known-Good OpenAI Catalog Resolver

**Files:**
- Create: `src-tauri/src/codex_model_routing.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/codex_model_routing.rs`

- [ ] **Step 1: Write resolver tests with a temporary model cache**

Create the module with this test scaffold:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    fn routing() -> CodexModelRoutingConfig {
        CodexModelRoutingConfig {
            openai_provider_id: "codex-official".to_string(),
            exact_routes: HashMap::from([(
                "k3".to_string(),
                "kimi-provider".to_string(),
            )]),
        }
    }

    #[test]
    fn resolves_exact_and_cached_openai_models() {
        let dir = tempdir().expect("temp dir");
        let cache = dir.path().join("models_cache.json");
        fs::write(
            &cache,
            serde_json::to_vec(&json!({
                "models": [
                    {"slug": "gpt-5.5", "display_name": "GPT-5.5"},
                    {"slug": "gpt-5.6-sol", "display_name": "GPT-5.6 Sol"}
                ]
            }))
            .expect("cache json"),
        )
        .expect("write cache");

        let resolved = ResolvedCodexModelRouting::load(&routing(), &cache)
            .expect("load model routes");
        assert_eq!(resolved.provider_for("k3"), Some("kimi-provider"));
        assert_eq!(resolved.provider_for("gpt-5.5"), Some("codex-official"));
        assert_eq!(resolved.provider_for("unknown-model"), None);
        assert_eq!(resolved.openai_entries().len(), 2);
    }

    #[test]
    fn exact_routes_win_only_when_cache_has_no_conflicting_slug() {
        let dir = tempdir().expect("temp dir");
        let cache = dir.path().join("models_cache.json");
        fs::write(&cache, r#"{"models":[{"slug":"k3"}]}"#).expect("write cache");
        let error = ResolvedCodexModelRouting::load(&routing(), &cache)
            .expect_err("conflicting route must fail");
        assert!(error.to_string().contains("k3"));
    }

    #[test]
    fn invalid_refresh_keeps_last_known_good_catalog() {
        let dir = tempdir().expect("temp dir");
        let cache = dir.path().join("models_cache.json");
        let snapshot = dir.path().join("routing-catalog-last-good.json");
        fs::write(&cache, r#"{"models":[{"slug":"gpt-5.5"}]}"#).expect("write cache");
        let first = load_with_last_good(&routing(), &cache, &snapshot).expect("first load");
        assert_eq!(first.provider_for("gpt-5.5"), Some("codex-official"));
        fs::write(&cache, b"not-json").expect("break cache");
        let fallback = load_with_last_good(&routing(), &cache, &snapshot).expect("fallback");
        assert_eq!(fallback.provider_for("gpt-5.5"), Some("codex-official"));
    }
}
```

- [ ] **Step 2: Run the resolver tests and verify they fail**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_model_routing::tests --lib
```

Expected: compilation fails because the resolver is not implemented.

- [ ] **Step 3: Implement the resolver**

Implement these public types and functions in the same file:

```rust
use crate::error::AppError;
use crate::provider::CodexModelRoutingConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCodexModelRouting {
    routes: BTreeMap<String, String>,
    openai_entries: Vec<Value>,
}

impl ResolvedCodexModelRouting {
    pub fn load(config: &CodexModelRoutingConfig, cache_path: &Path) -> Result<Self, AppError> {
        let bytes = fs::read(cache_path).map_err(|error| AppError::io(cache_path, error))?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| AppError::json(cache_path, error))?;
        let entries = value
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(|| AppError::Message("OpenAI model cache has no models array".into()))?;

        let mut routes = BTreeMap::new();
        let mut openai_entries = Vec::new();
        for entry in entries {
            let slug = entry
                .get("slug")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|slug| !slug.is_empty())
                .ok_or_else(|| AppError::Message("OpenAI catalog entry has no slug".into()))?;
            if config.exact_routes.contains_key(slug) {
                return Err(AppError::Message(format!(
                    "Codex model route conflict for {slug}"
                )));
            }
            routes.insert(slug.to_string(), config.openai_provider_id.clone());
            openai_entries.push(entry.clone());
        }
        for (slug, provider_id) in &config.exact_routes {
            let slug = slug.trim();
            let provider_id = provider_id.trim();
            if slug.is_empty() || provider_id.is_empty() {
                return Err(AppError::Message(
                    "Codex exact model routes require non-empty slug and provider id".into(),
                ));
            }
            routes.insert(slug.to_string(), provider_id.to_string());
        }
        Ok(Self {
            routes,
            openai_entries,
        })
    }

    pub fn provider_for(&self, slug: &str) -> Option<&str> {
        self.routes.get(slug).map(String::as_str)
    }

    pub fn openai_entries(&self) -> &[Value] {
        &self.openai_entries
    }
}

pub fn load_with_last_good(
    config: &CodexModelRoutingConfig,
    cache_path: &Path,
    snapshot_path: &Path,
) -> Result<ResolvedCodexModelRouting, AppError> {
    match ResolvedCodexModelRouting::load(config, cache_path) {
        Ok(resolved) => {
            let bytes = serde_json::to_vec_pretty(&resolved)
                .map_err(|error| AppError::Message(error.to_string()))?;
            crate::config::atomic_write(snapshot_path, &bytes)?;
            Ok(resolved)
        }
        Err(refresh_error) => {
            let bytes = fs::read(snapshot_path)
                .map_err(|_| refresh_error)?;
            serde_json::from_slice(&bytes)
                .map_err(|error| AppError::json(snapshot_path, error))
        }
    }
}
```

`atomic_write` is defined in `src-tauri/src/config.rs`; do not replace it with a direct `fs::write`.

- [ ] **Step 4: Register the module**

Add to `src-tauri/src/lib.rs`:

```rust
mod codex_model_routing;
```

- [ ] **Step 5: Run tests and formatting**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_model_routing::tests --lib
```

Expected: all three resolver tests pass.

- [ ] **Step 6: Commit the resolver**

```bash
git add src-tauri/src/codex_model_routing.rs src-tauri/src/lib.rs
git commit -m "feat(codex): resolve routed model catalogs"
```

### Task 4: Select the Upstream Provider from the Requested Model

**Files:**
- Modify: `src-tauri/src/proxy/provider_router.rs`
- Modify: `src-tauri/src/proxy/handler_context.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/proxy/handlers.rs`
- Test: `src-tauri/src/proxy/provider_router.rs`
- Test: `src-tauri/src/proxy/handlers.rs`

- [ ] **Step 1: Add failing provider-router tests**

Add tests that create active `Kimi Bridge`, target `codex-official`, and an OpenAI cache containing `gpt-5.5`:

```rust
#[tokio::test]
async fn codex_model_routing_selects_exact_kimi_and_cached_openai_targets() {
    let fixture = ModelRoutingFixture::new().await;
    let kimi = fixture
        .router
        .select_providers_for_model("codex", "k3")
        .await
        .expect("route k3");
    assert_eq!(kimi[0].id, "kimi-provider");

    let openai = fixture
        .router
        .select_providers_for_model("codex", "gpt-5.5")
        .await
        .expect("route gpt");
    assert_eq!(openai[0].id, "codex-official");
}

#[tokio::test]
async fn codex_model_routing_rejects_unknown_slug_without_fallback() {
    let fixture = ModelRoutingFixture::new().await;
    let error = fixture
        .router
        .select_providers_for_model("codex", "unknown-model")
        .await
        .expect_err("unknown model must fail closed");
    assert!(matches!(error, AppError::NoCodexModelRoute(model) if model == "unknown-model"));
}

#[tokio::test]
async fn non_codex_apps_keep_existing_provider_selection() {
    let fixture = ModelRoutingFixture::new().await;
    let providers = fixture
        .router
        .select_providers_for_model("claude", "gpt-5.5")
        .await
        .expect("legacy selection");
    assert_eq!(providers[0].id, fixture.claude_current_provider_id);
}
```

The fixture must use a temporary Codex config directory and set its `models_cache.json`; it must not read the user's home directory.

- [ ] **Step 2: Run the tests and confirm the new method is missing**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_model_routing_selects_exact_kimi_and_cached_openai_targets --lib
```

Expected: compilation fails because `select_providers_for_model` and the error variant do not exist.

- [ ] **Step 3: Add fail-closed errors**

Add to `AppError`:

```rust
NoCodexModelRoute(String),
CodexModelRouteProviderMissing { model: String, provider_id: String },
```

Format them as:

```text
No configured Codex upstream route for model: {model}
Codex model {model} routes to missing provider: {provider_id}
```

- [ ] **Step 4: Implement request-model selection**

Add a method next to `select_providers`:

```rust
pub async fn select_providers_for_model(
    &self,
    app_type: &str,
    request_model: &str,
) -> Result<Vec<Provider>, AppError> {
    if app_type != "codex" {
        return self.select_providers(app_type).await;
    }
    let current = self.current_provider(app_type)?;
    let Some(config) = current
        .meta
        .as_ref()
        .and_then(|meta| meta.codex_model_routing.as_ref())
    else {
        return self.select_providers(app_type).await;
    };
    let cache_path = crate::app_config::get_codex_config_dir().join("models_cache.json");
    let snapshot_path = crate::app_config::get_cc_switch_config_dir()
        .join("cache")
        .join("codex-routed-catalog-last-good.json");
    let resolved = crate::codex_model_routing::load_with_last_good(
        config,
        &cache_path,
        &snapshot_path,
    )?;
    let provider_id = resolved
        .provider_for(request_model)
        .ok_or_else(|| AppError::NoCodexModelRoute(request_model.to_string()))?;
    let provider = self
        .db
        .get_provider_by_id(provider_id, app_type)?
        .ok_or_else(|| AppError::CodexModelRouteProviderMissing {
            model: request_model.to_string(),
            provider_id: provider_id.to_string(),
        })?;
    Ok(vec![provider])
}
```

Extract the existing current-provider lookup into a private `current_provider` helper rather than duplicating it. Routed mode intentionally bypasses cross-family failover.

- [ ] **Step 5: Wire the request model into `HandlerContext`**

Replace the current call:

```rust
.select_providers(app_type_str)
```

with:

```rust
.select_providers_for_model(app_type_str, &request_model)
```

Map both new `AppError` variants to a dedicated proxy error that returns HTTP `400` with code `cc_switch_codex_model_route_missing`. The JSON body includes the requested model but no provider credential or request body.

- [ ] **Step 6: Run router and error-response tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml proxy::provider_router --lib
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_proxy_forward_error_includes_context_and_cause --lib
```

Expected: routing tests pass; existing provider selection and error tests remain green.

- [ ] **Step 7: Commit model-aware selection**

```bash
git add src-tauri/src/proxy/provider_router.rs \
  src-tauri/src/proxy/handler_context.rs \
  src-tauri/src/proxy/handlers.rs \
  src-tauri/src/error.rs
git commit -m "feat(codex): route requests by model"
```

### Task 5: Generate a Union Catalog Without Losing Official Metadata

**Files:**
- Modify: `src-tauri/src/codex_model_routing.rs`
- Modify: `src-tauri/src/codex_config.rs`
- Modify: `src-tauri/src/services/proxy.rs`
- Test: `src-tauri/src/codex_model_routing.rs`
- Test: `src-tauri/src/codex_config.rs`

- [ ] **Step 1: Write a failing catalog-union test**

Add:

```rust
#[test]
fn merged_catalog_preserves_official_entries_and_appends_exact_models() {
    let official = vec![json!({
        "slug": "gpt-5.5",
        "display_name": "GPT-5.5",
        "context_window": 400000,
        "supports_parallel_tool_calls": true
    })];
    let exact = HashMap::from([(
        "k3".to_string(),
        json!({
            "slug": "k3",
            "display_name": "Kimi K3",
            "context_window": 262144,
            "supports_parallel_tool_calls": true
        }),
    )]);
    let merged = merge_catalog_entries(&official, &exact).expect("merge catalog");
    assert_eq!(merged[0]["slug"], "gpt-5.5");
    assert_eq!(merged[0]["context_window"], 400000);
    assert_eq!(merged[1]["slug"], "k3");
}
```

- [ ] **Step 2: Run the test and verify the merge function is missing**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  merged_catalog_preserves_official_entries_and_appends_exact_models --lib
```

Expected: compilation fails because `merge_catalog_entries` does not exist.

- [ ] **Step 3: Implement deterministic union logic**

Implement:

```rust
pub fn merge_catalog_entries(
    official_entries: &[Value],
    exact_entries: &HashMap<String, Value>,
) -> Result<Vec<Value>, AppError> {
    let mut seen = HashSet::new();
    let mut merged = Vec::with_capacity(official_entries.len() + exact_entries.len());
    for entry in official_entries {
        let slug = entry
            .get("slug")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Message("Catalog entry has no slug".into()))?;
        if !seen.insert(slug.to_string()) {
            return Err(AppError::Message(format!(
                "Duplicate official catalog slug {slug}"
            )));
        }
        merged.push(entry.clone());
    }
    let mut exact_slugs = exact_entries.keys().cloned().collect::<Vec<_>>();
    exact_slugs.sort();
    for slug in exact_slugs {
        if !seen.insert(slug.clone()) {
            return Err(AppError::Message(format!(
                "Catalog route conflict for {slug}"
            )));
        }
        merged.push(exact_entries[&slug].clone());
    }
    Ok(merged)
}
```

Import `HashSet` beside `HashMap`. This preserves official cache order and appends exact-route slugs in lexical order.

- [ ] **Step 4: Add routed catalog projection**

Add a `project_routed_codex_catalog` function that:

1. Loads the active provider's `codexModelRouting` metadata.
2. Loads the last-known-good official entries.
3. Changes `codex_model_catalog_from_settings` visibility from private to `pub(crate)` and uses that existing converter with the target provider's `settings_config`, config text, and `CodexCatalogToolProfile`. Do not read `modelCatalog` rows as native Codex entries: CCSwitch rows use `model`/`displayName`, while generated Codex entries use `slug`/`display_name`.
4. Extracts the converted catalog's `models` array and requires exactly one generated row whose `slug` matches each exact route.
5. Writes `{ "models": merged_entries }` atomically to `cc-switch-model-catalog.json`.
6. Leaves the previous file untouched on any error.

Use this signature:

```rust
pub fn project_routed_codex_catalog(
    db: &crate::database::Database,
    active_provider: &Provider,
    codex_dir: &Path,
) -> Result<Option<PathBuf>, AppError>;
```

- [ ] **Step 5: Invoke projection after Codex takeover updates the live config**

In `src-tauri/src/services/proxy.rs`, call `project_routed_codex_catalog` only when the active provider has `codexModelRouting`. Ensure the live `config.toml` contains:

```toml
model_provider = "custom"
model_catalog_json = "cc-switch-model-catalog.json"
```

The existing non-routed provider projection remains unchanged.

- [ ] **Step 6: Test official metadata preservation and atomic fallback**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_model_routing::tests --lib
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_config --lib
```

Expected: official fields such as context window and parallel-tool support remain byte-for-byte equal to `models_cache.json`; malformed refresh retains the last-good file.

- [ ] **Step 7: Commit catalog projection**

```bash
git add src-tauri/src/codex_model_routing.rs \
  src-tauri/src/codex_config.rs \
  src-tauri/src/services/proxy.rs
git commit -m "feat(codex): project unified GPT and K3 catalog"
```

### Task 6: Persist Provider-Family Pins for Every Thread

**Files:**
- Modify: `src-tauri/src/database/schema.rs`
- Create: `src-tauri/src/database/dao/codex_thread_routes.rs`
- Modify: `src-tauri/src/database/dao/mod.rs`
- Modify: `src-tauri/src/database/mod.rs`
- Test: `src-tauri/src/database/tests.rs`

- [ ] **Step 1: Add a failing schema and DAO test**

```rust
#[test]
fn codex_thread_route_pins_roundtrip() {
    let db = Database::new_in_memory().expect("database");
    db.pin_codex_thread_route("thread-gpt", "openai", "gpt-5.5")
        .expect("pin gpt thread");
    db.pin_codex_thread_route("thread-k3", "kimi", "k3")
        .expect("pin k3 thread");
    let gpt = db
        .get_codex_thread_route_pin("thread-gpt")
        .expect("read pin")
        .expect("gpt pin");
    assert_eq!(gpt.provider_family, "openai");
    assert_eq!(gpt.model, "gpt-5.5");
    let conflict = db.pin_codex_thread_route("thread-gpt", "kimi", "k3");
    assert!(conflict.is_err());
}
```

- [ ] **Step 2: Run the test and verify the DAO is missing**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_thread_route_pins_roundtrip --lib
```

Expected: compilation fails on the missing DAO methods.

- [ ] **Step 3: Add the table**

Create this table from `schema.rs`:

```sql
CREATE TABLE IF NOT EXISTS codex_thread_route_pins (
    thread_id TEXT PRIMARY KEY,
    provider_family TEXT NOT NULL CHECK(provider_family IN ('openai', 'kimi')),
    model TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 4: Implement the DAO**

Create:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexThreadRoutePin {
    pub thread_id: String,
    pub provider_family: String,
    pub model: String,
}
```

Implement:

```rust
pub fn pin_codex_thread_route(
    &self,
    thread_id: &str,
    provider_family: &str,
    model: &str,
) -> Result<(), AppError>;

pub fn get_codex_thread_route_pin(
    &self,
    thread_id: &str,
) -> Result<Option<CodexThreadRoutePin>, AppError>;
```

An existing identical family is idempotent and may update the model. A different family returns an error and does not update the row.

- [ ] **Step 5: Export the DAO and run database tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  codex_thread_route_pins_roundtrip --lib
cargo test --manifest-path src-tauri/Cargo.toml database --lib
```

Expected: the new test and existing database tests pass.

- [ ] **Step 6: Commit route pins**

```bash
git add src-tauri/src/database/schema.rs \
  src-tauri/src/database/dao/codex_thread_routes.rs \
  src-tauri/src/database/dao/mod.rs \
  src-tauri/src/database/mod.rs \
  src-tauri/src/database/tests.rs
git commit -m "feat(codex): persist thread provider families"
```

### Task 7: Seed Pins During History Migration and Block Cross-Family Resume

**Files:**
- Modify: `src-tauri/src/codex_history_migration.rs`
- Modify: `src-tauri/src/proxy/handler_context.rs`
- Modify: `src-tauri/src/proxy/response_processor.rs`
- Modify: `src-tauri/src/error.rs`
- Test: `src-tauri/src/codex_history_migration.rs`
- Test: `src-tauri/src/proxy/handler_context.rs`

- [ ] **Step 1: Add a migration test that preserves route families**

```rust
#[test]
fn unified_history_migration_seeds_openai_and_kimi_route_pins() {
    let fixture = UnifiedHistoryFixture::new();
    fixture.insert_thread("gpt-thread", "openai", "gpt-5.5");
    fixture.insert_thread("k3-thread", "custom", "k3");
    let outcome = fixture.run_migration().expect("migrate history");
    assert_eq!(outcome.migrated_state_rows, 1);
    assert_eq!(
        fixture.pin("gpt-thread").provider_family,
        "openai"
    );
    assert_eq!(fixture.pin("k3-thread").provider_family, "kimi");
    assert_eq!(fixture.thread_provider("gpt-thread"), "custom");
    assert_eq!(fixture.thread_provider("k3-thread"), "custom");
}
```

- [ ] **Step 2: Run the migration test and verify pins are absent**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  unified_history_migration_seeds_openai_and_kimi_route_pins --lib
```

Expected: test fails because migration does not seed route pins.

- [ ] **Step 3: Seed pins before rewriting provider identity**

Before `migrate_codex_state_dbs` updates `openai` to `custom`, query thread ID and model from each source state database. In one CCSwitch database transaction:

- seed `openai` threads as family `openai`;
- seed existing `custom` threads whose model resolves to exact Kimi route as family `kimi`;
- mark an empty-model OpenAI thread as family `openai` with model `unknown-openai-model`;
- abort migration if an existing pin conflicts.

Reuse the existing guarded migration path: back up each affected JSONL file, verify its size and modification time have not changed, atomically rewrite only `session_meta.model_provider`, and leave every conversation-event line byte-for-byte unchanged.

- [ ] **Step 4: Add a failing cross-family guard test**

```rust
#[tokio::test]
async fn handler_context_blocks_cross_family_resume_before_forwarding() {
    let fixture = HandlerFixture::with_routed_models().await;
    fixture.pin_thread("thread-1", "openai", "gpt-5.5");
    let error = fixture
        .context_for("thread-1", "k3")
        .await
        .expect_err("cross-family resume must fail");
    assert!(matches!(
        error,
        ProxyError::CrossProviderResumeBlocked { thread_id, .. }
        if thread_id == "thread-1"
    ));
    assert_eq!(fixture.forwarded_request_count(), 0);
}
```

- [ ] **Step 5: Implement the request guard**

After extracting the session ID and resolving the target provider family, read the pin:

```rust
if let Some(pin) = state
    .db
    .get_codex_thread_route_pin(&session_id)
    .map_err(|error| ProxyError::DatabaseError(error.to_string()))?
{
    if pin.provider_family != resolved_family {
        return Err(ProxyError::CrossProviderResumeBlocked {
            thread_id: session_id.clone(),
            original_family: pin.provider_family,
            requested_family: resolved_family,
        });
    }
}
```

Return HTTP `409` with code `cc_switch_cross_provider_resume_blocked` and the instruction `Create a new Codex task to change provider families.`

- [ ] **Step 6: Pin new threads only after a successful upstream response**

In `response_processor.rs`, after the request is known to have succeeded, call `pin_codex_thread_route` with the session ID, resolved provider family, and outbound model. Failed requests must not create pins.

- [ ] **Step 7: Run migration, handler, and response tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml codex_history_migration --lib
cargo test --manifest-path src-tauri/Cargo.toml \
  handler_context_blocks_cross_family_resume_before_forwarding --lib
cargo test --manifest-path src-tauri/Cargo.toml proxy::response_processor --lib
```

Expected: all tests pass; the guard proves zero forwarded requests.

- [ ] **Step 8: Commit history safety**

```bash
git add src-tauri/src/codex_history_migration.rs \
  src-tauri/src/proxy/handler_context.rs \
  src-tauri/src/proxy/response_processor.rs \
  src-tauri/src/error.rs
git commit -m "feat(codex): preserve provider family across history merge"
```

### Task 8: Add End-to-End Routing Fixtures

**Files:**
- Create: `src-tauri/tests/codex_unified_model_routing.rs`
- Modify: `src-tauri/Cargo.toml` only if an existing dev dependency cannot provide mock HTTP servers

- [ ] **Step 1: Create two mock upstreams**

The integration test starts:

- an OpenAI Responses mock that accepts only `gpt-5.5` and records the incoming OAuth authorization header;
- a Kimi Chat Completions mock that accepts only `k3`, checks that CCSwitch used the Kimi provider credential, and returns an SSE tool-call sequence.

Use loopback-only ephemeral ports and synthetic credentials:

```rust
const TEST_OAUTH: &str = "oauth-test-token";
const TEST_KIMI_KEY: &str = "kimi-test-key";
```

- [ ] **Step 2: Write the failing GPT and K3 routing test**

```rust
#[tokio::test]
async fn unified_catalog_routes_gpt_and_k3_to_distinct_upstreams() {
    let fixture = UnifiedRoutingFixture::start().await;
    let gpt = fixture.responses_request("gpt-5.5", TEST_OAUTH).await;
    assert_eq!(gpt.status(), 200);
    assert_eq!(fixture.openai_requests(), 1);
    assert_eq!(fixture.kimi_requests(), 0);

    let k3 = fixture.responses_request("k3", TEST_OAUTH).await;
    assert_eq!(k3.status(), 200);
    assert_eq!(fixture.openai_requests(), 1);
    assert_eq!(fixture.kimi_requests(), 1);
    assert!(fixture.kimi_saw_api_key(TEST_KIMI_KEY));
    assert!(!fixture.kimi_saw_api_key(TEST_OAUTH));
}
```

- [ ] **Step 3: Add unknown-model and cross-family tests**

```rust
#[tokio::test]
async fn unified_router_fails_closed_without_touching_upstreams() {
    let fixture = UnifiedRoutingFixture::start().await;
    let response = fixture.responses_request("not-configured", TEST_OAUTH).await;
    assert_eq!(response.status(), 400);
    assert_eq!(fixture.openai_requests(), 0);
    assert_eq!(fixture.kimi_requests(), 0);
}

#[tokio::test]
async fn provider_family_guard_returns_conflict_without_network_io() {
    let fixture = UnifiedRoutingFixture::start().await;
    fixture.pin_thread("existing-gpt", "openai", "gpt-5.5");
    let response = fixture
        .responses_request_for_thread("existing-gpt", "k3", TEST_OAUTH)
        .await;
    assert_eq!(response.status(), 409);
    assert_eq!(fixture.openai_requests(), 0);
    assert_eq!(fixture.kimi_requests(), 0);
}
```

- [ ] **Step 4: Run the integration target**

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  --test codex_unified_model_routing -- --nocapture
```

Expected: all integration tests pass and no real network endpoint is contacted.

- [ ] **Step 5: Run full static and unit verification**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
corepack pnpm typecheck
corepack pnpm test:unit
corepack pnpm format:check
```

Expected: all commands exit `0`.

- [ ] **Step 6: Commit integration coverage**

```bash
git add src-tauri/tests/codex_unified_model_routing.rs src-tauri/Cargo.toml
git commit -m "test(codex): cover unified GPT and K3 routing"
```

### Task 9: Build and Validate a Side-by-Side Test Application

**Files:**
- Modify only on the test branch: `src-tauri/tauri.conf.json`
- Build output: `src-tauri/target/release/bundle/macos/CC Switch Unified Test.app`
- Create test home: `/tmp/ccswitch-unified-test-home-*`

- [ ] **Step 1: Give the test build a distinct identity**

In the feature worktree only, change the test build product name and bundle identifier:

```json
{
  "productName": "CC Switch Unified Test",
  "identifier": "io.ccswitch.unified-test"
}
```

Do not commit this packaging-only edit.

- [ ] **Step 2: Build the test application**

```bash
cd /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing
corepack pnpm build
```

Expected: the macOS application bundle is produced successfully.

- [ ] **Step 3: Create a credential-free disposable runtime**

```bash
test_root="$(mktemp -d /tmp/ccswitch-unified-test-home.XXXXXX)"
mkdir -m 700 "$test_root/.cc-switch" "$test_root/.codex"
sqlite3 "$test_root/.cc-switch/cc-switch.db" 'VACUUM;'
jq '{client_version,etag,fetched_at,models}' \
  /Users/cabbos/.codex/models_cache.json \
  > "$test_root/.codex/models_cache.json"
chmod 600 "$test_root/.codex/models_cache.json"
printf '%s\n' "$test_root" > /tmp/ccswitch-unified-test-home-path
```

Do not copy the production CCSwitch database or Codex authentication file into the test home.

- [ ] **Step 4: Initialize and seed the isolated database**

Start the test executable with the disposable `HOME`, wait for schema initialization, then stop only that test process:

```bash
test_root="$(sed -n '1p' /tmp/ccswitch-unified-test-home-path)"
test_exe="/Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing/src-tauri/target/release/bundle/macos/CC Switch Unified Test.app/Contents/MacOS/CC Switch Unified Test"
HOME="$test_root" "$test_exe" &
test_pid=$!
for attempt in $(jot 100 1); do
  sqlite3 "$test_root/.cc-switch/cc-switch.db" \
    "SELECT 1 FROM sqlite_master WHERE type='table' AND name='providers';" \
    | rg -q '^1$' && break
  sleep 0.1
done
test "$(sqlite3 "$test_root/.cc-switch/cc-switch.db" "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='providers';")" = "1"
kill "$test_pid"
wait "$test_pid" || true
```

Insert only synthetic provider data:

```bash
sqlite3 "$test_root/.cc-switch/cc-switch.db" <<'SQL'
BEGIN IMMEDIATE;
INSERT INTO providers (id, app_type, name, settings_config, meta, is_current)
VALUES (
  'codex-official', 'codex', 'OpenAI Official Test',
  json('{"auth":{"OPENAI_API_KEY":"oauth-test-token"},"config":"model_provider = \"openai\"\nmodel = \"gpt-5.5\"\n"}'),
  '{}', 0
);
INSERT INTO providers (id, app_type, name, settings_config, meta, is_current)
VALUES (
  'kimi-provider', 'codex', 'Kimi Bridge Test',
  json('{"auth":{"OPENAI_API_KEY":"kimi-test-key"},"config":"model_provider = \"custom\"\nmodel = \"k3\"\n\n[model_providers.custom]\nname = \"kimi_test\"\nbase_url = \"http://127.0.0.1:9/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n","modelCatalog":{"models":[{"model":"k3","displayName":"Kimi K3","contextWindow":262144}]}}'),
  json('{"codexModelRouting":{"openaiProviderId":"codex-official","exactRoutes":{"k3":"kimi-provider"}}}'),
  1
);
COMMIT;
SQL
```

Expected: both inserts commit and all stored keys are the synthetic Task 8 values.

- [ ] **Step 5: Launch the side-by-side app from the build output**

```bash
test_root="$(sed -n '1p' /tmp/ccswitch-unified-test-home-path)"
test_exe="/Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing/src-tauri/target/release/bundle/macos/CC Switch Unified Test.app/Contents/MacOS/CC Switch Unified Test"
HOME="$test_root" "$test_exe"
```

Expected: the test app launches under the distinct bundle identity, selects `Kimi Bridge Test`, and does not change `/Users/cabbos/.cc-switch` or `/Users/cabbos/.codex`.

- [ ] **Step 6: Verify the disposable catalog and model picker**

Using a disposable Codex configuration directory, verify:

```bash
jq -e '[.models[].slug] | index("gpt-5.5") != null and index("k3") != null' \
  "$test_root/.codex/cc-switch-model-catalog.json" >/dev/null
```

In the test CCSwitch UI, verify diagnostics report a valid merged catalog with both slugs. Routing behavior remains covered by the loopback-only integration suite in Task 8. Do not launch the production ChatGPT/Codex GUI with the disposable home; the real picker is verified only after the confirmation-gated production migration in Task 11.

- [ ] **Step 7: Remove packaging-only edits**

```bash
git restore -- src-tauri/tauri.conf.json
git status --short
```

Expected: only intended committed source changes remain; packaging identity changes are absent.

### Task 10: Prepare Production Backups and Routing Metadata

**Files:**
- Create: `/Users/cabbos/.cc-switch/backups/codex-unified-routing-YYYYMMDD-HHMMSS/`
- Modify after confirmation: `/Users/cabbos/.cc-switch/cc-switch.db`
- Modify through CCSwitch UI: unified Codex history setting

- [ ] **Step 1: Stop before production mutation and request confirmation**

Present the exact planned production changes:

- replace or run a custom CCSwitch build;
- enable unified history and migrate existing OpenAI sessions;
- add model-routing metadata to `Kimi Bridge`;
- keep ChatGPT OAuth and the Kimi credential in their current stores.

Do not continue until the user confirms at action time.

- [ ] **Step 2: Create a private production backup**

```bash
backup_dir="/Users/cabbos/.cc-switch/backups/codex-unified-routing-$(date +%Y%m%d-%H%M%S)"
mkdir -m 700 "$backup_dir"
sqlite3 /Users/cabbos/.cc-switch/cc-switch.db ".backup '$backup_dir/cc-switch.db'"
sqlite3 /Users/cabbos/.codex/state_5.sqlite ".backup '$backup_dir/state_5.sqlite'"
cp -p /Users/cabbos/.codex/config.toml "$backup_dir/config.toml"
cp -p /Users/cabbos/.codex/auth.json "$backup_dir/auth.json"
cp -p /Users/cabbos/.codex/models_cache.json "$backup_dir/models_cache.json"
tar -C /Users/cabbos/.codex -czf "$backup_dir/session-index-files.tar.gz" \
  sessions archived_sessions
chmod 600 "$backup_dir"/*
printf '%s\n' "$backup_dir" > /tmp/codex-unified-routing-backup-path
```

Expected: no secret is printed.

- [ ] **Step 3: Validate backup integrity**

```bash
backup_dir="$(sed -n '1p' /tmp/codex-unified-routing-backup-path)"
test "$(sqlite3 "$backup_dir/cc-switch.db" 'PRAGMA integrity_check;')" = "ok"
test "$(sqlite3 "$backup_dir/state_5.sqlite" 'PRAGMA integrity_check;')" = "ok"
tar -tzf "$backup_dir/session-index-files.tar.gz" >/dev/null
test "$(stat -f '%Lp' "$backup_dir")" = "700"
```

Expected: all checks exit `0`.

- [ ] **Step 4: Configure `Kimi Bridge` routing metadata without exposing keys**

Quit CCSwitch, assert the exact provider IDs exist, then update only `meta.codexModelRouting`:

```bash
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND id='a15a786a-9670-46f2-9081-208d74cb103f' AND name='Kimi Bridge';")" = "1"
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND id='codex-official' AND name='OpenAI Official';")" = "1"
sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "
BEGIN IMMEDIATE;
UPDATE providers
SET meta = json_set(
  COALESCE(NULLIF(meta, ''), '{}'),
  '$.codexModelRouting',
  json('{\"openaiProviderId\":\"codex-official\",\"exactRoutes\":{\"k3\":\"a15a786a-9670-46f2-9081-208d74cb103f\"}}')
)
WHERE app_type='codex'
  AND id='a15a786a-9670-46f2-9081-208d74cb103f';
SELECT changes();
COMMIT;
"
```

Expected: `changes()` prints exactly `1`.

- [ ] **Step 5: Enable unified history with migration through CCSwitch UI**

Open Settings → General → Codex enhancements. Enable **Unified Codex session history**, select **migrate existing official sessions**, and confirm.

Expected: CCSwitch creates its own official-history backup ledger before rewriting provider identity.

- [ ] **Step 6: Verify migration counts without reading content**

```bash
sqlite3 -header -column /Users/cabbos/.codex/state_5.sqlite "
SELECT model_provider, archived, COUNT(*) AS count
FROM threads
GROUP BY model_provider, archived
ORDER BY model_provider, archived;
"
```

Expected: all previously unarchived `openai` and `custom` threads are in the shared `custom` bucket; archived counts and total row count are unchanged.

### Task 11: Production End-to-End Verification and Rollback Gate

**Files:**
- Read: `/Users/cabbos/.codex/config.toml`
- Read: `/Users/cabbos/.codex/cc-switch-model-catalog.json`
- Read: `/Users/cabbos/.codex/state_5.sqlite`
- Rollback source: path recorded in `/tmp/codex-unified-routing-backup-path`

- [ ] **Step 1: Select Kimi Bridge and verify the merged catalog**

Restart CCSwitch, enable Codex routing takeover, and select `Kimi Bridge`. Then run:

```bash
rg -q '^model_provider = "custom"$' /Users/cabbos/.codex/config.toml
rg -q '127\.0\.0\.1:15721' /Users/cabbos/.codex/config.toml
jq -e '
  ([.models[].slug] | index("k3") != null) and
  ([.models[].slug] | index("gpt-5.5") != null) and
  ([.models[].slug] | length == (unique | length))
' /Users/cabbos/.codex/cc-switch-model-catalog.json >/dev/null
```

Expected: all assertions exit `0`.

- [ ] **Step 2: Run isolated GPT and K3 smoke tests**

```bash
test_dir="$(mktemp -d /tmp/codex-unified-routing.XXXXXX)"
cd "$test_dir"
/Applications/ChatGPT.app/Contents/Resources/codex exec \
  --skip-git-repo-check --sandbox read-only --color never \
  -m gpt-5.5 'Reply with exactly GPT_ROUTE_OK.'
/Applications/ChatGPT.app/Contents/Resources/codex exec \
  --skip-git-repo-check --sandbox read-only --color never \
  -m k3 'Reply with exactly K3_ROUTE_OK.'
```

Expected: both commands exit `0` and return their exact markers.

- [ ] **Step 3: Run a K3 tool-call test**

```bash
cd "$test_dir"
/Applications/ChatGPT.app/Contents/Resources/codex exec \
  --skip-git-repo-check --sandbox read-only --color never \
  -m k3 'Use the shell tool to run pwd once, then reply with exactly K3_TOOL_OK.'
```

Expected: one `pwd` call succeeds and the final output contains `K3_TOOL_OK`.

- [ ] **Step 4: Verify history visibility and route pins**

```bash
sqlite3 -header -column /Users/cabbos/.codex/state_5.sqlite "
SELECT model_provider, COUNT(*) AS count
FROM threads
WHERE archived=0
GROUP BY model_provider;
"
sqlite3 -header -column /Users/cabbos/.cc-switch/cc-switch.db "
SELECT provider_family, COUNT(*) AS count
FROM codex_thread_route_pins
GROUP BY provider_family
ORDER BY provider_family;
"
```

Expected: unarchived history is in one `custom` bucket; both `openai` and `kimi` route-pin families exist.

- [ ] **Step 5: Verify cross-family resume is blocked using a copied Codex home**

Copy one GPT thread and its state row into a private disposable Codex home, rewrite only the copied row's rollout path, and resume the copied thread with K3:

```bash
resume_home="$(mktemp -d /tmp/codex-cross-family-home.XXXXXX)"
chmod 700 "$resume_home"
thread_id="$(sqlite3 /Users/cabbos/.codex/state_5.sqlite "SELECT id FROM threads WHERE model_provider='custom' AND model GLOB 'gpt-*' AND archived=0 ORDER BY updated_at DESC LIMIT 1;")"
rollout_path="$(sqlite3 /Users/cabbos/.codex/state_5.sqlite "SELECT rollout_path FROM threads WHERE id='$thread_id';")"
test -n "$thread_id" && test -f "$rollout_path"
cp -p /Users/cabbos/.codex/config.toml "$resume_home/config.toml"
cp -p /Users/cabbos/.codex/auth.json "$resume_home/auth.json"
cp -p /Users/cabbos/.codex/models_cache.json "$resume_home/models_cache.json"
sqlite3 /Users/cabbos/.codex/state_5.sqlite ".backup '$resume_home/state_5.sqlite'"
find "$resume_home" -type f -exec chmod 600 {} +
mkdir -m 700 "$resume_home/sessions"
copied_rollout="$resume_home/sessions/$(basename "$rollout_path")"
cp -p "$rollout_path" "$copied_rollout"
sqlite3 "$resume_home/state_5.sqlite" "DELETE FROM threads WHERE id <> '$thread_id'; UPDATE threads SET rollout_path='$copied_rollout' WHERE id='$thread_id';"
set +e
CODEX_HOME="$resume_home" /Applications/ChatGPT.app/Contents/Resources/codex exec resume \
  --skip-git-repo-check --sandbox read-only --json -m k3 \
  "$thread_id" 'This request must be rejected before forwarding.' \
  >"$resume_home/resume.out" 2>"$resume_home/resume.err"
resume_status=$?
set -e
test "$resume_status" -ne 0
rg -q 'cc_switch_cross_provider_resume_blocked' \
  "$resume_home/resume.out" "$resume_home/resume.err"
```

Expected: the command fails with the guard code, and neither the production state database nor production rollout file is opened for writing.

- [ ] **Step 6: Run the final secret scan**

Feed the stored Kimi token to ripgrep over stdin so it never appears in the command line. Search only files that must not contain the token; exclude the credential database and encrypted backups by construction:

```bash
kimi_token="$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT json_extract(settings_config,'$.auth.OPENAI_API_KEY') FROM providers WHERE app_type='codex' AND id='a15a786a-9670-46f2-9081-208d74cb103f';")"
test -n "$kimi_token"
scan_targets=(
  /Users/cabbos/.codex/config.toml
  /Users/cabbos/.codex/cc-switch-model-catalog.json
  /Users/cabbos/.cc-switch/logs
  /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing/src
  /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing/src-tauri
  /Users/cabbos/project/forge/docs/superpowers/specs/2026-07-20-codex-gpt-k3-unified-routing-history-design.md
  /Users/cabbos/project/forge/docs/superpowers/plans/2026-07-20-codex-gpt-k3-unified-routing-history.md
  "$test_dir"
  "$resume_home"
)
leaks="$(printf '%s\n' "$kimi_token" | rg -l -F -f - "${scan_targets[@]}" 2>/dev/null || true)"
unset kimi_token
if test -n "$leaks"; then
  printf 'secret leak files:\n%s\n' "$leaks"
  exit 1
fi
printf '%s\n' 'secret scan: PASS'
```

Expected: only `secret scan: PASS` is printed.

- [ ] **Step 7: Hand off one desktop restart**

Ask the user to quit and reopen ChatGPT/Codex. Verify in a new task that the GUI model picker contains GPT entries and `Kimi K3`, while the existing task list remains visible.

- [ ] **Step 8: Roll back on any required-check failure**

Quit both CCSwitch builds, then restore:

```bash
backup_dir="$(sed -n '1p' /tmp/codex-unified-routing-backup-path)"
cp -p "$backup_dir/cc-switch.db" /Users/cabbos/.cc-switch/cc-switch.db
cp -p "$backup_dir/state_5.sqlite" /Users/cabbos/.codex/state_5.sqlite
cp -p "$backup_dir/config.toml" /Users/cabbos/.codex/config.toml
cp -p "$backup_dir/auth.json" /Users/cabbos/.codex/auth.json
cp -p "$backup_dir/models_cache.json" /Users/cabbos/.codex/models_cache.json
tar -C /Users/cabbos/.codex -xzf "$backup_dir/session-index-files.tar.gz"
```

Restore the original `/Applications/CC Switch.app`, restart it, and verify OpenAI `default` history visibility. Report rollback instead of claiming unified routing works.

### Task 12: Final Source Review and Branch Handoff

**Files:**
- Review all committed CCSwitch feature files
- Preserve Forge's unrelated dirty files

- [ ] **Step 1: Run the complete CCSwitch verification suite**

```bash
cd /Users/cabbos/project/cc-switch/.worktrees/codex-unified-model-routing
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
corepack pnpm typecheck
corepack pnpm test:unit
corepack pnpm format:check
corepack pnpm build:renderer
```

Expected: all commands exit `0`.

- [ ] **Step 2: Inspect the final diff for scope and secret safety**

```bash
git status --short
git diff 613fef70bc7d5e35299b4131935f738c85765b35...HEAD --stat
git diff 613fef70bc7d5e35299b4131935f738c85765b35...HEAD --check
rg -n 'sk-kimi-|oauth-test-token|kimi-test-key' src src-tauri \
  --glob '!**/*test*' --glob '!**/tests/**'
```

Expected: the first three commands show only intended feature scope and no whitespace errors; the final search returns no production-code credential literals.

- [ ] **Step 3: Use the finishing-a-development-branch workflow**

Present merge, pull-request, keep, and discard options for branch `codex/codex-unified-model-routing`. Do not replace the production application, push, or open a pull request without the corresponding user choice.
