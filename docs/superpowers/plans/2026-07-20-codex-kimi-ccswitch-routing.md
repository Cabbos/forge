# Codex–Kimi CCSwitch Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the existing Kimi Code credential to CCSwitch's Codex providers, enable CCSwitch's built-in Responses-to-Chat-Completions routing at login, and verify safe one-click switching between Kimi and OpenAI.

**Architecture:** CCSwitch 3.17.0 remains the single provider manager and protocol gateway. Codex sends Responses API traffic to CCSwitch on `127.0.0.1:15721`; CCSwitch converts it to Kimi Code's Chat Completions endpoint and converts streaming/tool-call results back. The implementation uses CCSwitch's built-in `Kimi For Coding` preset and never installs an external bridge.

**Tech Stack:** CCSwitch 3.17.0, Codex CLI/Desktop, SQLite, macOS Login Items, CCSwitch local routing, Kimi Code OpenAI-compatible API

---

## File and State Map

- Modify through CCSwitch: `/Users/cabbos/.cc-switch/cc-switch.db` — provider records and local-routing state.
- Modify through CCSwitch: `/Users/cabbos/.codex/config.toml` — active Codex provider projection.
- Potentially modify through CCSwitch: `/Users/cabbos/.codex/auth.json` — active local-routing credential projection; preserve the official login according to CCSwitch 3.17 behavior.
- Create: `/Users/cabbos/.cc-switch/backups/codex-kimi-routing-YYYYMMDD-HHMMSS/` — private pre-change database/config/auth backups.
- Modify: macOS per-user Login Items — open `/Applications/CC Switch.app` at login.
- No Forge application source files change during implementation.

### Task 1: Preflight and Recoverable Backups

**Files:**
- Read: `/Applications/CC Switch.app/Contents/Info.plist`
- Read: `/Users/cabbos/.cc-switch/cc-switch.db`
- Create: `/Users/cabbos/.cc-switch/backups/codex-kimi-routing-YYYYMMDD-HHMMSS/cc-switch.db`
- Create: `/Users/cabbos/.cc-switch/backups/codex-kimi-routing-YYYYMMDD-HHMMSS/config.toml`
- Create: `/Users/cabbos/.cc-switch/backups/codex-kimi-routing-YYYYMMDD-HHMMSS/auth.json`

- [ ] **Step 1: Verify the installed CCSwitch version and required files**

Run:

```bash
defaults read '/Applications/CC Switch.app/Contents/Info.plist' CFBundleShortVersionString
test -f /Users/cabbos/.cc-switch/cc-switch.db
test -f /Users/cabbos/.codex/config.toml
test -f /Users/cabbos/.codex/auth.json
```

Expected: version is `3.17.0`; all three file checks exit `0`.

- [ ] **Step 2: Verify exactly one Claude-side Kimi provider and a non-empty Kimi token without printing it**

Run:

```bash
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='claude' AND name='Kimi';")" = "1"
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT length(COALESCE(json_extract(settings_config, '$.env.ANTHROPIC_AUTH_TOKEN'), '')) FROM providers WHERE app_type='claude' AND name='Kimi';")" -gt 20
```

Expected: both checks exit `0`; no credential is printed.

- [ ] **Step 3: Confirm the source provider points at Kimi Code rather than the unrelated GLM fields**

Run:

```bash
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT json_extract(settings_config, '$.env.ANTHROPIC_BASE_URL') FROM providers WHERE app_type='claude' AND name='Kimi';")" = "https://api.kimi.com/coding"
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT json_extract(settings_config, '$.env.ANTHROPIC_DEFAULT_SONNET_MODEL') FROM providers WHERE app_type='claude' AND name='Kimi';")" = "kimi-for-coding"
```

Expected: both checks exit `0`.

- [ ] **Step 4: Create a private backup directory and consistent backups**

Run:

```bash
kimi_backup_dir="/Users/cabbos/.cc-switch/backups/codex-kimi-routing-$(date +%Y%m%d-%H%M%S)"
mkdir -m 700 "$kimi_backup_dir"
sqlite3 /Users/cabbos/.cc-switch/cc-switch.db ".backup '$kimi_backup_dir/cc-switch.db'"
cp -p /Users/cabbos/.codex/config.toml "$kimi_backup_dir/config.toml"
cp -p /Users/cabbos/.codex/auth.json "$kimi_backup_dir/auth.json"
chmod 600 "$kimi_backup_dir/cc-switch.db" "$kimi_backup_dir/config.toml" "$kimi_backup_dir/auth.json"
printf '%s\n' "$kimi_backup_dir" > /tmp/codex-kimi-routing-backup-path
```

Expected: the command prints no secret and `/tmp/codex-kimi-routing-backup-path` contains the exact rollback directory.

- [ ] **Step 5: Validate the backups**

Run:

```bash
kimi_backup_dir="$(sed -n '1p' /tmp/codex-kimi-routing-backup-path)"
sqlite3 "$kimi_backup_dir/cc-switch.db" 'PRAGMA integrity_check;'
test -s "$kimi_backup_dir/config.toml"
test -s "$kimi_backup_dir/auth.json"
stat -f '%Lp %N' "$kimi_backup_dir" "$kimi_backup_dir"/*
```

Expected: SQLite reports `ok`; the directory mode is `700` and backup files are `600`.

### Task 2: Create the Codex Kimi Provider Through CCSwitch

**Files:**
- Modify through UI: `/Users/cabbos/.cc-switch/cc-switch.db`

- [ ] **Step 1: Open CCSwitch and select the Codex provider panel**

Open `/Applications/CC Switch.app`, select **Codex**, and choose **Add Provider**. Use the built-in **Kimi For Coding** preset, not the generic Kimi Platform preset.

Expected preset values:

```text
Name: Kimi Bridge
Base URL: https://api.kimi.com/coding/v1
API format: OpenAI Chat Completions (Requires routing)
Default model: kimi-for-coding
Model mapping: kimi-for-coding / Kimi For Coding / 262144
Prompt-cache routing: enabled
Custom User-Agent: empty
```

- [ ] **Step 2: Put the existing Kimi token on the clipboard without displaying it**

Run immediately before focusing the API Key field:

```bash
sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT json_extract(settings_config, '$.env.ANTHROPIC_AUTH_TOKEN') FROM providers WHERE app_type='claude' AND name='Kimi';" | tr -d '\n' | pbcopy
```

Expected: clipboard contains the Kimi token; terminal output is empty.

- [ ] **Step 3: Paste the token, preserve honest client identity, and save**

Paste into **API Key**. Confirm **Custom User-Agent** remains empty, **Thinking** is enabled, **Effort levels** are disabled, and the reasoning output field remains `reasoning_content`. Save the provider.

Expected: CCSwitch shows `Kimi Bridge` in the Codex provider list and marks it as requiring local routing.

- [ ] **Step 4: Clear the system clipboard**

Run:

```bash
printf '' | pbcopy
```

Expected: `pbpaste | wc -c` returns `0`.

- [ ] **Step 5: Validate the stored provider without reading its token**

Run:

```bash
sqlite3 -header -column /Users/cabbos/.cc-switch/cc-switch.db "
SELECT name,
       app_type,
       category,
       json_extract(meta, '$.apiFormat') AS api_format,
       json_extract(settings_config, '$.modelCatalog.models[0].model') AS model,
       json_extract(settings_config, '$.modelCatalog.models[0].contextWindow') AS context_window,
       length(COALESCE(json_extract(settings_config, '$.auth.OPENAI_API_KEY'), '')) > 20 AS has_key
FROM providers
WHERE app_type='codex' AND name='Kimi Bridge';
"
```

Expected: one row with `openai_chat`, `kimi-for-coding`, `262144`, and `has_key=1`.

### Task 3: Enable Automatic CCSwitch Startup and Local Codex Routing

**Files:**
- Modify: macOS per-user Login Items
- Modify through CCSwitch: `/Users/cabbos/.cc-switch/cc-switch.db`
- Modify through CCSwitch: `/Users/cabbos/.codex/config.toml`
- Potentially modify through CCSwitch: `/Users/cabbos/.codex/auth.json`

- [ ] **Step 1: Confirm port 15721 is free or already owned by CCSwitch**

Run:

```bash
listener="$(lsof -nP -iTCP:15721 -sTCP:LISTEN -Fpct 2>/dev/null || true)"
if [ -n "$listener" ]; then
  printf '%s\n' "$listener" | rg -i 'CC Switch|cc-switch' >/dev/null
fi
```

Expected: no listener, or the listener belongs to CCSwitch. Stop if another process owns the port.

- [ ] **Step 2: Record whether a CCSwitch login item already exists**

Run:

```bash
osascript -e 'tell application "System Events" to count every login item whose name is "CC Switch"' | awk '{print ($1 > 0 ? 1 : 0)}' > /tmp/codex-kimi-login-item-preexisting
```

Expected: the file contains exactly `0` or `1`.

- [ ] **Step 3: Register one per-user login item**

Run:

```bash
osascript -e 'tell application "System Events" to if not (exists login item "CC Switch") then make login item at end with properties {name:"CC Switch", path:"/Applications/CC Switch.app", hidden:true}'
```

Expected: the command exits `0`.

- [ ] **Step 4: Verify the login item points at the expected application**

Run:

```bash
osascript -e 'tell application "System Events" to get {name, path, hidden} of every login item whose name is "CC Switch"'
```

Expected: exactly one `CC Switch` item points to `/Applications/CC Switch.app` and is hidden.

- [ ] **Step 5: Start CCSwitch local routing and enable Codex takeover**

In CCSwitch, open **Local Routing** settings, start the routing service, keep the listen address `127.0.0.1` and port `15721`, disable request-body logging, and enable routing/takeover for **Codex**.

Expected: CCSwitch reports local routing active for Codex.

- [ ] **Step 6: Verify the loopback listener**

Run:

```bash
lsof -nP -iTCP:15721 -sTCP:LISTEN
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT listen_address FROM proxy_config WHERE app_type='codex';")" = "127.0.0.1"
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT proxy_enabled FROM proxy_config WHERE app_type='codex';")" = "1"
```

Expected: CCSwitch owns `127.0.0.1:15721`; both database assertions exit `0`.

### Task 4: Switch to Kimi and Validate the Live Codex Projection

**Files:**
- Modify through CCSwitch: `/Users/cabbos/.codex/config.toml`
- Potentially modify through CCSwitch: `/Users/cabbos/.codex/auth.json`

- [ ] **Step 1: Select Kimi Bridge in the CCSwitch Codex panel**

Click `Kimi Bridge` and wait for the provider card and local-routing status to become active.

Expected: `Kimi Bridge` is the current Codex provider.

- [ ] **Step 2: Validate provider selection and live routing without printing credentials**

Run:

```bash
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND name='Kimi Bridge' AND is_current=1;")" = "1"
rg -q '^model = "kimi-for-coding"$' /Users/cabbos/.codex/config.toml
rg -q '^model_provider = "custom"$' /Users/cabbos/.codex/config.toml
rg -q '127\.0\.0\.1:15721' /Users/cabbos/.codex/config.toml
```

Expected: all checks exit `0`.

- [ ] **Step 3: Confirm model catalog and context size**

Run:

```bash
catalog_path="$(sed -n 's/^model_catalog_json = "\(.*\)"$/\1/p' /Users/cabbos/.codex/config.toml | head -n 1)"
test -n "$catalog_path"
jq -e '.models[] | select(.slug == "kimi-for-coding" and .context_window == 262144)' "$catalog_path" >/dev/null
```

Expected: `jq` exits `0` without printing catalog contents.

### Task 5: Run End-to-End Kimi Tests Without Identity Spoofing

**Files:**
- Read: `/Users/cabbos/.codex/config.toml`
- Read: CCSwitch operational logs only when diagnosing a failure

- [ ] **Step 1: Run a streaming text smoke test**

Run from an isolated temporary directory:

```bash
kimi_test_dir="$(mktemp -d /tmp/codex-kimi-test.XXXXXX)"
cd "$kimi_test_dir"
/Applications/ChatGPT.app/Contents/Resources/codex exec --skip-git-repo-check --sandbox read-only --color never 'Reply with exactly KIMI_ROUTE_OK.'
```

Expected: command exits `0` and final output contains `KIMI_ROUTE_OK`.

- [ ] **Step 2: Run a read-only tool-call round trip**

Run:

```bash
cd "$kimi_test_dir"
/Applications/ChatGPT.app/Contents/Resources/codex exec --skip-git-repo-check --sandbox read-only --color never 'Use the shell tool to run pwd once, then reply with exactly TOOL_ROUTE_OK.'
```

Expected: command exits `0`, performs one `pwd` tool call, and final output contains `TOOL_ROUTE_OK`.

- [ ] **Step 3: Enforce the honest-identity boundary**

If either test returns an upstream `403` or a message about an unsupported/blocked User-Agent, do not set CCSwitch's Custom User-Agent field and do not impersonate Claude Code. Immediately execute Task 7 rollback and report that Kimi currently rejects Codex as a client.

Expected when supported: no User-Agent rejection and no custom User-Agent stored in provider metadata.

- [ ] **Step 4: Verify the provider has no custom User-Agent override**

Run:

```bash
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND name='Kimi Bridge' AND COALESCE(json_extract(meta, '$.customUserAgent'), '') <> '';")" = "0"
```

Expected: check exits `0`.

### Task 6: Verify One-Click OpenAI Fallback and Restore Kimi as the Selected Provider

**Files:**
- Modify through CCSwitch: `/Users/cabbos/.codex/config.toml`
- Potentially modify through CCSwitch: `/Users/cabbos/.codex/auth.json`

- [ ] **Step 1: Switch to OpenAI Official through CCSwitch**

Select `OpenAI Official` in the Codex panel. If CCSwitch requires disabling Codex takeover before switching to the official provider, disable takeover, switch, and leave the saved Kimi provider intact.

Expected: OpenAI Official becomes current and `Kimi Bridge` remains saved.

- [ ] **Step 2: Verify the official provider is usable**

Run:

```bash
/Applications/ChatGPT.app/Contents/Resources/codex login status
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND name='OpenAI Official' AND is_current=1;")" = "1"
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND name='Kimi Bridge';")" = "1"
```

Expected: login status identifies the official authentication method; both database assertions exit `0`.

- [ ] **Step 3: Switch back to Kimi Bridge as the requested final state**

Re-enable Codex local routing/takeover if necessary, select `Kimi Bridge`, and rerun the three live projection assertions from Task 4 Step 2.

Expected: Kimi Bridge is current, OpenAI Official remains saved, and the loopback route is active.

### Task 7: Final Verification, Secret Scan, and Rollback Procedure

**Files:**
- Read: `/Users/cabbos/.cc-switch/cc-switch.db`
- Read: `/Users/cabbos/.codex/config.toml`
- Read: `/Users/cabbos/.codex/auth.json`
- Rollback source: directory recorded in `/tmp/codex-kimi-routing-backup-path`

- [ ] **Step 1: Check that no unapproved file contains the upstream Kimi token**

Run this in-memory scan. It reads the source key directly from SQLite, skips private backups, and never prints the key or its hash:

```bash
python3 - <<'PY'
import json
import sqlite3
from pathlib import Path

db_path = Path('/Users/cabbos/.cc-switch/cc-switch.db')
with sqlite3.connect(db_path) as conn:
    row = conn.execute(
        "SELECT settings_config FROM providers WHERE app_type='claude' AND name='Kimi'"
    ).fetchone()
if row is None:
    raise SystemExit('source Kimi provider missing')
token = json.loads(row[0]).get('env', {}).get('ANTHROPIC_AUTH_TOKEN', '')
if len(token) <= 20:
    raise SystemExit('source Kimi token missing')

candidates = [
    Path('/Users/cabbos/.codex/config.toml'),
    Path('/Users/cabbos/.codex/auth.json'),
    Path('/tmp/codex-kimi-routing-backup-path'),
    Path('/Users/cabbos/project/forge/docs/superpowers/specs/2026-07-20-codex-kimi-bridge-design.md'),
    Path('/Users/cabbos/project/forge/docs/superpowers/plans/2026-07-20-codex-kimi-ccswitch-routing.md'),
]
log_root = Path('/Users/cabbos/.cc-switch/logs')
if log_root.exists():
    candidates.extend(path for path in log_root.rglob('*') if path.is_file())

leaks = []
for path in candidates:
    try:
        if token.encode() in path.read_bytes():
            leaks.append(str(path))
    except (FileNotFoundError, PermissionError, IsADirectoryError):
        continue
if leaks:
    raise SystemExit('Kimi token found in unapproved file(s): ' + ', '.join(leaks))
print('secret scan: ok')
PY
```

Expected: `secret scan: ok`. The only new credential copy is the CCSwitch-managed Codex provider record inside `cc-switch.db`; no scripts, logs, Codex live files, or Forge files contain it.

- [ ] **Step 2: Confirm final state**

Run:

```bash
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND name='Kimi Bridge' AND is_current=1;")" = "1"
test "$(sqlite3 /Users/cabbos/.cc-switch/cc-switch.db "SELECT count(*) FROM providers WHERE app_type='codex' AND name='OpenAI Official';")" = "1"
lsof -nP -iTCP:15721 -sTCP:LISTEN | rg '127\.0\.0\.1:15721'
```

Expected: all checks exit `0`.

- [ ] **Step 3: Roll back only if an earlier required check fails**

First quit CCSwitch so it cannot overwrite restored files, then run:

```bash
kimi_backup_dir="$(sed -n '1p' /tmp/codex-kimi-routing-backup-path)"
cp -p "$kimi_backup_dir/cc-switch.db" /Users/cabbos/.cc-switch/cc-switch.db
cp -p "$kimi_backup_dir/config.toml" /Users/cabbos/.codex/config.toml
cp -p "$kimi_backup_dir/auth.json" /Users/cabbos/.codex/auth.json
if [ "$(sed -n '1p' /tmp/codex-kimi-login-item-preexisting)" = "0" ]; then
  osascript -e 'tell application "System Events" to delete every login item whose name is "CC Switch"'
fi
open '/Applications/CC Switch.app'
```

Expected: pre-change provider state and Codex configuration are restored. Report that rollback occurred and do not claim Kimi routing works.

- [ ] **Step 4: Hand off the desktop restart**

After successful verification, tell the user to quit and reopen the ChatGPT/Codex desktop application once. The active conversation must not be terminated during implementation merely to force this restart.

Expected: new Codex tasks load the Kimi model catalog and provider selection after restart.
