# Root Desktop Start Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `npm run dev:desktop` start the full Forge Tauri app from the monorepo root while retaining a frontend-only command.

**Architecture:** Keep root scripts as thin npm delegations into `apps/desktop`. Lock the command mapping with a small Node contract test and document the root-level entry point in the repository README.

**Tech Stack:** npm scripts, Node.js built-in test runner, Markdown

---

### Task 1: Lock the root command contract

**Files:**
- Create: `scripts/root-startup-command.test.mjs`
- Modify: `package.json:5-8`

- [ ] **Step 1: Write the failing test**

```js
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("root desktop commands distinguish full app and frontend-only startup", async () => {
  const packageJson = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );

  assert.equal(
    packageJson.scripts["dev:desktop"],
    "npm --prefix apps/desktop run tauri dev",
  );
  assert.equal(
    packageJson.scripts["dev:desktop:web"],
    "npm --prefix apps/desktop run dev",
  );
});
```

- [ ] **Step 2: Run the test and verify the expected failure**

Run: `node --test scripts/root-startup-command.test.mjs`

Expected: FAIL because `dev:desktop` still maps to the frontend-only Vite command.

- [ ] **Step 3: Update the root scripts**

Set the root script mappings to:

```json
"dev:desktop": "npm --prefix apps/desktop run tauri dev",
"dev:desktop:web": "npm --prefix apps/desktop run dev"
```

- [ ] **Step 4: Run the contract test again**

Run: `node --test scripts/root-startup-command.test.mjs`

Expected: PASS with one passing test and zero failures.

### Task 2: Document and verify the startup path

**Files:**
- Modify: `README.md` in the `开发` section

- [ ] **Step 1: Document both root commands**

Add this block before the build commands:

````markdown
从仓库根目录启动完整桌面应用：

```bash
npm run dev:desktop
```

只启动桌面前端 Vite 服务：

```bash
npm run dev:desktop:web
```
````

- [ ] **Step 2: Run focused static verification**

Run: `node --test scripts/root-startup-command.test.mjs`

Expected: PASS with one passing test and zero failures.

- [ ] **Step 3: Launch the full app from the root**

Run: `npm run dev:desktop`

Expected: output contains `VITE ... ready`, Cargo completes, and `target/debug/forge` runs. Stop the development process after capturing this evidence.

- [ ] **Step 4: Check the affected scope**

Run: `node scripts/gitnexus-safe.mjs -- detect-changes --scope compare --base-ref main`

Expected: either a GitNexus report limited to the startup script/docs/test or the required fallback impact template if the local GitNexus CLI remains unavailable.
