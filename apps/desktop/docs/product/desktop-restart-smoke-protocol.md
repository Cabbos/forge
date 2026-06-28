# Desktop Restart Smoke Protocol

## Scope

This is a manual or semi-automated macOS smoke until Forge has a dedicated Tauri WebDriver harness. Run it against the desktop app and a disposable current project so restart recovery is checked against the real Tauri runtime, IndexedDB persistence, and live permission gate state rather than only browser-level mocks.

## Automation Preflight

Before claiming a true desktop restart harness exists, run:

```bash
node scripts/desktop-restart-harness-preflight.mjs --json
```

On current macOS runs this is expected to report `blocked_official_macos`, including the `official macOS WKWebView WebDriver support` gap, and keep `npm --prefix apps/desktop run test:e2e -- e2e/level3-runtime-restart.spec.ts` as the partial mocked fallback. Use `--require-harness` only on a platform/runner that is expected to provide the official Tauri/WebDriver pieces.

The acceptance matrix also runs `node --test scripts/desktop-restart-harness-preflight.test.mjs` and a blocker-documentation status gate that checks this protocol plus the beta log, so the macOS blocker remains explicit even if local `tauri-driver` and WebDriver client dependencies are later added.

## Required Evidence

1. Current project path before quit.
2. Permission mode before quit.
3. Whether a pending confirmation exists before quit.
4. Session id before quit.
5. Screenshot or log after restart showing restored session.
6. Whether Composer permission mode, Project Status mode, pending confirmation card, health alerts, and context usage agree after restart.

## Steps

1. Start Forge with `npm --prefix apps/desktop run tauri -- dev`.
2. Select `/Users/cabbos/project/forge-test-app` or a disposable test project.
3. Start a new conversation and enable `信任项目` or `完全访问`.
4. Send a prompt that causes a current-project write confirmation.
5. Quit the app with an active session or pending confirmation.
6. Restart Forge.
7. Record restored session status, permission mode, pending confirmation card, project status, and context usage.

## Pass Criteria

- Restored UI never claims a broader permission mode than the live session gate can honor.
- Pending confirmation is replayed as interrupted, resolved, or pending with a clear explanation.
- Stale health alerts from the old run are cleared or scoped to the restored active session.
- Context usage is either restored from provider usage or explicitly unknown.
