import { test, expect, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

/** Setup: inject mock IPC before the app loads */
async function setup(page: Page, options?: { workingDir?: string | null }) {
  const initialWorkingDir = options && "workingDir" in options ? options.workingDir : "/Users/cabbos/project/forge";
  await page.addInitScript(({ initialWorkingDir }) => {
    let callbackId = 0;
    const callbacks = new Map<number, (data: unknown) => void>();
    const workingDir = initialWorkingDir ?? "/Users/cabbos/project/forge";
    const sessionWorkingDirs = new Map<string, string>();
    if (initialWorkingDir === null) {
      window.localStorage.removeItem("forge-working-dir");
    } else {
      window.localStorage.setItem("forge-working-dir", workingDir);
    }
    const projectRuntimeStatus = {
      working_dir: workingDir,
      has_package_json: true,
      package_manager: "npm",
      dev_script: "dev",
      command: "npm run dev",
      port: 1420,
      url: "http://localhost:1420",
      running: false,
      managed: false,
      pid: null,
      can_start: true,
      can_stop: false,
      can_open: true,
      message: "Preview not running",
      logs: [],
    };
    const projectCheckpointStatus = {
      working_dir: workingDir,
      is_git_repo: true,
      dirty: false,
      last_checkpoint: null,
      message: "No checkpoint yet",
    };
    const mcpContextSources = {
      resources: [
        {
          server_id: "obsidian",
          uri: "file:///notes/forge.md",
          name: "Forge 研发记录",
          description: "Obsidian 中的项目研发记录。",
          mime_type: "text/markdown",
        },
      ],
      prompts: [
        {
          server_id: "linear",
          name: "summarize_issue",
          description: "整理当前任务风险。",
          arguments: [{ name: "focus", description: "关注点", required: false }],
        },
      ],
    };
    let forgeWikiExists = false;
    const forgeWikiPages = [
      {
        id: "index",
        project_path: workingDir,
        path: "index.md",
        title: "项目概览",
        kind: "index",
        summary: "项目目标、边界和当前结构。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
      {
        id: "tasks",
        project_path: workingDir,
        path: "tasks.md",
        title: "当前任务",
        kind: "tasks",
        summary: "当前任务、验收步骤和后续事项。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
      {
        id: "decisions",
        project_path: workingDir,
        path: "decisions.md",
        title: "决策记录",
        kind: "decisions",
        summary: "重要方案和取舍。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
    ];
    const forgeWikiProposals = new Map<string, Record<string, unknown>>();
    const forgeWikiState = (projectPath: string, exists: boolean) => ({
      project_path: projectPath,
      exists,
      wiki_dir: `${projectPath}/.forge/wiki`,
      pages: exists ? forgeWikiPages.map((page) => ({ ...page, project_path: projectPath })) : [],
      message: exists ? "项目记录已就绪。" : "还没有项目记录",
    });
    const forgeWikiProposal = (projectPath: string, args: Record<string, unknown>) => ({
      id: String(args.proposalId ?? args.id ?? "forge-wiki-proposal"),
      project_path: projectPath,
      session_id: typeof args.sessionId === "string" ? args.sessionId : null,
      target_pages: Array.isArray(args.targetPages) ? args.targetPages.map(String) : ["tasks.md"],
      title: String(args.title ?? "记录项目进展"),
      summary: String(args.summary ?? "补充本轮任务产生的项目记录。"),
      patch_preview: typeof args.patchPreview === "string" ? args.patchPreview : null,
      status: "pending",
      created_at: "2026-05-13T00:00:00.000Z",
    });
    const openKeyvalDb = async () => {
      let db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains("keyval")) database.createObjectStore("keyval");
        };
      });
      if (db.objectStoreNames.contains("keyval")) return db;

      const nextVersion = db.version + 1;
      db.close();
      db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store", nextVersion);
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains("keyval")) database.createObjectStore("keyval");
        };
      });
      return db;
    };
    const readKeyval = async <T,>(key: string): Promise<T | null> => {
      try {
        const db = await openKeyvalDb();
        const value = await new Promise<T | null>((resolve, reject) => {
          const tx = db.transaction("keyval", "readonly");
          const request = tx.objectStore("keyval").get(key);
          request.onerror = () => reject(request.error);
          request.onsuccess = () => resolve((request.result ?? null) as T | null);
        });
        db.close();
        return value;
      } catch {
        return null;
      }
    };
    const writeKeyval = async (key: string, value: unknown) => {
      try {
        const db = await openKeyvalDb();
        await new Promise<void>((resolve, reject) => {
          const tx = db.transaction("keyval", "readwrite");
          tx.objectStore("keyval").put(value, key);
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      } catch {
        // Tests that do not need persistence should not fail setup because of IndexedDB.
      }
    };
    const deleteKeyval = async (key: string) => {
      try {
        const db = await openKeyvalDb();
        await new Promise<void>((resolve, reject) => {
          const tx = db.transaction("keyval", "readwrite");
          tx.objectStore("keyval").delete(key);
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      } catch {
        // Tests that do not need persistence should not fail setup because of IndexedDB.
      }
    };
    const saveAppMetadataToIndexedDb = async (metadata: Record<string, unknown>) => {
      await writeKeyval("forge-workspaces", Array.isArray(metadata.workspaces) ? metadata.workspaces : []);
      if (typeof metadata.activeWorkspaceId === "string") await writeKeyval("forge-active-workspace", metadata.activeWorkspaceId);
      else await deleteKeyval("forge-active-workspace");
      if (typeof metadata.activeSessionId === "string") await writeKeyval("forge-active-session", metadata.activeSessionId);
      else await deleteKeyval("forge-active-session");
      if (typeof metadata.selectedProvider === "string") await writeKeyval("forge-provider", metadata.selectedProvider);
      if (typeof metadata.selectedModel === "string") await writeKeyval("forge-model", metadata.selectedModel);
    };
    const persistedSessionsForBackend = async () => {
      const sessions = await readKeyval<Array<Record<string, unknown>>>("forge-sessions");
      return (sessions ?? []).map((session) => {
        const createdAt = typeof session.createdAt === "number" ? session.createdAt : Date.now();
        const updatedAt = typeof session.updatedAt === "number" ? session.updatedAt : createdAt;
        return {
          id: String(session.id ?? crypto.randomUUID()),
          provider: String(session.agentType ?? session.provider ?? "deepseek"),
          model: String(session.model ?? "deepseek-v4-flash[1m]"),
          status: String(session.status ?? "stopped"),
          created_at: new Date(createdAt).toISOString(),
          working_dir: typeof session.workingDir === "string" ? session.workingDir : null,
          created_at_ms: createdAt,
          updated_at_ms: updatedAt,
          context_window_tokens: typeof session.contextWindowTokens === "number" ? session.contextWindowTokens : null,
          latest_workflow: session.workflowState ?? null,
          latest_delivery: session.deliverySummary ?? null,
        };
      });
    };
    const appMetadataFromIndexedDb = async () => ({
      workspaces: await readKeyval("forge-workspaces") ?? [],
      activeWorkspaceId: await readKeyval("forge-active-workspace"),
      activeSessionId: await readKeyval("forge-active-session"),
      selectedProvider: await readKeyval("forge-provider"),
      selectedModel: await readKeyval("forge-model"),
    });
    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
      const projectPath = String(args.projectPath ?? workingDir);
      switch (cmd) {
        case "create_session":
          {
            // @ts-expect-error mock
            if (window.__mockCreateSessionError) throw new Error(String(window.__mockCreateSessionError));
            // @ts-expect-error mock
            const sessionId = window.__mockSessionId ?? crypto.randomUUID();
            sessionWorkingDirs.set(sessionId, String(args.workingDir ?? workingDir));
            // @ts-expect-error mock
            window.__lastCreateSessionArgs = args;
            // @ts-expect-error mock
            return {
              session_id: sessionId,
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              // @ts-expect-error mock
              missing_api_key: Boolean(window.__mockMissingApiKey),
            };
          }
        case "resume_session":
          {
            const sessionId = String(args.sessionId ?? "");
            // @ts-expect-error mock
            window.__lastResumedSessionId = sessionId;
            // @ts-expect-error mock
            const deliverySummary = window.__mockResumeDeliverySummary;
            if (deliverySummary) {
              window.setTimeout(() => {
                // @ts-expect-error listeners
                for (const listener of window.__tauriListeners?.["session-output"] ?? []) {
                  listener({
                    payload: {
                      event_type: "delivery_summary",
                      session_id: sessionId,
                      block_id: "resume-delivery-summary",
                      summary: deliverySummary,
                    },
                  });
                }
                // @ts-expect-error mock
                window.__resumeDeliveryEmitted = true;
              }, 0);
            }
            return {
              session_id: sessionId,
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              missing_api_key: false,
            };
          }
        case "send_input":
          // @ts-expect-error mock
          window.__lastSendInputArgs = args;
          return undefined;
        case "kill_session":
          // @ts-expect-error mock
          window.__lastKilledSessionId = args.sessionId;
          return undefined;
        case "delete_session":
          // @ts-expect-error mock
          window.__lastDeletedSessionId = args.sessionId;
          return undefined;
        case "confirm_response":
        case "set_api_key":
          return undefined;
        case "list_sessions":
          // @ts-expect-error mock
          if (Array.isArray(window.__mockListSessions)) return window.__mockListSessions;
          return persistedSessionsForBackend();
        case "load_app_metadata":
          return appMetadataFromIndexedDb();
        case "save_app_metadata":
          await saveAppMetadataToIndexedDb(args.metadata as Record<string, unknown>);
          return undefined;
        case "load_session_transcript":
          return [];
        case "get_default_working_dir":
          return workingDir;
        case "list_capabilities":
          return [
            { id: "read_file", name: "File Reader", description: "Read files", kind: "tool", source: "builtin", version: "1.0", enabled: true },
            { id: "code-review", name: "Code Review", description: "Review code", kind: "skill", source: "github", version: "1.2", enabled: true },
          ];
        case "search_workspace_files":
          {
            // @ts-expect-error mock
            window.__lastSearchWorkspaceFilesArgs = args;
            const sessionWorkspace = sessionWorkingDirs.get(String(args.sessionId ?? ""));
            const searchWorkspace = String(args.workingDir ?? sessionWorkspace ?? workingDir);
            const files = searchWorkspace.includes("forge-test-app")
              ? ["src/DemoApp.tsx", "src/components/TimerPanel.tsx", "README.md"]
              : [
                  "src/App.tsx",
                  "src/components/session/InputBar.tsx",
                  "README.md",
                  "src/features/deep-context/adapters/anthropic-session-stream-router.ts",
                  "src/features/deep-context/adapters/openai-compatible-stream-router.ts",
                  "src/features/deep-context/components/RunEvidenceTimeline.tsx",
                  "src/features/deep-context/components/ProjectArchiveInspector.tsx",
                  "src/features/deep-context/lib/workspace-boundary-policy.ts",
                  "src/features/deep-context/lib/markdown-diagram-normalizer.ts",
                  "src/features/deep-context/tests/composer-chip-overflow.fixture.ts",
                  "src/features/deep-context/docs/long-path-reference-material.md",
                ];
            return files.filter((path) => path.toLowerCase().includes(String(args.query ?? "").toLowerCase()));
          }
        case "toggle_capability":
          return undefined;
        case "get_api_key_status":
          // @ts-expect-error mock
          if (window.__mockApiKeyStatus) return window.__mockApiKeyStatus;
          return [{ provider: "deepseek", set: true, preview: "sk-e0...23ef" }];
        case "get_project_runtime_status":
          // @ts-expect-error mock
          window.__lastProjectRuntimeStatusArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "get_project_checkpoint_status":
          // @ts-expect-error mock
          window.__lastProjectCheckpointStatusArgs = args;
          return {
            ...projectCheckpointStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "start_project_dev_server":
          // @ts-expect-error mock
          window.__lastStartProjectDevServerArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
            running: true,
            managed: true,
            can_start: false,
            can_stop: true,
            can_open: true,
          };
        case "stop_project_dev_server":
          // @ts-expect-error mock
          window.__lastStopProjectDevServerArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "open_project_preview":
          // @ts-expect-error mock
          window.__lastOpenProjectPreviewArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
            running: true,
            managed: true,
            can_start: false,
            can_stop: true,
            can_open: true,
          };
        case "create_project_checkpoint":
          // @ts-expect-error mock
          window.__lastCreateProjectCheckpointArgs = args;
          return {
            ...projectCheckpointStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "restore_project_checkpoint":
          // @ts-expect-error mock
          window.__lastRestoreProjectCheckpointArgs = args;
          return {
            ...projectCheckpointStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "preview_file":
          // @ts-expect-error mock
          window.__lastPreviewFileArgs = args;
          return {
            path: `${String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir)}/${String(args.path ?? "src/App.tsx")}`,
            display_path: String(args.path ?? "src/App.tsx"),
            requested_line: args.line ?? null,
            start_line: 1,
            total_lines: 3,
            lines: [
              { number: 1, content: "export function Demo() {", is_target: args.line === 1 },
              { number: 2, content: "  return null;", is_target: args.line === 2 },
              { number: 3, content: "}", is_target: args.line === 3 },
            ],
          };
        case "open_file":
          // @ts-expect-error mock
          window.__lastOpenFileArgs = args;
          return undefined;
        case "list_mcp_context_sources":
          return mcpContextSources;
        case "list_memories":
          return [];
        case "get_workflow_state":
          return null;
        case "override_workflow_route":
          return {
            session_id: String(args.sessionId ?? "session"),
            route: args.action === "debug" ? "recovery" : args.action === "verify" ? "verification" : args.action === "plan_first" ? "workflow" : "direct",
            phase: args.action === "debug" ? "debugging" : args.action === "verify" ? "verifying" : args.action === "plan_first" ? "clarifying" : "idle",
            beginner_label: args.action === "debug" ? "遇到问题，正在排查" : args.action === "verify" ? "正在检查结果" : args.action === "plan_first" ? "先梳理想法" : "直接回答",
            developer_label: String(args.action ?? "direct"),
            matched_signals: ["manual override"],
            reason: "用户手动切换了当前工作方式。",
            gate: "none",
            override_actions: ["direct", "plan_first", "debug", "verify"],
            spec_path: null,
            plan_path: null,
            checkpoint_id: null,
            updated_at: Date.now(),
          };
        case "get_forge_wiki_state":
          return forgeWikiState(projectPath, forgeWikiExists);
        case "init_forge_wiki":
          forgeWikiExists = true;
          return forgeWikiState(projectPath, true);
        case "list_forge_wiki_pages":
          return forgeWikiExists ? forgeWikiPages.map((page) => ({ ...page, project_path: projectPath })) : [];
        case "read_forge_wiki_page":
          return args.pagePath === "tasks.md" ? "# 当前任务\n\n覆盖项目档案面板。" : "# 项目概览\n\n项目记录预览。";
        case "select_forge_wiki_context":
          return [
            {
              page_id: "tasks",
              title: "当前任务",
              path: "tasks.md",
              kind: "tasks",
              summary: "当前任务、验收步骤和后续事项。",
              score: 0.96,
              reason: "和当前请求最相关",
              injected: true,
            },
          ];
        case "create_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposal(projectPath, args);
          forgeWikiProposals.set(String(proposal.id), proposal);
          return proposal;
        }
        case "accept_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
          const accepted = { ...proposal, status: "accepted" };
          forgeWikiProposals.set(String(accepted.id), accepted);
          return accepted;
        }
        case "discard_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
          const discarded = { ...proposal, status: "discarded" };
          forgeWikiProposals.set(String(discarded.id), discarded);
          return discarded;
        }
        default:
          return undefined;
      }
    };
    // @ts-expect-error mock
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "plugin:event|listen") {
          // @ts-expect-error listeners
          if (!window.__tauriListeners[args.event as string]) window.__tauriListeners[args.event as string] = [];
          const callback = callbacks.get(args.handler as number);
          if (callback) {
            // @ts-expect-error listeners
            window.__tauriListeners[args.event as string].push(callback);
          }
          return args.handler;
        }
        if (cmd === "plugin:event|unlisten") {
          const event = args.event as string;
          const id = args.eventId as number;
          // @ts-expect-error listeners
          window.__tauriListeners[event] = (window.__tauriListeners[event] ?? []).filter((fn: unknown) => fn !== callbacks.get(id));
          callbacks.delete(id);
          return undefined;
        }
        return window.__tauriMockIPC?.(cmd, args);
      },
      transformCallback: (callback: (data: unknown) => void) => {
        callbackId += 1;
        callbacks.set(callbackId, callback);
        return callbackId;
      },
      unregisterCallback: (id: number) => {
        callbacks.delete(id);
      },
      callbacks,
    };
    // @ts-expect-error mock
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (_event: string, id: number) => {
        callbacks.delete(id);
      },
    };
    // @ts-expect-error listeners
    window.__tauriListeners = {};
    // Mock Tauri listen()
    // @ts-expect-error
    window.__TAURI__ = {
      event: {
        listen: (event: string, fn: (data: unknown) => void) => {
          // @ts-expect-error
          if (!window.__tauriListeners[event]) window.__tauriListeners[event] = [];
          // @ts-expect-error
          window.__tauriListeners[event].push(fn);
          return () => {};
        },
      },
    };
  }, { initialWorkingDir });
}

async function holdSendInput(page: Page) {
  await page.evaluate(() => {
    // @ts-expect-error mock
    const original = window.__tauriMockIPC;
    const calls: Record<string, unknown>[] = [];
    const resolvers: Array<() => void> = [];

    // @ts-expect-error mock
    window.__heldSendInput = {
      calls,
      releaseNext: () => {
        const resolve = resolvers.shift();
        resolve?.();
      },
    };

    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === "send_input") {
        calls.push(args);
        await new Promise<void>((resolve) => {
          resolvers.push(resolve);
        });
        return undefined;
      }
      return original?.(cmd, args);
    };
  });
}

async function expectHeldSendInput(page: Page, textIncludes: string) {
  await expect.poll(async () => page.evaluate(() => {
    // @ts-expect-error mock
    return window.__heldSendInput?.calls.length ?? 0;
  })).toBe(1);

  const [call] = await page.evaluate(() => {
    // @ts-expect-error mock
    return window.__heldSendInput?.calls ?? [];
  });
  expect(String(call.text)).toContain(textIncludes);
  return call;
}

async function getLastSendInputArgs(page: Page): Promise<Record<string, unknown> | undefined> {
  return page.evaluate(() => {
    // @ts-expect-error mock
    return window.__lastSendInputArgs;
  });
}

async function expectLastSendInputArgs(page: Page, expected: Record<string, unknown>) {
  await expect.poll(async () => getLastSendInputArgs(page)).toMatchObject(expected);
  const args = await getLastSendInputArgs(page);
  expect(args).toBeDefined();
  return args!;
}

async function expectNoSendInput(page: Page) {
  await expect(await getLastSendInputArgs(page)).toBeUndefined();
}

async function releaseHeldSendInput(page: Page) {
  await page.evaluate(() => {
    // @ts-expect-error mock
    window.__heldSendInput?.releaseNext();
  });
}

function projectArchive(page: Page) {
  return page.getByTestId("project-archive-panel");
}

async function openProjectArchive(page: Page, section?: "records") {
  const archive = projectArchive(page);
  if (await archive.isVisible().catch(() => false)) return archive;

  await page.getByRole("button", { name: "打开项目档案" }).click();
  if (await archive.isVisible({ timeout: 750 }).catch(() => false)) return archive;

  await page.evaluate((targetSection) => {
    window.dispatchEvent(new CustomEvent("open-hub", {
      detail: targetSection ? { section: targetSection } : undefined,
    }));
  }, section);
  await expect(archive).toBeVisible();
  return archive;
}

async function expandArchiveRecords(page: Page) {
  const archive = projectArchive(page);
  const records = archive.getByTestId("archive-disclosure-records");
  const trigger = records.getByRole("button", { name: /项目记录/ }).first();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await trigger.click();
  }
  return records;
}

async function expandArchiveFiles(page: Page) {
  const archive = projectArchive(page);
  const files = archive.getByTestId("archive-disclosure-files");
  const trigger = files.getByRole("button", { name: /资料/ }).first();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await trigger.click();
  }
  return files;
}

test.describe("Frontend maintainability guardrails", () => {
  test("brand theme styles avoid pre-brand warm gray literals", () => {
    const styleFiles = [
      "src/styles/capabilities.css",
      "src/styles/composer.css",
      "src/styles/diff.css",
      "src/styles/globals.css",
      "src/styles/layout.css",
      "src/styles/markdown.css",
      "src/styles/messages.css",
      "src/styles/process.css",
    ];
    const deprecatedBrandLiterals = [
      "rgba(194, 187, 174",
      "rgba(210, 204, 190",
      "#181816",
      "#22221E",
      "#282822",
      "#99958B",
      "#DCB671",
    ];

    for (const path of styleFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      for (const literal of deprecatedBrandLiterals) {
        expect(source, `${path} should not contain ${literal}`).not.toContain(literal);
      }
    }
  });

  test("warm precision brand assets avoid cold graphite and blue code literals", () => {
    const checkedFiles = [
      "src/assets/forge-mark.svg",
      "src/styles/diff.css",
      "src/styles/markdown.css",
    ];
    const coldBrandLiterals = [
      "#0D0D0D",
      "#1C1C1C",
      "#2A2A2A",
      "rgba(9, 11, 14",
      "rgba(10, 12, 15",
      "#d6deeb",
      "#D6DEEB",
      "#CBD5E1",
      "#7CAED8",
      "rgba(148, 163, 184",
      "rgba(188, 198, 214",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      for (const literal of coldBrandLiterals) {
        expect(source, `${path} should not contain ${literal}`).not.toContain(literal);
      }
    }
  });

  test("reader affordances avoid default blue links and cold hover surfaces", () => {
    const checkedFiles = [
      "src/styles/markdown.css",
      "src/styles/messages.css",
    ];
    const coldAffordanceLiterals = [
      "#6BA6D8",
      "rgba(107, 166, 216",
      "rgba(27, 30, 37",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      for (const literal of coldAffordanceLiterals) {
        expect(source, `${path} should not contain ${literal}`).not.toContain(literal);
      }
    }
  });

  test("warm precision semantic accents use shared tokens instead of legacy greens and preview blues", () => {
    const checkedFiles = [
      "src/components/messages/FilePreviewBody.tsx",
      "src/components/messages/filePreviewPresentation.ts",
      "src/styles/composer.css",
      "src/styles/diff.css",
      "src/styles/globals.css",
      "src/styles/markdown.css",
      "src/styles/process.css",
      "src/styles/tokens.css",
    ];
    const legacySemanticLiterals = [
      "rgba(91,155,213",
      "#8FC7FF",
      "text-[#c9c9c9]",
      "#4A9E6B",
      "rgba(74, 158, 107",
      "#8BCB9D",
      "#7AB88E",
      "#9BC7A8",
      "#8FB8C9",
      "#B8A0D9",
      "#78C08D",
      "#D49CAB",
      "#D9622A",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      for (const literal of legacySemanticLiterals) {
        expect(source, `${path} should not contain ${literal}`).not.toContain(literal);
      }
    }
  });

  test("brand metaphors avoid fire, magic spectacle, and raw agent framing", () => {
    const checkedFiles = [
      "src/components/context/ProjectOverviewCard.tsx",
      "src/components/layout/Sidebar.tsx",
      "src/lib/capability-icons.ts",
      "src/styles/tokens.css",
    ];
    const offBrandMetaphors = [
      "--forge-ember",
      "WandSparkles",
      "Sparkles",
      "Local agent",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      for (const literal of offBrandMetaphors) {
        expect(source, `${path} should not contain ${literal}`).not.toContain(literal);
      }
    }

    const sidebar = readFileSync(resolve(process.cwd(), "src/components/layout/Sidebar.tsx"), "utf8");
    expect(sidebar).toContain("Local workbench");
  });

  test("brand surfaces avoid decorative radial glows", () => {
    const checkedFiles = [
      "src/styles/capabilities.css",
      "src/styles/composer.css",
      "src/styles/globals.css",
      "src/styles/layout.css",
      "src/styles/messages.css",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      expect(source, `${path} should not contain decorative radial-gradient glows`).not.toContain("radial-gradient");
    }
  });

  test("modal overlays stay warm and legible without dark glass", () => {
    const checkedFiles = [
      "src/components/ui/dialog.tsx",
      "src/components/ui/sheet.tsx",
      "src/styles/globals.css",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      expect(source, `${path} should not use a dark application overlay`).not.toContain("rgba(36,42,36,0.18)");
      expect(source, `${path} should not use a dark application overlay`).not.toContain("rgba(36, 42, 36, 0.18)");
      expect(source, `${path} should not use Tailwind black overlay utilities`).not.toContain("bg-black");
      expect(source, `${path} should avoid blurred overlay glass`).not.toContain("backdrop-blur-xs");
    }

    const dialog = readFileSync(resolve(process.cwd(), "src/components/ui/dialog.tsx"), "utf8");
    const sheet = readFileSync(resolve(process.cwd(), "src/components/ui/sheet.tsx"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(dialog).toContain("bg-[rgba(251,244,234,0.78)]");
    expect(sheet).toContain("bg-[rgba(251,244,234,0.78)]");
    expect(globals).toContain("background: rgba(251, 244, 234, 0.78);");
  });

  test("composer surfaces avoid decorative overlay lines", () => {
    const composer = readFileSync(resolve(process.cwd(), "src/styles/composer.css"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(composer).not.toContain(".forge-composer::before");
    expect(composer).not.toContain(".forge-composer[data-state=\"paused\"]::before");
    expect(composer).toContain("backdrop-filter: none;");
    expect(globals).not.toContain(".forge-empty-composer::before");
  });

  test("project archive scan rows avoid low-alpha helper text", () => {
    const checkedFiles = [
      "src/components/context/ProjectOverviewCard.tsx",
      "src/components/context/FirstLoopCard.tsx",
      "src/components/context/ActiveContextSection.tsx",
      "src/components/context/WikiSections.tsx",
    ];

    for (const path of checkedFiles) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      expect(source, `${path} should keep archive helper copy readable`).not.toContain("text-muted-foreground/55");
      expect(source, `${path} should keep archive helper copy readable`).not.toContain("text-muted-foreground/60");
      expect(source, `${path} should keep archive helper copy readable`).not.toContain("text-muted-foreground/65");
      expect(source, `${path} should avoid vertical accent rule fragments`).not.toContain("border-l border-border");
      expect(source, `${path} should avoid loose horizontal rule fragments`).not.toContain("border-t border-border");
      expect(source, `${path} should avoid loose horizontal rule fragments`).not.toContain("border-b border-border");
    }
  });

  test("shared card primitive keeps product radius within the design contract", () => {
    const card = readFileSync(resolve(process.cwd(), "src/components/ui/card.tsx"), "utf8");

    expect(card).not.toContain("rounded-xl");
    expect(card).not.toContain("rounded-b-xl");
    expect(card).not.toContain("rounded-2xl");
    expect(card).not.toContain("rounded-3xl");
  });

  test("shared button primitive forwards refs for Base UI trigger composition", () => {
    const button = readFileSync(resolve(process.cwd(), "src/components/ui/button.tsx"), "utf8");

    expect(button).toContain("React.forwardRef");
    expect(button).toContain("ref={ref}");
    expect(button).toContain("Button.displayName");
  });

  test("dialog content forwards refs for scoped surface animation", () => {
    const dialog = readFileSync(resolve(process.cwd(), "src/components/ui/dialog.tsx"), "utf8");
    const settings = readFileSync(resolve(process.cwd(), "src/components/settings/SettingsDialog.tsx"), "utf8");

    expect(dialog).toContain("React.forwardRef");
    expect(dialog).toContain("ref={ref}");
    expect(dialog).toContain("DialogContent.displayName");
    expect(settings).toContain("dialogRef");
    expect(settings).toContain("ref={dialogRef}");
    expect(settings).toContain("gsap.timeline");
    expect(settings).toContain(".forge-settings-summary-item, [data-testid='settings-provider-row'], .forge-settings-danger-zone");
  });

  test("settings summarize provider readiness before detailed rows", () => {
    const settings = readFileSync(resolve(process.cwd(), "src/components/settings/SettingsDialog.tsx"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(settings).toContain("settings-summary-strip");
    expect(settings).toContain("SettingsSummaryItem");
    expect(settings).toContain("configuredCount");
    expect(settings).toContain("keyByProvider");
    expect(settings).toContain("knownProviderStatuses");
    expect(settings).toContain("forge-settings-provider-mark");
    expect(settings).not.toContain("text-muted-foreground/60");
    const hubPanel = readFileSync(resolve(process.cwd(), "src/components/layout/HubPanel.tsx"), "utf8");
    expect(hubPanel).not.toContain("border-t border-border pt-3 first:border-t-0 first:pt-0");
    expect(globals).toContain(".forge-settings-summary-strip");
    expect(globals).toContain("grid-template-columns: repeat(3, minmax(0, 1fr))");
    expect(globals).toContain(".forge-settings-provider-mark[data-configured=\"true\"]");
    expect(globals).toContain(".forge-settings-preferences-panel");
    expect(globals).toContain("gap: 0.5rem;");
    expect(globals).not.toContain(".forge-settings-row:first-child");
  });

  test("capability manager summarizes capability state with scoped motion", () => {
    const manager = readFileSync(resolve(process.cwd(), "src/components/settings/CapabilityManager.tsx"), "utf8");
    const styles = readFileSync(resolve(process.cwd(), "src/styles/capabilities.css"), "utf8");

    expect(manager).toContain("managerRef");
    expect(manager).toContain("scope: managerRef");
    expect(manager).toContain("capability-summary-strip");
    expect(manager).toContain("data-forge-motion=\"capability-entry\"");
    expect(manager).toContain("[data-forge-motion='capability-entry']");
    expect(manager).toContain("filterCapabilities");
    expect(styles).toContain(".forge-capability-summary-strip");
    expect(styles).toContain(".forge-capability-summary-item");
  });

  test("command palette uses scoped motion on desktop shell entries", () => {
    const commandPalette = readFileSync(resolve(process.cwd(), "src/components/CommandPalette.tsx"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(commandPalette).toContain("paletteRef");
    expect(commandPalette).toContain("scope: paletteRef");
    expect(commandPalette).toContain("prefersReducedMotion");
    expect(commandPalette).toContain("data-forge-motion=\"command-entry\"");
    expect(commandPalette).toContain("[data-forge-motion='command-entry']");
    expect(globals).toContain(".forge-command-motion-root");
    expect(globals).toContain("[data-forge-motion=\"command-entry\"]");
  });

  test("project archive opens with a compact inspector summary and scoped motion", () => {
    const hub = readFileSync(resolve(process.cwd(), "src/components/layout/HubPanel.tsx"), "utf8");
    const archiveStyles = readFileSync(resolve(process.cwd(), "src/styles/archive.css"), "utf8");

    expect(hub).toContain("project-archive-summary-strip");
    expect(hub).toContain("ArchiveSummaryStrip");
    expect(hub).toContain("data-forge-motion=\"archive-section\"");
    expect(hub).toContain("gsap.timeline");
    expect(hub).toContain("[data-forge-motion='archive-section']");
    expect(archiveStyles).toContain(".forge-archive-summary-strip");
    expect(archiveStyles).toContain(".forge-inspector-title-block");
  });

  test("project delivery status uses compact inspector motion", () => {
    const projectStatus = readFileSync(resolve(process.cwd(), "src/components/layout/ProjectStatusCard.tsx"), "utf8");
    const archiveStyles = readFileSync(resolve(process.cwd(), "src/styles/archive.css"), "utf8");

    expect(projectStatus).toContain("data-testid=\"project-status-card\"");
    expect(projectStatus).toContain("data-testid=\"project-status-summary\"");
    expect(projectStatus).toContain("data-forge-motion=\"project-status-entry\"");
    expect(projectStatus).toContain("scope: cardRef");
    expect(projectStatus).toContain("prefersReducedMotion");
    expect(projectStatus).toContain("forge-project-status-summary");
    expect(archiveStyles).toContain(".forge-project-status");
    expect(archiveStyles).toContain(".forge-project-status-metric");
    expect(archiveStyles).toContain("[data-forge-motion=\"project-status-entry\"]");
  });

  test("markdown reader styles are owned by the markdown stylesheet", () => {
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");
    const markdown = readFileSync(resolve(process.cwd(), "src/styles/markdown.css"), "utf8");

    expect(globals).toContain('@import "./markdown.css";');
    for (const selector of [
      ".markdown-content",
      ".code-surface",
      ".diagram-surface",
      ".forge-inline-code",
      ".forge-file-ref",
    ]) {
      expect(markdown).toContain(selector);
      expect(globals).not.toContain(selector);
    }
    expect(markdown).not.toContain("border-left: 2px solid");
    expect(markdown).not.toContain("linear-gradient(var(--forge-code-grid-line)");
  });

  test("project archive inspector styles are owned by the archive stylesheet", () => {
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");
    const archive = readFileSync(resolve(process.cwd(), "src/styles/archive.css"), "utf8");

    expect(globals).toContain('@import "./archive.css";');
    for (const selector of [
      ".forge-inspector",
      ".forge-inspector-header",
      ".forge-inspector-body",
      ".forge-disclosure-row",
      ".forge-project-status",
    ]) {
      expect(archive).toContain(selector);
      expect(globals).not.toContain(selector);
    }
  });

  test("composer static commands and local types are owned by composer modules", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const commands = readFileSync(resolve(process.cwd(), "src/components/session/composerCommands.ts"), "utf8");
    const types = readFileSync(resolve(process.cwd(), "src/components/session/composerTypes.ts"), "utf8");

    expect(commands).toContain("COMPOSER_COMMANDS");
    expect(commands).toContain("/code-review");
    expect(commands).toContain("检查有没有风险");
    expect(types).toContain("ComposerChip");
    expect(types).toContain("ComposerMenuMode");
    expect(inputBar).not.toContain("const COMMANDS");
    expect(inputBar).not.toContain("interface Chip");
  });

  test("composer chip tray rendering is owned by its subcomponent", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const chipTray = readFileSync(resolve(process.cwd(), "src/components/session/ComposerChipTray.tsx"), "utf8");

    expect(inputBar).toContain("ComposerChipTray");
    expect(chipTray).toContain("forge-composer-chips");
    expect(chipTray).toContain("forge-composer-chip-label");
    expect(inputBar).not.toContain("forge-composer-chip-label");
  });

  test("composer suggestion menu rendering is owned by its subcomponent", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const suggestionMenu = readFileSync(resolve(process.cwd(), "src/components/session/ComposerSuggestionMenu.tsx"), "utf8");

    expect(inputBar).toContain("ComposerSuggestionMenu");
    expect(suggestionMenu).toContain("forge-composer-suggestion-menu");
    expect(suggestionMenu).toContain("引用文件");
    expect(suggestionMenu).toContain("常用请求");
    expect(inputBar).not.toContain("forge-composer-suggestion-menu");
  });

  test("composer model menu rendering is owned by its subcomponent", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const modelMenu = readFileSync(resolve(process.cwd(), "src/components/session/ComposerModelMenu.tsx"), "utf8");

    expect(inputBar).toContain("ComposerModelMenu");
    expect(modelMenu).toContain("forge-composer-model-menu");
    expect(modelMenu).toContain("role=\"menu\"");
    expect(modelMenu).toContain("menuitemradio");
    expect(inputBar).not.toContain("forge-composer-model-menu");
  });

  test("composer toolbar rendering is owned by its subcomponent", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const toolbar = readFileSync(resolve(process.cwd(), "src/components/session/ComposerToolbar.tsx"), "utf8");

    expect(inputBar).toContain("ComposerToolbar");
    expect(toolbar).toContain("forge-composer-toolbar");
    expect(toolbar).toContain("composer-model-chip");
    expect(toolbar).toContain("composer-send");
    expect(inputBar).not.toContain("forge-composer-toolbar");
  });

  test("composer suggestion state is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const suggestionsHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerSuggestions.ts"), "utf8");

    expect(inputBar).toContain("useComposerSuggestions");
    expect(suggestionsHook).toContain("searchWorkspaceFiles");
    expect(suggestionsHook).toContain("syncSuggestionsForInput");
    expect(suggestionsHook).toContain("toggleSuggestion");
    expect(inputBar).not.toContain("searchWorkspaceFiles");
    expect(inputBar).not.toContain("setAtResults");
  });

  test("composer draft text behavior is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const draftHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerDraft.ts"), "utf8");

    expect(inputBar).toContain("useComposerDraft");
    expect(draftHook).toContain("COMPOSER_MAX_INPUT_HEIGHT");
    expect(draftHook).toContain("pendingInput");
    expect(draftHook).toContain("valueRef");
    expect(inputBar).not.toContain("COMPOSER_MAX_INPUT_HEIGHT");
    expect(inputBar).not.toContain("setPendingInput(\"\")");
  });

  test("composer submit flow is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const submitHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerSubmit.ts"), "utf8");

    expect(inputBar).toContain("useComposerSubmit");
    expect(submitHook).toContain("createProjectCheckpoint");
    expect(submitHook).toContain("buildFirstLoopAgentPrompt");
    expect(submitHook).toContain("ComposerCapabilitySelection");
    expect(inputBar).not.toContain("createProjectCheckpoint");
    expect(inputBar).not.toContain("buildFirstLoopAgentPrompt");
  });

  test("composer model selection state is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const modelHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerModelMenu.ts"), "utf8");

    expect(inputBar).toContain("useComposerModelMenu");
    expect(modelHook).toContain("getModelContextWindow");
    expect(modelHook).toContain("setSelectedModel");
    expect(modelHook).toContain("toggleModelMenu");
    expect(inputBar).not.toContain("setSelectedModel");
    expect(inputBar).not.toContain("getModelLabel");
  });

  test("composer resume state is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const resumeHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerResume.ts"), "utf8");

    expect(inputBar).toContain("useComposerResume");
    expect(resumeHook).toContain("setIsResuming");
    expect(resumeHook).toContain("resumeError");
    expect(resumeHook).toContain("handleResume");
    expect(inputBar).not.toContain("setIsResuming");
  });

  test("composer chip state is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const chipHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerChips.ts"), "utf8");

    expect(inputBar).toContain("useComposerChips");
    expect(chipHook).toContain("crypto.randomUUID");
    expect(chipHook).toContain("removeTriggerTextForChip");
    expect(chipHook).toContain("clearChips");
    expect(inputBar).not.toContain("setChips");
  });

  test("composer keyboard behavior is owned by its hook", () => {
    const inputBar = readFileSync(resolve(process.cwd(), "src/components/session/InputBar.tsx"), "utf8");
    const keyboardHook = readFileSync(resolve(process.cwd(), "src/components/session/useComposerKeyboard.ts"), "utf8");

    expect(inputBar).toContain("useComposerKeyboard");
    expect(keyboardHook).toContain("COMPOSER_COMMANDS");
    expect(keyboardHook).toContain("commitActiveSuggestion");
    expect(keyboardHook).toContain("removeLastChip");
    expect(inputBar).not.toContain("ArrowDown");
    expect(inputBar).not.toContain("COMPOSER_COMMANDS");
  });

  test("process activity summary is owned by its view model", () => {
    const group = readFileSync(resolve(process.cwd(), "src/components/messages/ToolActivityGroup.tsx"), "utf8");
    const viewModel = readFileSync(resolve(process.cwd(), "src/components/messages/processActivity.ts"), "utf8");

    expect(group).toContain("deriveToolActivityView");
    expect(viewModel).toContain("summarizeActivity");
    expect(viewModel).toContain("处理遇到问题");
    expect(viewModel).toContain("processActivityTone");
    expect(group).not.toContain("function summarizeActivity");
    expect(group).not.toContain("处理遇到问题");
  });

  test("process activity summary row is owned by a focused subview", () => {
    const group = readFileSync(resolve(process.cwd(), "src/components/messages/ToolActivityGroup.tsx"), "utf8");
    const summary = readFileSync(resolve(process.cwd(), "src/components/messages/ToolActivitySummary.tsx"), "utf8");

    expect(group).toContain("ToolActivitySummary");
    expect(summary).toContain("forge-tool-activity-summary");
    expect(summary).toContain("forge-tool-activity-summary-item");
    expect(summary).toContain("data-running-icon");
    expect(summary).toContain("CollapsibleTrigger");
    expect(group).not.toContain("forge-tool-activity-summary-item");
    expect(group).not.toContain("data-running-icon");
  });

  test("process activity expanded details are owned by a focused subview", () => {
    const group = readFileSync(resolve(process.cwd(), "src/components/messages/ToolActivityGroup.tsx"), "utf8");
    const details = readFileSync(resolve(process.cwd(), "src/components/messages/ToolActivityDetails.tsx"), "utf8");

    expect(group).toContain("ToolActivityDetails");
    expect(details).toContain("forge-tool-activity-list");
    expect(details).toContain("ShellCard");
    expect(details).toContain("ToolCallCard");
    expect(group).not.toContain("ShellCard");
    expect(group).not.toContain("ToolCallCard");
    expect(group).not.toContain("forge-tool-activity-list");
  });

  test("tool call presentation is owned by its view model", () => {
    const card = readFileSync(resolve(process.cwd(), "src/components/messages/ToolCallCard.tsx"), "utf8");
    const styles = readFileSync(resolve(process.cwd(), "src/styles/messages.css"), "utf8");
    const viewModel = readFileSync(resolve(process.cwd(), "src/components/messages/processToolPresentation.ts"), "utf8");

    expect(card).toContain("deriveToolCallView");
    expect(card).toContain("forge-evidence-row");
    expect(viewModel).toContain("TOOL_COPY");
    expect(viewModel).toContain("summarizeToolInput");
    expect(viewModel).toContain("summarizeToolResult");
    expect(card).not.toContain("const TOOL_COPY");
    expect(card).not.toContain("function summarizeToolInput");
    expect(card).not.toContain("tool-machine-meter");
    expect(card).not.toContain("tool-machine-led");
    expect(styles).not.toContain(".tool-machine-meter");
    expect(styles).not.toContain(".tool-machine-led");
  });

  test("shell output presentation is owned by its view model", () => {
    const card = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCard.tsx"), "utf8");
    const styles = readFileSync(resolve(process.cwd(), "src/styles/messages.css"), "utf8");
    const viewModel = readFileSync(resolve(process.cwd(), "src/components/messages/processShellPresentation.ts"), "utf8");

    expect(card).toContain("deriveShellView");
    expect(viewModel).toContain("parseShellOutput");
    expect(viewModel).toContain("outputSections");
    expect(viewModel).toContain("exitCode");
    expect(card).not.toContain("function parseShellOutput");
    expect(card).not.toContain("shell-reel-cap");
    expect(styles).not.toContain(".shell-reel-cap");
  });

  test("shell card header is owned by a focused subview", () => {
    const card = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCard.tsx"), "utf8");
    const header = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCardHeader.tsx"), "utf8");

    expect(card).toContain("ShellCardHeader");
    expect(header).toContain("shell-card-trigger");
    expect(header).toContain("forge-log-status");
    expect(header).toContain("shell-exit-code");
    expect(header).toContain("CollapsibleTrigger");
    expect(card).not.toContain("shell-card-trigger");
    expect(card).not.toContain("forge-log-status");
  });

  test("shell output detail rendering is owned by focused subviews", () => {
    const card = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCard.tsx"), "utf8");
    const detail = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCardDetail.tsx"), "utf8");
    const output = readFileSync(resolve(process.cwd(), "src/components/messages/ShellOutputSections.tsx"), "utf8");

    expect(card).toContain("ShellCardDetail");
    expect(detail).toContain("navigator.clipboard");
    expect(detail).toContain("log-detail-header");
    expect(output).toContain("shell-output-section");
    expect(output).toContain("forge-shell-output-label");
    expect(card).not.toContain("navigator.clipboard");
    expect(card).not.toContain("shell-output-section");
  });

  test("process feedback focus affordance is token-driven", () => {
    const processStyles = readFileSync(resolve(process.cwd(), "src/styles/process.css"), "utf8");

    expect(processStyles).toContain(".forge-log-line:focus-visible");
    expect(processStyles).toContain(".forge-tool-activity-summary:focus-visible");
    expect(processStyles).toContain(".forge-status-trigger:focus-visible");
    expect(processStyles).toContain("var(--forge-focus-ring)");
  });

  test("prototype motion uses scoped GSAP with reduced motion support", () => {
    const messageList = readFileSync(resolve(process.cwd(), "src/components/chat/MessageList.tsx"), "utf8");
    const shellCard = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCard.tsx"), "utf8");
    const motion = readFileSync(resolve(process.cwd(), "src/lib/forgeMotion.ts"), "utf8");

    expect(motion).toContain("@gsap/react");
    expect(motion).toContain("gsap.registerPlugin(useGSAP)");
    expect(motion).toContain("prefersReducedMotion");
    expect(motion).toContain("(prefers-reduced-motion: reduce)");
    expect(messageList).toContain("useGSAP");
    expect(messageList).toContain("scope: laneRef");
    expect(shellCard).toContain("data-forge-motion=\"shell-detail\"");
  });

  test("empty workbench keeps CSS motion hooks without eager GSAP runtime", () => {
    const appShell = readFileSync(resolve(process.cwd(), "src/components/layout/AppShell.tsx"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(appShell).not.toContain("@/lib/forgeMotion");
    expect(appShell).not.toContain("useGSAP");
    expect(appShell).not.toContain("emptyShellRef");
    expect(appShell).toContain("data-forge-motion=\"empty-entry\"");
    expect(appShell).toContain("data-forge-motion=\"empty-composer\"");
    expect(globals).toContain("[data-forge-motion=\"empty-entry\"]");
    expect(globals).toContain("will-change: transform, opacity");
    expect(globals).not.toContain(".forge-empty-entry-card::before");
    expect(globals).toContain(".forge-empty-entry-card[data-active=\"true\"] .forge-empty-entry-icon");
  });

  test("sidebar keeps CSS motion hooks without eager GSAP runtime", () => {
    const sidebar = readFileSync(resolve(process.cwd(), "src/components/layout/Sidebar.tsx"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(sidebar).not.toContain("@/lib/forgeMotion");
    expect(sidebar).not.toContain("useGSAP");
    expect(sidebar).not.toContain("sidebarRef");
    expect(sidebar).toContain("data-forge-motion=\"sidebar-entry\"");
    expect(sidebar).not.toContain("data-forge-motion=\"sidebar-history-row\"");
    expect(globals).toContain(".forge-sidebar-history-list");
    expect(globals).toContain(".forge-sidebar-history-group-label");
    expect(globals).not.toContain(".forge-sidebar-history-row[data-active=\"true\"]::before");
    expect(globals).toContain("[data-forge-motion=\"sidebar-entry\"]");
  });

  test("settings dialog stays behind a lazy boundary from the sidebar", () => {
    const sidebar = readFileSync(resolve(process.cwd(), "src/components/layout/Sidebar.tsx"), "utf8");

    expect(sidebar).not.toContain("import { SettingsDialog }");
    expect(sidebar).toContain("lazy(() => import(\"@/components/settings/SettingsDialog\")");
    expect(sidebar).toContain("<LazySettingsDialog");
  });

  test("assistant prose keeps the lightweight Codex-style message shape", () => {
    const textBlock = readFileSync(resolve(process.cwd(), "src/components/messages/TextBlock.tsx"), "utf8");
    const messages = readFileSync(resolve(process.cwd(), "src/styles/messages.css"), "utf8");

    expect(textBlock).toContain("forge-assistant-avatar");
    expect(textBlock).toContain("data-message-role=\"assistant\"");
    expect(messages).toContain(".forge-assistant-message");
    expect(messages).toContain("background: transparent");
    expect(messages).toContain(".forge-assistant-avatar");
    expect(messages).not.toContain(".forge-assistant-message {\n    border: 1px solid");
  });

  test("process evidence rows stay collapsed and inline by default", () => {
    const shellHeader = readFileSync(resolve(process.cwd(), "src/components/messages/ShellCardHeader.tsx"), "utf8");
    const processStyles = readFileSync(resolve(process.cwd(), "src/styles/process.css"), "utf8");

    expect(shellHeader).toContain("forge-evidence-row");
    expect(shellHeader).toContain("data-forge-motion=\"evidence-row\"");
    expect(processStyles).toContain(".forge-evidence-row");
    expect(processStyles).toContain("min-height: 2.75rem");
    expect(processStyles).toContain(".forge-log-line-command");
    expect(processStyles).toContain("text-overflow: ellipsis");
    expect(processStyles).not.toContain("rgba(81, 71, 55");
  });

  test("process status dots are owned by a shared component", () => {
    const thinking = readFileSync(resolve(process.cwd(), "src/components/messages/ThinkingBlock.tsx"), "utf8");
    const pending = readFileSync(resolve(process.cwd(), "src/components/messages/PendingBlock.tsx"), "utf8");
    const dots = readFileSync(resolve(process.cwd(), "src/components/messages/ProcessStatusDots.tsx"), "utf8");

    expect(thinking).toContain("ProcessStatusDots");
    expect(pending).toContain("ProcessStatusDots");
    expect(dots).toContain("forge-status-dots");
    expect(dots).toContain("animationDelay");
    expect(thinking).not.toContain("forge-status-dot");
    expect(pending).not.toContain("forge-status-dot");
  });

  test("message block routing is owned by the block renderer", () => {
    const messageList = readFileSync(resolve(process.cwd(), "src/components/chat/MessageList.tsx"), "utf8");
    const blockRenderer = readFileSync(resolve(process.cwd(), "src/components/chat/BlockRenderer.tsx"), "utf8");

    expect(messageList).toContain("MemoizedBlockRenderer");
    expect(blockRenderer).toContain("function BlockRenderer");
    expect(blockRenderer).toContain("switch (block.event_type)");
    expect(blockRenderer).toContain("MissingApiKeyCard");
    expect(messageList).not.toContain("switch (block.event_type)");
    expect(messageList).not.toContain("MissingApiKeyCard");
  });

  test("markdown rendering is owned by the markdown renderer module", () => {
    const textBlock = readFileSync(resolve(process.cwd(), "src/components/messages/TextBlock.tsx"), "utf8");
    const userMessage = readFileSync(resolve(process.cwd(), "src/components/messages/UserMessage.tsx"), "utf8");
    const markdownRenderer = readFileSync(resolve(process.cwd(), "src/components/messages/MarkdownRenderer.tsx"), "utf8");

    expect(textBlock).toContain("MarkdownRenderer");
    expect(userMessage).toContain("MarkdownRenderer");
    expect(markdownRenderer).toContain("ReactMarkdown");
    expect(markdownRenderer).toContain("stabilizeStreamingMarkdown");
    expect(markdownRenderer).toContain("extractMarkdownHeadings");
    expect(textBlock).not.toContain("ReactMarkdown");
    expect(textBlock).not.toContain("extractMarkdownHeadings");
    expect(userMessage).not.toContain("@/components/messages/TextBlock");
  });

  test("assistant streaming status reuses the shared process dots", () => {
    const textBlock = readFileSync(resolve(process.cwd(), "src/components/messages/TextBlock.tsx"), "utf8");
    const dots = readFileSync(resolve(process.cwd(), "src/components/messages/ProcessStatusDots.tsx"), "utf8");

    expect(textBlock).toContain("ProcessStatusDots");
    expect(textBlock).not.toContain("forge-status-dot");
    expect(dots).toContain("forge-status-dot");
  });

  test("diff presentation is owned by its view model", () => {
    const diffCard = readFileSync(resolve(process.cwd(), "src/components/messages/DiffCard.tsx"), "utf8");
    const viewModel = readFileSync(resolve(process.cwd(), "src/components/messages/diffPresentation.ts"), "utf8");

    expect(diffCard).toContain("deriveDiffView");
    expect(viewModel).toContain("parseDiff");
    expect(viewModel).toContain("INITIAL_VISIBLE_DIFF_LINES");
    expect(viewModel).toContain("DIFF_LINE_CLASS");
    expect(diffCard).not.toContain("function parseDiff");
    expect(diffCard).not.toContain("const DIFF_LINE_CLASS");
  });

  test("diff header actions are owned by a focused subview", () => {
    const diffCard = readFileSync(resolve(process.cwd(), "src/components/messages/DiffCard.tsx"), "utf8");
    const actions = readFileSync(resolve(process.cwd(), "src/components/messages/DiffHeaderActions.tsx"), "utf8");

    expect(diffCard).toContain("DiffHeaderActions");
    expect(actions).toContain("navigator.clipboard");
    expect(actions).toContain("openFile");
    expect(actions).toContain("LocateFixed");
    expect(diffCard).not.toContain("navigator.clipboard");
    expect(diffCard).not.toContain("openFile(");
    expect(diffCard).not.toContain("LocateFixed");
  });

  test("diff body rows and expansion are owned by a focused subview", () => {
    const diffCard = readFileSync(resolve(process.cwd(), "src/components/messages/DiffCard.tsx"), "utf8");
    const body = readFileSync(resolve(process.cwd(), "src/components/messages/DiffBody.tsx"), "utf8");

    expect(diffCard).toContain("DiffBody");
    expect(body).toContain("DIFF_LINE_CLASS");
    expect(body).toContain("diff-line-old-number");
    expect(body).toContain("forge-diff-body");
    expect(body).toContain("forge-diff-expand");
    expect(diffCard).not.toContain("DIFF_LINE_CLASS");
    expect(diffCard).not.toContain("forge-diff-body");
    expect(diffCard).not.toContain("forge-diff-expand");
  });

  test("diff patches collapse behind a lightweight evidence toggle", () => {
    const diffCard = readFileSync(resolve(process.cwd(), "src/components/messages/DiffCard.tsx"), "utf8");
    const diffStyles = readFileSync(resolve(process.cwd(), "src/styles/diff.css"), "utf8");
    const messageStyles = readFileSync(resolve(process.cwd(), "src/styles/messages.css"), "utf8");

    expect(diffCard).toContain("bodyOpen");
    expect(diffCard).toContain("diff-body-toggle");
    expect(diffCard).toContain("data-diff-open");
    expect(diffCard).toContain("data-forge-motion=\"diff-body\"");
    expect(diffCard).not.toContain("diff-filmstrip-perf");
    expect(diffCard).not.toContain("rgba(247, 241, 232");
    expect(diffStyles).toContain(".forge-diff-toggle");
    expect(diffStyles).toContain(".forge-diff-card[data-diff-open=\"false\"]");
    expect(messageStyles).not.toContain("border-left: 3px solid transparent");
  });

  test("confirmation copy and risk presentation are owned by its view model", () => {
    const confirmCard = readFileSync(resolve(process.cwd(), "src/components/messages/ConfirmCard.tsx"), "utf8");
    const confirmViews = readFileSync(resolve(process.cwd(), "src/components/messages/ConfirmViews.tsx"), "utf8");
    const viewModel = readFileSync(resolve(process.cwd(), "src/components/messages/confirmPresentation.ts"), "utf8");

    expect(confirmCard).toContain("deriveConfirmPromptView");
    expect(confirmViews).toContain("confirmRiskColor");
    expect(confirmViews).not.toContain("permission-ticket");
    expect(viewModel).toContain("kindLabels");
    expect(viewModel).toContain("helperTextForKind");
    expect(viewModel).toContain("boundaryCommandLabel");
    expect(confirmCard).not.toContain("const kindLabels");
    expect(confirmCard).not.toContain("function boundaryCommandLabel");
  });

  test("delivery summary parsing and tone mapping are owned by its view model", () => {
    const deliveryCard = readFileSync(resolve(process.cwd(), "src/components/messages/DeliverySummaryCard.tsx"), "utf8");
    const viewModel = readFileSync(resolve(process.cwd(), "src/components/messages/deliverySummaryPresentation.ts"), "utf8");

    expect(deliveryCard).toContain("deriveDeliverySummaryPresentation");
    expect(viewModel).toContain("parseSummary");
    expect(viewModel).toContain("messagePanelTone");
    expect(viewModel).toContain("deliveryTone");
    expect(deliveryCard).not.toContain("function parseSummary");
    expect(deliveryCard).not.toContain("function messagePanelTone");
  });

  test("delivery summary uses the shared motion and lightweight handoff material", () => {
    const deliveryCard = readFileSync(resolve(process.cwd(), "src/components/messages/DeliverySummaryCard.tsx"), "utf8");
    const globals = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

    expect(deliveryCard).toContain("data-forge-motion=\"delivery-card\"");
    expect(deliveryCard).toContain("useGSAP");
    expect(deliveryCard).toContain("forge-delivery-item, .forge-delivery-action");
    expect(globals).toContain(".forge-delivery-card .forge-message-panel-header");
    expect(globals).toContain("background: var(--forge-material-raised) !important");
    expect(globals).toContain("border-bottom-color: var(--forge-border-subtle)");
    expect(globals).toContain("color: var(--forge-text-primary)");
  });

  test("confirmation boundary rendering is owned by focused subviews", () => {
    const confirmCard = readFileSync(resolve(process.cwd(), "src/components/messages/ConfirmCard.tsx"), "utf8");
    const views = readFileSync(resolve(process.cwd(), "src/components/messages/ConfirmViews.tsx"), "utf8");

    expect(confirmCard).toContain("ConfirmBoundaryPendingView");
    expect(confirmCard).toContain("ConfirmBoundaryResolvedView");
    expect(confirmCard).toContain("ConfirmPromptView");
    expect(views).toContain("ConfirmActionBar");
    expect(views).toContain("forge-confirm-boundary-row");
    expect(views).toContain("confirm-resolved-summary");
    expect(views).not.toContain("permission-ticket-tag");
    expect(confirmCard).not.toContain("forge-confirm-boundary-row");
    expect(confirmCard).not.toContain("confirm-resolved-summary");
  });

  test("delivery summary items and action rendering are owned by focused subviews", () => {
    const deliveryCard = readFileSync(resolve(process.cwd(), "src/components/messages/DeliverySummaryCard.tsx"), "utf8");
    const views = readFileSync(resolve(process.cwd(), "src/components/messages/DeliverySummaryViews.tsx"), "utf8");

    expect(deliveryCard).toContain("DeliverySummaryItemView");
    expect(deliveryCard).toContain("DeliveryPrimaryAction");
    expect(views).toContain("delivery-summary-item");
    expect(views).toContain("delivery-primary-action");
    expect(views).toContain("primaryIcon");
    expect(deliveryCard).not.toContain("function SummaryItem");
    expect(deliveryCard).not.toContain("function primaryIcon");
  });

  test("reader caption copy actions are owned by a shared subview", () => {
    const codeBlock = readFileSync(resolve(process.cwd(), "src/components/messages/CodeBlock.tsx"), "utf8");
    const diagramBlock = readFileSync(resolve(process.cwd(), "src/components/messages/DiagramBlock.tsx"), "utf8");
    const action = readFileSync(resolve(process.cwd(), "src/components/messages/ReaderCaptionAction.tsx"), "utf8");

    expect(codeBlock).toContain("ReaderCaptionAction");
    expect(diagramBlock).toContain("ReaderCaptionAction");
    expect(action).toContain("forge-caption-action");
    expect(action).toContain("navigator.clipboard");
    expect(codeBlock).not.toContain("navigator.clipboard");
    expect(diagramBlock).not.toContain("navigator.clipboard");
  });

  test("code block metadata is owned by its presentation module", () => {
    const codeBlock = readFileSync(resolve(process.cwd(), "src/components/messages/CodeBlock.tsx"), "utf8");
    const presentation = readFileSync(resolve(process.cwd(), "src/components/messages/codeBlockPresentation.ts"), "utf8");

    expect(codeBlock).toContain("deriveCodeBlockView");
    expect(presentation).toContain("formatLanguageLabel");
    expect(presentation).toContain("cacheKey");
    expect(presentation).toContain("renderer");
    expect(codeBlock).not.toContain("function formatLanguageLabel");
  });

  test("diagram detection is owned by its presentation module", () => {
    const diagramBlock = readFileSync(resolve(process.cwd(), "src/components/messages/DiagramBlock.tsx"), "utf8");
    const markdownRenderer = readFileSync(resolve(process.cwd(), "src/components/messages/MarkdownRenderer.tsx"), "utf8");
    const presentation = readFileSync(resolve(process.cwd(), "src/components/messages/diagramPresentation.ts"), "utf8");

    expect(diagramBlock).toContain("deriveDiagramView");
    expect(markdownRenderer).toContain("@/components/messages/diagramPresentation");
    expect(presentation).toContain("shouldRenderDiagram");
    expect(presentation).toContain("looksLikeAsciiDiagram");
    expect(diagramBlock).not.toContain("looksLikeAsciiDiagram");
    expect(diagramBlock).not.toContain("DIAGRAM_LANGS");
  });

  test("file preview metadata is owned by its presentation module", () => {
    const sheet = readFileSync(resolve(process.cwd(), "src/components/messages/FilePreviewSheet.tsx"), "utf8");
    const presentation = readFileSync(resolve(process.cwd(), "src/components/messages/filePreviewPresentation.ts"), "utf8");

    expect(sheet).toContain("deriveFilePreviewView");
    expect(presentation).toContain("locationLabel");
    expect(presentation).toContain("copyText");
    expect(presentation).toContain("lineTone");
    expect(sheet).not.toContain("第 ${line} 行");
    expect(sheet).not.toContain("requested_line ?");
  });

  test("file preview body states are owned by a focused subview", () => {
    const sheet = readFileSync(resolve(process.cwd(), "src/components/messages/FilePreviewSheet.tsx"), "utf8");
    const body = readFileSync(resolve(process.cwd(), "src/components/messages/FilePreviewBody.tsx"), "utf8");

    expect(sheet).toContain("FilePreviewBody");
    expect(body).toContain("正在读取文件");
    expect(body).toContain("无法预览这个文件");
    expect(body).toContain("grid-cols-[64px_minmax(0,1fr)]");
    expect(body).not.toContain("border-l-2");
    expect(body).toContain("border-b");
    expect(body).toContain("last:border-b-0");
    expect(sheet).not.toContain("正在读取文件");
    expect(sheet).not.toContain("grid-cols-[64px_minmax(0,1fr)]");
  });

  test("file preview actions are owned by a focused subview", () => {
    const sheet = readFileSync(resolve(process.cwd(), "src/components/messages/FilePreviewSheet.tsx"), "utf8");
    const actions = readFileSync(resolve(process.cwd(), "src/components/messages/FilePreviewActions.tsx"), "utf8");

    expect(sheet).toContain("FilePreviewActions");
    expect(actions).toContain("navigator.clipboard");
    expect(actions).toContain("openFile");
    expect(actions).toContain("在编辑器打开");
    expect(sheet).not.toContain("navigator.clipboard");
    expect(sheet).not.toContain("在编辑器打开");
  });

  test("file preview references are owned by a tiny shared type module", () => {
    const types = readFileSync(resolve(process.cwd(), "src/components/messages/filePreviewTypes.ts"), "utf8");
    const sheet = readFileSync(resolve(process.cwd(), "src/components/messages/FilePreviewSheet.tsx"), "utf8");

    expect(types).toContain("export interface FileRef");
    expect(sheet).toContain("@/components/messages/filePreviewTypes");
    expect(sheet).not.toContain("export interface FileRef");

    for (const path of [
      "src/components/messages/DiffCard.tsx",
      "src/components/messages/TextBlock.tsx",
      "src/components/messages/UserMessage.tsx",
      "src/components/messages/MarkdownRenderer.tsx",
      "src/components/messages/markdownFileRefs.tsx",
    ]) {
      const source = readFileSync(resolve(process.cwd(), path), "utf8");
      expect(source).toContain("@/components/messages/filePreviewTypes");
      expect(source).not.toContain("@/components/messages/FilePreviewSheet\",");
    }
  });
});

test.describe("Desktop Empty Workspace Layout", () => {
  test("no-project empty workbench stays centered inside wide desktop windows", async ({ page }) => {
    await setup(page, { workingDir: null });
    await page.setViewportSize({ width: 1600, height: 1000 });
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });

    const main = page.getByRole("main");
    await expect(main.getByTestId("empty-workbench")).toBeVisible();
    await expect(main.getByTestId("empty-entry-new-tool")).toBeVisible();
    await expect(main.getByTestId("empty-entry-existing-project")).toBeVisible();
    await expect(main.getByTestId("empty-start-composer")).toHaveCount(0);

    const metrics = await main.evaluate((node) => {
      const mainEl = node as HTMLElement;
      const shell = mainEl.querySelector<HTMLElement>(".forge-empty-shell-centered");
      const workbench = mainEl.querySelector<HTMLElement>("[data-testid='empty-workbench']");
      const grid = mainEl.querySelector<HTMLElement>(".forge-empty-entry-grid");
      const notice = mainEl.querySelector<HTMLElement>("[data-testid='empty-workspace-notice']");
      const cards = Array.from(mainEl.querySelectorAll<HTMLElement>("[data-testid^='empty-entry-']"));
      if (!shell || !workbench || !grid || !notice || cards.length < 2) return null;

      const mainRect = mainEl.getBoundingClientRect();
      const workbenchRect = workbench.getBoundingClientRect();
      const gridRect = grid.getBoundingClientRect();
      const noticeRect = notice.getBoundingClientRect();
      const gridCenter = gridRect.left + gridRect.width / 2;
      const mainCenter = mainRect.left + mainRect.width / 2;

      return {
        shellOverflowX: getComputedStyle(shell).overflowX,
        workbenchWidth: Math.round(workbenchRect.width),
        gridWidth: Math.round(gridRect.width),
        mainWidth: Math.round(mainRect.width),
        gridLeft: Math.round(gridRect.left - mainRect.left),
        gridRightGap: Math.round(mainRect.right - gridRect.right),
        centerDelta: Math.round(Math.abs(gridCenter - mainCenter)),
        cardWidths: cards.map((card) => Math.round(card.getBoundingClientRect().width)),
        noticeLeftGap: Math.round(noticeRect.left - mainRect.left),
        noticeRightGap: Math.round(mainRect.right - noticeRect.right),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.shellOverflowX).toBe("hidden");
    expect(metrics!.workbenchWidth).toBeLessThanOrEqual(640);
    expect(metrics!.gridWidth).toBeLessThanOrEqual(metrics!.mainWidth - 48);
    expect(metrics!.gridLeft).toBeGreaterThanOrEqual(24);
    expect(metrics!.gridRightGap).toBeGreaterThanOrEqual(24);
    expect(metrics!.centerDelta).toBeLessThanOrEqual(2);
    expect(Math.abs(metrics!.cardWidths[0] - metrics!.cardWidths[1])).toBeLessThanOrEqual(1);
    expect(metrics!.noticeLeftGap).toBeGreaterThanOrEqual(24);
    expect(metrics!.noticeRightGap).toBeGreaterThanOrEqual(24);
  });

  test("project archive open keeps empty start choices readable on narrow desktop", async ({ page }) => {
    await setup(page, { workingDir: null });
    await page.setViewportSize({ width: 900, height: 720 });
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByTitle("打开项目档案").click();
    await expect(page.getByTestId("project-archive-panel")).toBeVisible();

    const metrics = await page.getByRole("main").evaluate((node) => {
      const main = node as HTMLElement;
      const archive = document.querySelector<HTMLElement>("[data-testid='project-archive-panel']");
      const grid = main.querySelector<HTMLElement>(".forge-empty-entry-grid");
      const cards = Array.from(main.querySelectorAll<HTMLElement>(".forge-empty-entry-card"));
      if (!archive || !grid || cards.length < 2) return null;
      const archiveRect = archive.getBoundingClientRect();
      const gridRect = grid.getBoundingClientRect();
      const cardRects = cards.map((card) => card.getBoundingClientRect());
      const leadDecoration = getComputedStyle(cards[0], "::before");
      return {
        mainPaddingRight: getComputedStyle(main).paddingRight,
        columnCount: getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length,
        gridRight: Math.round(gridRect.right),
        archiveLeft: Math.round(archiveRect.left),
        cardWidths: cardRects.map((rect) => Math.round(rect.width)),
        cardTops: cardRects.map((rect) => Math.round(rect.top)),
        leadDecorationContent: leadDecoration.content,
        leadDecorationWidth: leadDecoration.width,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.mainPaddingRight).toBe("300px");
    expect(metrics!.columnCount).toBe(1);
    expect(metrics!.gridRight).toBeLessThanOrEqual(metrics!.archiveLeft - 12);
    expect(metrics!.cardWidths.every((width) => width >= 280)).toBe(true);
    expect(metrics!.cardTops[1]).toBeGreaterThan(metrics!.cardTops[0]);
    expect(metrics!.leadDecorationContent).toBe("none");
    expect(metrics!.leadDecorationWidth).toBe("auto");
  });
});

test.describe("Timeline Message Flow", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
  });

  test("app loads and shows empty state", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(page.getByTestId("app-titlebar")).toHaveAttribute("data-tauri-drag-region", "true");
    await expect(page.getByTestId("app-titlebar")).toHaveCSS("height", "56px");
    await expect(main.getByTestId("empty-workbench")).toBeVisible();
    await expect(main.getByTestId("empty-workbench-project")).toContainText("forge");
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByTestId("empty-workbench-action")).toBeVisible();
    await expect(main.getByTestId("empty-entry-new-tool")).toBeVisible();
    await expect(main.getByTestId("empty-entry-existing-project")).toBeVisible();
    await expect(main.getByRole("button", { name: /做个新工具/ })).toBeVisible();
    await expect(main.getByRole("button", { name: /打开已有项目/ })).toBeVisible();
    const emptyMetrics = await main.evaluate((node) => {
      const workbench = node.querySelector<HTMLElement>("[data-testid='empty-workbench']");
      const frame = node.querySelector<HTMLElement>(".forge-empty-composer-frame");
      const composer = node.querySelector<HTMLElement>("[data-testid='empty-start-composer']");
      const project = node.querySelector<HTMLElement>("[data-testid='empty-workbench-project']");
      const action = node.querySelector<HTMLElement>("[data-testid='empty-workbench-action']");
      const style = workbench ? getComputedStyle(workbench) : null;
      const actionStyle = action ? getComputedStyle(action) : null;
      const nodeRect = (node as HTMLElement).getBoundingClientRect();
      const frameRect = frame?.getBoundingClientRect();
      return {
        borderWidth: style?.borderTopWidth ?? "",
        background: style?.backgroundColor ?? "",
        textAlign: style?.textAlign ?? "",
        composerWidth: composer ? Math.round(composer.getBoundingClientRect().width) : 0,
        frameBottomGap: frameRect ? Math.round(nodeRect.bottom - frameRect.bottom) : -1,
        frameTop: frameRect ? Math.round(frameRect.top - nodeRect.top) : 0,
        mainHeight: Math.round(nodeRect.height),
        projectHeight: project ? Math.round(project.getBoundingClientRect().height) : 0,
        projectRadius: project ? Number.parseFloat(getComputedStyle(project).borderTopLeftRadius) : 0,
        actionHeight: action ? Math.round(action.getBoundingClientRect().height) : 0,
        actionRadius: actionStyle ? Number.parseFloat(actionStyle.borderTopLeftRadius) : 0,
        actionDisplay: actionStyle?.display ?? "",
      };
    });
    expect(emptyMetrics.borderWidth).toBe("0px");
    expect(emptyMetrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(emptyMetrics.textAlign).toBe("left");
    expect(emptyMetrics.composerWidth).toBeGreaterThanOrEqual(520);
    expect(emptyMetrics.frameBottomGap).toBeLessThanOrEqual(1);
    expect(emptyMetrics.frameTop).toBeGreaterThan(emptyMetrics.mainHeight * 0.65);
    expect(emptyMetrics.projectHeight).toBe(26);
    expect(emptyMetrics.projectRadius).toBeLessThanOrEqual(8);
    expect(emptyMetrics.actionHeight).toBe(26);
    expect(emptyMetrics.actionRadius).toBeLessThanOrEqual(8);
    expect(["inline-flex", "flex"]).toContain(emptyMetrics.actionDisplay);
    const entryMetrics = await main.evaluate(() => {
      const cards = Array.from(document.querySelectorAll<HTMLElement>("[data-testid^='empty-entry-']"));
      return cards.map((card) => {
        const style = getComputedStyle(card);
        return {
          width: Math.round(card.getBoundingClientRect().width),
          height: Math.round(card.getBoundingClientRect().height),
          borderColor: style.borderTopColor,
          radius: Number.parseFloat(style.borderTopLeftRadius),
        };
      });
    });
    expect(entryMetrics).toHaveLength(2);
    expect(Math.abs(entryMetrics[0].width - entryMetrics[1].width)).toBeLessThanOrEqual(1);
    expect(Math.abs(entryMetrics[0].height - entryMetrics[1].height)).toBeLessThanOrEqual(12);
    expect(entryMetrics[0].borderColor).toBe(entryMetrics[1].borderColor);
    expect(entryMetrics[0].radius).toBeLessThanOrEqual(8);
    await expect(main.locator("img")).toHaveCount(0);
    await expect(main.locator("p", { hasText: "从当前对话开始" })).toHaveCount(0);
    await expect(main.getByText("Forge 会带着项目档案，把结果推进到可预览、可检查、可继续。")).toHaveCount(0);
    await expect(main.getByText("当前任务", { exact: true })).toHaveCount(0);
    await expect(main.getByText("交付", { exact: true })).toHaveCount(0);
    await expect(main.getByText("创建一个任务开始")).toHaveCount(0);
  });

  test("empty workbench entry cards tune the composer for beginners and existing projects", async ({ page }) => {
    const main = page.getByRole("main");
    const textbox = main.getByTestId("empty-start-composer").getByRole("textbox");

    await main.getByTestId("empty-entry-new-tool").click();
    await expect(textbox).toBeFocused();
    await expect(textbox).toHaveAttribute("placeholder", /记录喝水次数/);

    await main.getByTestId("empty-entry-existing-project").click();
    await expect(textbox).toBeFocused();
    await expect(textbox).toHaveAttribute("placeholder", /当前项目/);
  });

  test("empty workbench start controls read as compact desktop rails", async ({ page }) => {
    const main = page.getByRole("main");
    const entries = main.locator("[data-forge-motion='empty-entry']");
    await expect(entries).toHaveCount(2);
    await expect(main.locator("[data-forge-motion='empty-composer']")).toBeVisible();
    await expect(main.locator("[data-forge-motion='empty-context']")).toBeVisible();

    const metrics = await main.evaluate((node) => {
      const cards = Array.from(node.querySelectorAll<HTMLElement>("[data-forge-motion='empty-entry']"));
      const composer = node.querySelector<HTMLElement>("[data-forge-motion='empty-composer']");
      if (!composer || cards.length < 2) return null;
      return {
        cardHeights: cards.map((card) => Math.round(card.getBoundingClientRect().height)),
        cardDisplays: cards.map((card) => getComputedStyle(card).display),
        cardAlignments: cards.map((card) => getComputedStyle(card).alignItems),
        cardShadows: cards.map((card) => getComputedStyle(card).boxShadow),
        composerWidth: Math.round(composer.getBoundingClientRect().width),
      };
    });

    expect(metrics).not.toBeNull();
    expect(Math.max(...metrics!.cardHeights)).toBeLessThanOrEqual(82);
    expect(metrics!.cardDisplays.every((display) => display === "flex")).toBe(true);
    expect(metrics!.cardAlignments.every((alignment) => alignment === "center")).toBe(true);
    expect(metrics!.cardShadows.every((shadow) => shadow === "none")).toBe(true);
    expect(metrics!.composerWidth).toBeGreaterThanOrEqual(520);
  });

  test("empty workbench primary action starts a conversation", async ({ page }) => {
    await page.getByRole("main").getByRole("button", { name: "开始新对话" }).click();

    await expect(page.locator("textarea")).toBeVisible();
    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    expect(createArgs.workingDir).toBe("/Users/cabbos/project/forge");
  });

  test("empty workbench shows start readiness before the first conversation", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockApiKeyStatus = [
        { provider: "deepseek", set: false, preview: "" },
      ];
    });
    await page.goto("http://localhost:1420");

    const main = page.getByRole("main");
    const readiness = main.getByTestId("start-readiness");
    await expect(readiness).toBeVisible();
    await expect(readiness.getByText("需要配置模型密钥")).toBeVisible();
    await expect(readiness.getByText("还没有配置 DeepSeek", { exact: true })).toBeVisible();
    await readiness.getByRole("button", { name: "打开设置" }).first().click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  });

  test("empty workbench does not duplicate readiness when start is ready", async ({ page }) => {
    const main = page.getByRole("main");
    await expect(main.getByText("准备开始", { exact: true })).toHaveCount(0);
    await expect(main.getByTestId("start-readiness")).toHaveCount(0);
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByRole("button", { name: "开始新对话" })).toBeVisible();
  });

  test("empty workbench can start directly from a prompt", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");

    const composer = page.getByTestId("empty-start-composer");
    await composer.getByRole("textbox").fill("做一个可以记录收支的小工具");
    await composer.getByRole("textbox").press("Enter");

    await expect(page.getByTestId("user-message").last()).toContainText("做一个可以记录收支的小工具");
    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    const checkpointArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateProjectCheckpointArgs;
    });
    const sendArgs = await expectLastSendInputArgs(page, { sessionId });
    const sentText = String(sendArgs.text);
    expect(createArgs.workingDir).toBe("/Users/cabbos/project/forge");
    expect(checkpointArgs.sessionId).toBe(sessionId);
    expect(checkpointArgs.workingDir).toBe("/Users/cabbos/project/forge");
    expect(sentText).toContain("Forge 第一闭环提示");
    expect(sentText).toContain("当前工作空间：/Users/cabbos/project/forge");
    expect(sentText).toContain("所有文件搜索、修改、预览、检查点和验证都必须限定在当前工作空间。");
    expect(sentText).toContain("如果预览端口来自其他项目，必须提示冲突，不要打开别的项目。");
    expect(sentText).toContain("本地网页小工具");
    expect(sentText).toContain("React/Vite");
    expect(sentText).toContain("少问问题");
    expect(sentText).toContain("做一个可以记录收支的小工具");
  });

  test("empty workbench composer send uses the same compact ready material", async ({ page }) => {
    await page.goto("http://localhost:1420");

    const composer = page.getByTestId("empty-start-composer");
    const send = composer.getByRole("button", { name: "发送并开始" });
    await expect(send).toBeDisabled();

    const disabledMetrics = await send.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        cursor: style.cursor,
        opacity: style.opacity,
      };
    });
    expect(disabledMetrics.cursor).toBe("default");
    expect(Number.parseFloat(disabledMetrics.opacity)).toBeLessThan(1);

    await composer.getByRole("textbox").fill("继续优化当前页面体验");
    await expect(send).toBeEnabled();
    await expect(send).toHaveAttribute("data-ready", "true");
    await expect.poll(async () => send.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        borderReady: style.borderTopColor !== "rgba(0, 0, 0, 0)",
        shadowReady: style.boxShadow !== "none",
      };
    })).toEqual({ borderReady: true, shadowReady: true });

    const readyMetrics = await send.evaluate((node) => {
      const style = getComputedStyle(node);
      const composer = node.closest("[data-testid='empty-start-composer']");
      const input = composer?.querySelector<HTMLElement>(".forge-empty-composer-input");
      return {
        width: Math.round(node.getBoundingClientRect().width),
        height: Math.round(node.getBoundingClientRect().height),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        boxShadow: style.boxShadow,
        radius: Number.parseFloat(style.borderTopLeftRadius),
        inputMinHeight: input ? Number.parseFloat(getComputedStyle(input).minHeight) : 0,
      };
    });

    expect(readyMetrics.width).toBe(32);
    expect(readyMetrics.height).toBe(32);
    expect(readyMetrics.background).not.toBe("rgb(184, 138, 86)");
    expect(readyMetrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(readyMetrics.boxShadow).not.toBe("none");
    expect(readyMetrics.radius).toBeLessThanOrEqual(8);
    expect(readyMetrics.inputMinHeight).toBeGreaterThanOrEqual(88);
  });

  test("empty workbench stays grounded in short desktop windows", async ({ page }) => {
    await page.setViewportSize({ width: 1024, height: 520 });
    await page.goto("http://localhost:1420");

    const main = page.getByRole("main");
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByTestId("empty-middle-hints")).toBeHidden();

    const metrics = await main.evaluate((node) => {
      const mainRect = (node as HTMLElement).getBoundingClientRect();
      const frame = node.querySelector<HTMLElement>(".forge-empty-composer-frame");
      const composer = node.querySelector<HTMLElement>("[data-testid='empty-start-composer']");
      const input = node.querySelector<HTMLElement>(".forge-empty-composer-input");
      const frameRect = frame?.getBoundingClientRect();
      const composerRect = composer?.getBoundingClientRect();
      return {
        composerBottomGap: frameRect ? Math.round(mainRect.bottom - frameRect.bottom) : -1,
        composerTop: composerRect ? Math.round(composerRect.top - mainRect.top) : 0,
        mainHeight: Math.round(mainRect.height),
        inputMinHeight: input ? Math.round(Number.parseFloat(getComputedStyle(input).minHeight)) : 0,
      };
    });

    expect(metrics.composerBottomGap).toBeLessThanOrEqual(1);
    expect(metrics.composerTop).toBeGreaterThan(metrics.mainHeight * 0.5);
    expect(metrics.inputMinHeight).toBeLessThanOrEqual(64);
  });

  test("vague beginner idea is shaped before making", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");

    const composer = page.getByTestId("empty-start-composer");
    await composer.getByRole("textbox").fill("我想做个能记录客户的东西，最好能提醒我，还能导出表格，但我也不知道怎么说。");
    await composer.getByRole("textbox").press("Enter");

    const sendArgs = await expectLastSendInputArgs(page, { sessionId });
    const sentText = String(sendArgs.text);
    expect(sentText).toContain("Forge 需求梳理提示");
    expect(sentText).toContain("只问一个轻确认问题");
    expect(sentText).toContain("先不做");
    expect(sentText).not.toContain("请优先推进到一个可预览的第一版");
  });

  test("empty workbench hints fill the bottom composer without sending", async ({ page }) => {
    const main = page.getByRole("main");
    const hints = main.getByTestId("empty-middle-hints");
    await expect(hints).toBeVisible();
    await hints.getByRole("button", { name: "检查这个项目能不能运行" }).click();

    await expect(main.getByTestId("empty-start-composer").getByRole("textbox")).toHaveValue("检查这个项目能不能运行");
    await expectNoSendInput(page);
  });

  test("creating a session shows chat input", async ({ page }) => {
    await page.goto("http://localhost:1420");
    // Click new session button
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    // Input should appear
    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByRole("main").getByText("运行中", { exact: true })).toHaveCount(0);
    const composer = page.getByTestId("composer-lane");
    await expect(composer).toBeVisible();
    await expect(composer.getByRole("button", { name: "引用文件" })).toBeVisible();
    await expect(composer.getByRole("button", { name: "常用请求" })).toBeVisible();
    await expect(page.getByRole("button", { name: "我想做一个番茄钟小工具，可以开始、暂停、重置。" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "我想做一个记账小工具，先能记录一笔收入或支出。" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "我想做一个文案小工具，输入主题后生成一版短文案。" })).toHaveCount(0);
    await expect(page.getByRole("main").getByText("可以继续描述任务")).toHaveCount(0);
  });

  test("desktop chrome keeps the conversation surface restrained", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const titlebar = page.getByTestId("app-titlebar");
    await expect(titlebar).toHaveCSS("height", "56px");

    const composerFrame = page.getByTestId("composer-frame");
    await expect(composerFrame).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
    await expect(composerFrame).toHaveCSS("backdrop-filter", "none");
    await expect(page.getByTestId("composer-surface")).not.toHaveCSS("box-shadow", "none");

    await simulateStream(page, sessionId, fullConversation(sessionId), 10);
    const processSummary = page.getByTestId("tool-activity-summary").first();
    await expect(processSummary).toContainText("过程已收起 · 2 步");
    await expect(processSummary).toHaveCSS("min-height", "22px");
  });

  test("V3 operating surface keeps conversation focused without the Inspector rail", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const shell = page.getByTestId("operating-surface");
    await expect(shell).toHaveAttribute("data-design-version", "v3-light-workbench");

    const tokens = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      const parseHex = (hex: string) => {
        const value = hex.trim().replace("#", "");
        return {
          r: Number.parseInt(value.slice(0, 2), 16),
          g: Number.parseInt(value.slice(2, 4), 16),
          b: Number.parseInt(value.slice(4, 6), 16),
        };
      };
      const luminance = (color: { r: number; g: number; b: number }) => {
        const [r, g, b] = [color.r, color.g, color.b].map((channel) => {
          const value = channel / 255;
          return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
      };
      const contrast = (foreground: string, background: string) => {
        const fg = luminance(parseHex(foreground));
        const bg = luminance(parseHex(background));
        return (Math.max(fg, bg) + 0.05) / (Math.min(fg, bg) + 0.05);
      };
      const base = root.getPropertyValue("--forge-bg-base").trim();
      const depth = root.getPropertyValue("--forge-bg-depth").trim();
      const raised = root.getPropertyValue("--forge-bg-raised").trim();
      const muted = root.getPropertyValue("--forge-text-muted").trim();
      const faint = root.getPropertyValue("--forge-text-faint").trim();
      return {
        base,
        ink: root.getPropertyValue("--forge-ink").trim(),
        brass: root.getPropertyValue("--forge-accent").trim(),
        muted,
        faint,
        mutedOnBase: contrast(muted, base),
        mutedOnDepth: contrast(muted, depth),
        faintOnBase: contrast(faint, base),
        faintOnDepth: contrast(faint, depth),
        faintOnRaised: contrast(faint, raised),
      };
    });
    expect(tokens.base).toBe("#F7F1E8");
    expect(tokens.ink).toBe("#242A24");
    expect(tokens.brass).toBe("#C48A3A");
    expect(tokens.mutedOnBase).toBeGreaterThanOrEqual(4.5);
    expect(tokens.mutedOnDepth).toBeGreaterThanOrEqual(4.5);
    expect(tokens.faintOnBase).toBeGreaterThanOrEqual(4.5);
    expect(tokens.faintOnDepth).toBeGreaterThanOrEqual(4.5);
    expect(tokens.faintOnRaised).toBeGreaterThanOrEqual(4.5);

    await expect(page.getByTestId("project-cockpit")).toHaveCount(0);
    await expect(page.getByRole("complementary", { name: "Inspector" })).toHaveCount(0);
    await expect(page.getByTestId("message-lane")).toHaveAttribute("data-surface", "conversation");
    await expect(page.getByTestId("composer-frame")).toHaveAttribute("data-surface", "composer");

    const layout = await page.evaluate(() => {
      const shell = document.querySelector<HTMLElement>("[data-testid='operating-surface']");
      const main = document.querySelector<HTMLElement>("[data-testid='main-workbench']");
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      if (!shell || !main || !lane) return null;
      return {
        columns: getComputedStyle(shell).gridTemplateColumns,
        mainRight: Math.round(main.getBoundingClientRect().right),
        viewportRight: Math.round(window.innerWidth),
        laneWidth: Math.round(lane.getBoundingClientRect().width),
      };
    });
    expect(layout).not.toBeNull();
    expect(layout!.columns).not.toContain("320px");
    expect(layout!.mainRight).toBe(layout!.viewportRight);
    expect(layout!.laneWidth).toBeLessThanOrEqual(820);

    const decorativeSurfaces = await page.evaluate(() => {
      const scroll = document.querySelector("[data-testid='conversation-scroll']");
      const operatingLane = document.querySelector(".forge-operating-lane");
      return {
        scrollTexture: scroll ? getComputedStyle(scroll, "::after").backgroundImage : "",
        operatingRail: operatingLane ? getComputedStyle(operatingLane, "::before").content : "",
      };
    });
    expect(decorativeSurfaces.scrollTexture).toBe("none");
    expect(decorativeSurfaces.operatingRail).toBe("none");

    await simulateStream(page, sessionId, fullConversation(sessionId), 10);
    await expect(page.getByTestId("tool-activity-summary").first()).toContainText("过程已收起 · 2 步");
    await expect(page.getByRole("complementary", { name: "Inspector" })).toHaveCount(0);
    const turnDecoration = await page.getByTestId("conversation-scroll").evaluate(() => {
      const turn = document.querySelector(".forge-conversation-turn");
      if (!turn) return null;
      const turnStyle = getComputedStyle(turn);
      return {
        bead: getComputedStyle(turn, "::after").content,
        borderLeft: turnStyle.borderLeftWidth,
      };
    });
    expect(turnDecoration).not.toBeNull();
    expect(turnDecoration!.bead).toBe("none");
    expect(turnDecoration!.borderLeft).toBe("0px");

    const assistantDecoration = await page.getByTestId("assistant-message").first().evaluate((node) => {
      const before = getComputedStyle(node, "::before");
      return {
        content: before.content,
        width: before.width,
        background: before.backgroundColor,
      };
    });
    expect(assistantDecoration.content).toBe("none");
    expect(assistantDecoration.width).toBe("auto");
  });

  test("titlebar presents session and project state as a compact desktop status bar", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await holdSendInput(page);
    await page.locator("textarea").fill("Titlebar polish status");
    await page.locator("textarea").press("Enter");
    await expectHeldSendInput(page, "Titlebar polish status");

    const titlebar = page.getByTestId("app-titlebar");
    await expect(titlebar.getByTestId("titlebar-title")).toContainText("Titlebar polish status");
    await expect(titlebar.getByTestId("titlebar-project-boundary")).toContainText("forge");
    await expect(titlebar.getByTestId("titlebar-status-pill")).toContainText("响应中");
    await expect(titlebar.getByTestId("titlebar-status-pill")).toHaveAttribute("data-state", "running");
    await expect(titlebar.getByTestId("titlebar-actions")).toBeVisible();

    const metrics = await titlebar.evaluate((node) => {
      const title = node.querySelector<HTMLElement>("[data-testid='titlebar-title']");
      const project = node.querySelector<HTMLElement>("[data-testid='titlebar-project-boundary']");
      const status = node.querySelector<HTMLElement>("[data-testid='titlebar-status-pill']");
      const actions = node.querySelector<HTMLElement>("[data-testid='titlebar-actions']");
      const buttons = Array.from(node.querySelectorAll<HTMLElement>(".forge-titlebar-button"));
      if (!title || !project || !status || !actions) return null;
      const statusStyle = getComputedStyle(status);
      const projectStyle = getComputedStyle(project);
      const actionsStyle = getComputedStyle(actions);
      return {
        titlebarHeight: Math.round(node.getBoundingClientRect().height),
        contextLeft: Math.round(title.getBoundingClientRect().left - node.getBoundingClientRect().left),
        actionsRightGap: Math.round(node.getBoundingClientRect().right - actions.getBoundingClientRect().right),
        titleLineHeight: Math.round(Number.parseFloat(getComputedStyle(title).lineHeight)),
        projectHeight: Math.round(project.getBoundingClientRect().height),
        projectTopGap: Math.round(project.getBoundingClientRect().top - title.getBoundingClientRect().bottom),
        projectBackground: projectStyle.backgroundColor,
        projectBorder: projectStyle.borderTopColor,
        statusHeight: Math.round(status.getBoundingClientRect().height),
        statusRadius: Number.parseFloat(statusStyle.borderTopLeftRadius),
        statusBackground: statusStyle.backgroundColor,
        actionsGap: Math.round(Number.parseFloat(actionsStyle.columnGap)),
        buttonSizes: buttons.map((button) => ({
          width: Math.round(button.getBoundingClientRect().width),
          height: Math.round(button.getBoundingClientRect().height),
        })),
        buttonTransitions: buttons.map((button) => getComputedStyle(button).transitionProperty),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.titlebarHeight).toBe(56);
    expect(metrics!.contextLeft).toBeGreaterThanOrEqual(24);
    expect(metrics!.actionsRightGap).toBeGreaterThanOrEqual(18);
    expect(metrics!.titleLineHeight).toBeLessThanOrEqual(20);
    expect(metrics!.projectHeight).toBe(22);
    expect(metrics!.projectTopGap).toBeGreaterThanOrEqual(4);
    expect(metrics!.projectBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.projectBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.statusHeight).toBeLessThanOrEqual(20);
    expect(metrics!.statusRadius).toBeLessThanOrEqual(8);
    expect(metrics!.statusBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.actionsGap).toBe(4);
    expect(metrics!.buttonSizes).toEqual([{ width: 28, height: 28 }, { width: 28, height: 28 }]);
    expect(metrics!.buttonTransitions.every((value) => value.includes("box-shadow"))).toBe(true);

    const searchButton = titlebar.getByRole("button", { name: "搜索" });
    await searchButton.hover();
    await expect(searchButton).not.toHaveCSS("box-shadow", "none");
    await releaseHeldSendInput(page);
  });

  test("reduced motion keeps running chrome steady", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await holdSendInput(page);
    await page.locator("textarea").fill("Reduced motion status");
    await page.locator("textarea").press("Enter");
    await expectHeldSendInput(page, "Reduced motion status");

    const metrics = await page.evaluate(() => {
      const statusDot = document.querySelector<HTMLElement>(".forge-titlebar-status-dot");
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const dotStyle = statusDot ? getComputedStyle(statusDot) : null;
      const composerStyle = composer ? getComputedStyle(composer) : null;
      return {
        dotAnimationName: dotStyle?.animationName ?? "",
        dotAnimationDuration: dotStyle?.animationDuration ?? "",
        composerTransitionDuration: composerStyle?.transitionDuration ?? "",
      };
    });

    expect(metrics.dotAnimationName).toBe("none");
    expect(metrics.dotAnimationDuration).toBe("0s");
    expect(metrics.composerTransitionDuration.split(", ").every((duration) => duration === "0s")).toBe(true);
    await releaseHeldSendInput(page);
  });

  test("start readiness stays compact in an empty session", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const readiness = page.getByTestId("start-readiness-panel");
    await expect(readiness).toBeVisible();
    await expect(readiness).toHaveCSS("border-radius", "8px");
    await expect(readiness.getByTestId("start-readiness-row")).toHaveCount(0);
    await expect(readiness.getByText("当前项目", { exact: true })).toHaveCount(0);
    await expect(readiness.getByText("模型密钥", { exact: true })).toHaveCount(0);
    await expect(readiness.getByText("预览", { exact: true })).toHaveCount(0);
    await expect(readiness.getByText("检查点", { exact: true })).toHaveCount(0);
    await expect(readiness.getByRole("button", { name: "刷新准备状态" })).toBeVisible();
  });

  test("missing API key is shown as an actionable setup card", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      window.__mockMissingApiKey = true;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await expect(page.getByText("需要配置模型密钥")).toBeVisible();
    await expect(page.getByText("需要配置模型密钥")).toHaveCount(1);
    const setupPanel = page.getByTestId("message-panel").filter({ hasText: "需要配置模型密钥" });
    await expect(setupPanel).toHaveAttribute("role", "status");
    await expect(setupPanel.getByTestId("missing-api-key-card")).toBeVisible();
    const setupMetrics = await setupPanel.evaluate((node) => {
      const body = node.querySelector<HTMLElement>("[data-testid='missing-api-key-card']");
      const action = node.querySelector<HTMLElement>("[data-testid='missing-api-key-action']");
      const style = getComputedStyle(node);
      const actionStyle = action ? getComputedStyle(action) : null;
      return {
        width: Math.round(node.getBoundingClientRect().width),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        border: style.borderTopColor,
        bodyHeight: body ? Math.round(body.getBoundingClientRect().height) : 0,
        actionHeight: action ? Math.round(action.getBoundingClientRect().height) : 0,
        actionRadius: action ? Number.parseFloat(getComputedStyle(action).borderTopLeftRadius) : 0,
        actionBackground: actionStyle?.backgroundColor ?? "",
        actionBorder: actionStyle?.borderTopColor ?? "",
        actionShadow: actionStyle?.boxShadow ?? "",
      };
    });
    expect(setupMetrics.width).toBeLessThanOrEqual(620);
    expect(setupMetrics.radius).toBeLessThanOrEqual(8);
    expect(setupMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(setupMetrics.border).not.toBe("rgba(0, 0, 0, 0)");
    expect(setupMetrics.bodyHeight).toBeLessThanOrEqual(38);
    expect(setupMetrics.actionHeight).toBe(28);
    expect(setupMetrics.actionRadius).toBeLessThanOrEqual(8);
    expect(setupMetrics.actionBackground).not.toBe("rgb(184, 138, 86)");
    expect(setupMetrics.actionBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(setupMetrics.actionShadow).not.toBe("none");
    await page.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "模型服务" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "本机数据" })).toBeVisible();
    await expect(page.getByText("API Key")).toHaveCount(0);
    await expect(page.getByText("~/.forge/config.json")).toHaveCount(0);
    await expect(page.getByText("IndexedDB")).toHaveCount(0);
  });

  test("session creation errors stay inline and can open settings", async ({ page }) => {
    const dialogs: string[] = [];
    page.on("dialog", async (dialog) => {
      dialogs.push(dialog.message());
      await dialog.dismiss();
    });

    await page.evaluate(() => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "create_session") {
          throw new Error("No DeepSeek API key configured. Open Settings (Cmd+,) to set one.");
        }
        return original?.(cmd, args);
      };
    });

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("status")).toContainText("模型服务还没有可用密钥");
    expect(dialogs).toEqual([]);

    await sidebar.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  });

  test("settings show provider defaults and context window quietly", async ({ page }) => {
    await page.getByRole("button", { name: "设置" }).click();

    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "模型服务" })).toBeVisible();
    await expect(dialog.getByTestId("settings-preferences-panel")).toBeVisible();
    const providerRows = dialog.getByTestId("settings-provider-row");
    await expect(providerRows.first()).toBeVisible();
    expect(await providerRows.count()).toBeGreaterThanOrEqual(1);
    const settingsMetrics = await dialog.evaluate((node) => {
      const panel = node.querySelector<HTMLElement>("[data-testid='settings-preferences-panel']");
      const rows = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='settings-provider-row']"));
      const status = node.querySelector<HTMLElement>("[data-testid='settings-provider-status']");
      const panelStyle = panel ? getComputedStyle(panel) : null;
      const rowStyle = rows[0] ? getComputedStyle(rows[0]) : null;
      const secondRowStyle = rows[1] ? getComputedStyle(rows[1]) : null;
      return {
        panelRadius: panelStyle ? Number.parseFloat(panelStyle.borderTopLeftRadius) : 0,
        panelDisplay: panelStyle?.display ?? "",
        panelGap: panelStyle?.rowGap ?? "",
        panelOverflow: panelStyle?.overflow ?? "",
        firstRowHeight: rows[0] ? Math.round(rows[0].getBoundingClientRect().height) : 0,
        firstRowDisplay: rowStyle?.display ?? "",
        firstRowBackground: rowStyle?.backgroundColor ?? "",
        firstRowBorderTop: rowStyle?.borderTopColor ?? "",
        secondRowBorderTop: secondRowStyle?.borderTopColor ?? "",
        firstRowTransition: rowStyle?.transitionProperty ?? "",
        statusRadius: status ? Number.parseFloat(getComputedStyle(status).borderTopLeftRadius) : 0,
        statusBoxShadow: status ? getComputedStyle(status).boxShadow : "",
      };
    });
    expect(settingsMetrics.panelRadius).toBeLessThanOrEqual(8);
    expect(settingsMetrics.panelDisplay).toBe("grid");
    expect(settingsMetrics.panelGap).toBe("8px");
    expect(settingsMetrics.panelOverflow).toBe("visible");
    expect(settingsMetrics.firstRowHeight).toBeGreaterThanOrEqual(64);
    expect(settingsMetrics.firstRowDisplay).toBe("grid");
    expect(settingsMetrics.firstRowBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(settingsMetrics.firstRowBorderTop).toBe("rgba(0, 0, 0, 0)");
    expect(settingsMetrics.secondRowBorderTop).toBe("rgba(0, 0, 0, 0)");
    expect(settingsMetrics.firstRowTransition.includes("box-shadow")).toBe(true);
    expect(settingsMetrics.statusRadius).toBeLessThanOrEqual(8);
    expect(settingsMetrics.statusBoxShadow).not.toBe("none");
    await providerRows.first().hover();
    await expect(providerRows.first()).not.toHaveCSS("box-shadow", "none");
    const deepseek = dialog.locator("section").filter({ hasText: "DeepSeek" });
    await expect(deepseek.getByText("DeepSeek V4 Flash 1M")).toBeVisible();
    await expect(deepseek.getByText("默认模型 · 上下文 1M")).toBeVisible();
    await expect(deepseek.getByText("deepseek-v4-flash[1m]")).toHaveCount(0);
  });

  test("internal skill context is not rendered in the conversation", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "internal-skills" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "internal-skills",
        content: "## Active Skills\n\n- code-review\n- browser",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "internal-skills" },
    ], 5);

    await expect(page.getByRole("main").getByText("Active Skills")).toHaveCount(0);
    await expect(page.getByRole("main").getByText("code-review")).toHaveCount(0);
  });

  test("long assistant replies do not add an automatic acceptance checklist", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "long-answer" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "long-answer",
        content: "我已经把第一版方向整理好了。这个版本会先保留一个核心界面，一个主要交互，以及一个清楚的下一步。用户可以直接继续描述想改哪里，Forge 会在当前项目里继续推进，而不是让用户管理一堆流程提示。这里还会补充当前版本包含什么、暂时不包含什么、为什么先从最小可用版本开始，以及如果预览失败应该优先检查哪个地方。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "long-answer" },
    ], 5);

    const main = page.getByRole("main");
    await expect(main.getByText("验收清单", { exact: true })).toHaveCount(0);
    await expect(main.getByText("下一步提示词", { exact: true })).toHaveCount(0);
  });

  test("restores the active conversation after reload", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.evaluate(async (sessionId) => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash",
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put([
        {
          block_id: "seed-user-message",
          event_type: "user_message",
          content: "已有对话内容",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, sessionId);

    await page.reload();

    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByRole("main").getByText("已有对话内容").last()).toBeVisible();
    await expect(page.getByRole("main").getByText("从当前任务开始")).toHaveCount(0);
  });

  test("stopped composer presents a quiet resume state", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.evaluate(async (sessionId) => {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash",
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put([
        {
          block_id: "seed-user-message",
          event_type: "user_message",
          content: "已有对话内容",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, sessionId);

    await page.reload();

    const composer = page.getByTestId("composer-surface");
    await expect(composer).toHaveAttribute("data-state", "paused");
    await expect(page.locator("textarea")).toBeDisabled();
    await expect(page.locator("textarea")).toHaveAttribute("placeholder", "这个会话已停止，可以继续后再发送");
    await expect(page.getByRole("button", { name: "继续会话" })).toBeVisible();

    const metrics = await composer.evaluate((node) => {
      const surface = node as HTMLElement;
      const textarea = surface.querySelector<HTMLTextAreaElement>(".forge-composer-textarea");
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const resume = surface.querySelector<HTMLElement>(".forge-composer-resume");
      const surfaceStyle = getComputedStyle(surface);
      const textareaStyle = textarea ? getComputedStyle(textarea) : null;
      const resumeStyle = resume ? getComputedStyle(resume) : null;
      return {
        background: surfaceStyle.backgroundColor,
        borderColor: surfaceStyle.borderTopColor,
        textareaMinHeight: textareaStyle ? Math.round(Number.parseFloat(textareaStyle.minHeight)) : 0,
        textareaCursor: textareaStyle?.cursor ?? "",
        toolbarHeight: toolbar ? Math.round(toolbar.getBoundingClientRect().height) : 0,
        resumeHeight: resume ? Math.round(resume.getBoundingClientRect().height) : 0,
        resumeRadius: resumeStyle ? Number.parseFloat(resumeStyle.borderTopLeftRadius) : 0,
        resumeShadow: resumeStyle?.boxShadow ?? "",
      };
    });

    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.textareaMinHeight).toBeLessThanOrEqual(36);
    expect(metrics.textareaCursor).toBe("default");
    expect(metrics.toolbarHeight).toBeLessThanOrEqual(36);
    expect(metrics.resumeHeight).toBe(32);
    expect(metrics.resumeRadius).toBeLessThanOrEqual(8);
    expect(metrics.resumeShadow).not.toBe("none");
  });

  test("resume does not duplicate persisted delivery summary blocks", async ({ page }) => {
    const sessionId = "legacy-delivery-resume";
    const projectPath = "/Users/cabbos/project/forge";
    const summary = {
      project_path: projectPath,
      preview_label: "预览未运行",
      checkpoint_label: "检查点已就绪",
      next_action: "下一步：交付状态可以继续验收。",
    };
    await setup(page);
    await page.addInitScript((summary) => {
      // @ts-expect-error mock
      window.__mockResumeDeliverySummary = summary;
    }, summary);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, projectPath, summary }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: projectPath, name: "forge", path: projectPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(projectPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: projectPath,
          workspaceId: projectPath,
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "legacy-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: { summary },
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, projectPath, summary });

    await page.reload();
    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(1);
    await page.getByRole("button", { name: "继续会话" }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error mock
      return window.__lastResumedSessionId === "legacy-delivery-resume";
    });
    await page.waitForFunction(() => {
      // @ts-expect-error mock
      return window.__resumeDeliveryEmitted === true;
    });

    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(1);
  });

  test("same delivery summary after a new user message appends a new card", async ({ page }) => {
    const sessionId = "same-summary-new-turn";
    const projectPath = "/Users/cabbos/project/forge";
    const summary = {
      project_path: projectPath,
      preview_label: "预览未运行",
      checkpoint_label: "检查点已就绪",
      next_action: "下一步：交付状态可以继续验收。",
    };
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, projectPath, summary }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: projectPath, name: "forge", path: projectPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(projectPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: projectPath,
          workspaceId: projectPath,
          contextWindowTokens: 1_000_000,
          status: "running",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "first-turn-user",
          event_type: "user_message",
          content: "第一轮",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "first-turn-delivery",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: { summary },
        },
        {
          block_id: "second-turn-user",
          event_type: "user_message",
          content: "第二轮",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, projectPath, summary });

    await page.reload();
    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(1);
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "second-turn-delivery",
        summary,
      },
    ], 1);

    await expect(page.getByTestId("message-panel").filter({ hasText: "本轮交付" })).toHaveCount(2);
  });

  test("timeline messages render correctly", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    // Create session
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    // Simulate a full conversation
    const events = fullConversation(sessionId);
    await simulateStream(page, sessionId, events, 30);

    await expect(page.getByRole("button", { name: /思考已收起/ })).toBeVisible({ timeout: 5000 });
    await expect(page.getByText("I'll create a fibonacci function.")).toBeVisible();

    const processSummary = page.getByTestId("tool-activity-summary");
    await expect(processSummary).toBeVisible({ timeout: 5000 });
    await expect(processSummary).toContainText("过程已收起 · 2 步");
    await processSummary.click();

    // Tool card should show write_to_file after expanding handled work.
    await expect(page.locator("text=write_to_file")).toBeVisible({ timeout: 5000 });

    // Shell card should show terminal output
    await expect(page.locator("text=python test.py")).toBeVisible();

    // Final text should be visible
    await expect(page.locator("text=The fibonacci function works correctly")).toBeVisible();
  });

  test("structured conversation blocks stay compact while collapsed", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, fullConversation(sessionId), 10);

    const thinkingTrigger = page.getByTestId("thinking-trigger").first();
    const processSummary = page.getByTestId("tool-activity-summary").first();
    await expect(thinkingTrigger).toBeVisible();
    await expect(processSummary).toBeVisible();
    await expect(processSummary).toHaveAttribute("aria-expanded", "false");

    const widths = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']")?.getBoundingClientRect();
      const process = document.querySelector("[data-testid='tool-activity-summary']")?.getBoundingClientRect();
      const processNode = document.querySelector<HTMLElement>("[data-testid='tool-activity-summary']");
      const processStyle = processNode ? getComputedStyle(processNode) : null;
      return thinking && process
        ? {
          thinking: Math.round(thinking.width),
          process: Math.round(process.width),
          processBorderTop: processStyle ? Math.round(Number.parseFloat(processStyle.borderTopWidth)) : -1,
          processBackground: processStyle?.backgroundColor ?? "",
        }
        : null;
    });
    expect(widths).not.toBeNull();
    expect(widths!.thinking).toBeLessThanOrEqual(220);
    expect(widths!.process).toBeLessThanOrEqual(520);
    await expect(thinkingTrigger).toHaveCSS("border-top-width", "0px");
    expect(widths!.processBorderTop).toBe(0);
    expect(widths!.processBackground).toBe("rgba(0, 0, 0, 0)");

    await processSummary.click();
    const toolTrigger = page.getByTestId("tool-card-trigger").first();
    const shellTrigger = page.getByTestId("shell-card-trigger").first();
    await expect(toolTrigger).toBeVisible();
    await expect(shellTrigger).toBeVisible();
    await toolTrigger.click();
    await expect(page.getByRole("button", { name: "复制工具输出" }).first()).toBeVisible();
    await shellTrigger.click();
    await expect(page.getByRole("button", { name: "复制命令输出" }).first()).toBeVisible();
  });

  test("tool-heavy turns render as a quiet desktop work trail", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, fullConversation(sessionId), 10);
    const evidenceTurn = page.locator("[data-testid='conversation-turn'][data-turn-shape='with-evidence']");
    await expect(evidenceTurn).toHaveCount(1);
    await expect(evidenceTurn.first()).toHaveCSS("border-left-width", "0px");
    await expect(evidenceTurn.first()).toHaveCSS("padding-left", "0px");
  });

  test("conversation area uses a compact centered prose lane", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("把这个页面整理得更像正式产品。");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "style-assistant-message" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "style-assistant-message",
        content: "可以。先把默认对话区收成一条安静的阅读栏，再处理行动卡片。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "style-assistant-message" },
    ], 10);

    const lane = page.getByTestId("message-lane");
    await expect(lane).toBeVisible();
    await expect(page.getByText("你的请求")).toHaveCount(0);

    const laneWidth = await lane.evaluate((node) => Math.round(node.getBoundingClientRect().width));
    expect(laneWidth).toBeLessThanOrEqual(860);

    const userMessage = page.getByTestId("user-message").last();
    const userMaterial = await userMessage.evaluate((node) => {
      const style = getComputedStyle(node);
      const alphaMatch = style.backgroundColor.match(/rgba?\(([^)]+)\)/);
      const channels = alphaMatch ? alphaMatch[1].split(",").map((part) => Number.parseFloat(part.trim())) : [];
      return {
        borderTopWidth: style.borderTopWidth,
        radius: Number.parseFloat(style.borderTopLeftRadius),
        backgroundAlpha: channels.length === 4 ? channels[3] : 1,
        boxShadow: style.boxShadow,
        transform: style.transform,
        before: getComputedStyle(node, "::before").content,
        after: getComputedStyle(node, "::after").content,
      };
    });
    expect(userMaterial.borderTopWidth).toBe("1px");
    expect(userMaterial.radius).toBeLessThanOrEqual(8);
    expect(userMaterial.backgroundAlpha).toBeGreaterThanOrEqual(0.9);
    expect(userMaterial.backgroundAlpha).toBeLessThanOrEqual(1);
    expect(userMaterial.boxShadow).toBe("none");
    expect(userMaterial.transform).toBe("none");
    expect(userMaterial.before).toBe("none");
    expect(userMaterial.after).toBe("none");
    await expect(page.getByTestId("assistant-message").last()).toHaveCSS("border-top-width", "0px");
  });

  test("scroll-to-bottom control stays quiet and editor-like", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const events = Array.from({ length: 40 }, (_, index) => ([
      { event_type: "text_start" as const, session_id: sessionId, block_id: `scroll-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `scroll-${index}`,
        content: `第 ${index + 1} 条输出，用来撑开滚动区域。这里保持足够长度，让对话区出现滚动。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `scroll-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, events, 1);

    await page.evaluate(() => {
      const lane = document.querySelector("[data-testid='message-lane']");
      const scroller = lane?.parentElement;
      if (!scroller) return;
      scroller.dispatchEvent(new WheelEvent("wheel", { deltaY: -160, bubbles: true }));
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    const control = page.getByTestId("scroll-to-bottom");
    await expect(control).toBeVisible();
    const metrics = await control.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        background: style.backgroundColor,
        backdrop: style.backdropFilter || style.getPropertyValue("-webkit-backdrop-filter"),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        shadow: style.boxShadow,
        width: Math.round(node.getBoundingClientRect().width),
        height: Math.round(node.getBoundingClientRect().height),
      };
    });
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.backdrop).not.toBe("none");
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.shadow).not.toBe("none");
    expect(metrics.width).toBe(28);
    expect(metrics.height).toBe(28);
  });

  test("scroll-to-bottom control stays outside the centered reading lane", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const events = Array.from({ length: 24 }, (_, index) => ([
      { event_type: "text_start" as const, session_id: sessionId, block_id: `scroll-lane-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `scroll-lane-${index}`,
        content: `第 ${index + 1} 条输出，用来撑开滚动区域。这里保持足够长度，让对话区出现滚动。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `scroll-lane-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, events, 1);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.dispatchEvent(new WheelEvent("wheel", { deltaY: -160, bubbles: true }));
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();

    const placement = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const button = document.querySelector<HTMLElement>("[data-testid='scroll-to-bottom']");
      if (!lane || !button) return null;
      const laneRect = lane.getBoundingClientRect();
      const buttonRect = button.getBoundingClientRect();
      return {
        laneLeft: Math.round(laneRect.left),
        laneRight: Math.round(laneRect.right),
        buttonLeft: Math.round(buttonRect.left),
        buttonRight: Math.round(buttonRect.right),
      };
    });

    expect(placement).not.toBeNull();
    expect(
      placement!.buttonLeft >= placement!.laneRight + 8 ||
      placement!.buttonRight <= placement!.laneLeft - 8,
    ).toBe(true);
  });

  test("composer aligns with the conversation lane and keeps pending state quiet", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const messageLane = page.getByTestId("message-lane");
    const composerLane = page.getByTestId("composer-lane");
    await expect(messageLane).toBeVisible();
    await expect(composerLane).toBeVisible();

    const layout = await page.evaluate(() => {
      const message = document.querySelector("[data-testid='message-lane']")?.getBoundingClientRect();
      const composer = document.querySelector("[data-testid='composer-lane']")?.getBoundingClientRect();
      return message && composer
        ? {
            messageX: Math.round(message.x),
            composerX: Math.round(composer.x),
            messageWidth: Math.round(message.width),
            composerWidth: Math.round(composer.width),
          }
        : null;
    });
    expect(layout).not.toBeNull();
    expect(layout!.messageWidth).toBeLessThanOrEqual(860);
    expect(layout!.composerWidth).toBeLessThanOrEqual(860);
    expect(Math.abs(layout!.messageX - layout!.composerX)).toBeLessThanOrEqual(4);
    await expect(composerLane.getByText("引用文件", { exact: true })).toHaveCount(0);
    await expect(composerLane.getByText("常用请求", { exact: true })).toHaveCount(0);
    await expect(composerLane.getByText("上下文 1M", { exact: true })).toHaveCount(0);
    await expect(composerLane.getByText("已启用能力")).toHaveCount(0);
    await expect(composerLane.getByRole("button", { name: /DeepSeek V4 Flash 1M/ })).toBeVisible();
    await expect(composerLane.getByText("DeepSeek V4 Flash 1M", { exact: true })).toBeVisible();

    await holdSendInput(page);

    await page.locator("textarea").fill("继续把对话区域靠近 Codex。");
    await page.locator("textarea").press("Enter");
    await expectHeldSendInput(page, "继续把对话区域靠近 Codex。");

    const pending = page.getByTestId("pending-block");
    await expect(pending).toBeVisible();
    await expect(pending).toHaveText(/正在组织回答/);
    await expect(pending).toHaveCSS("border-top-width", "0px");
    await expect(page.getByTestId("composer-surface")).toHaveAttribute("data-state", "running");
    await releaseHeldSendInput(page);
  });

  test("composer floats in a transparent frame with bottom breathing room", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const frameMetrics = await page.evaluate(() => {
      const composerFrameNode = document.querySelector("[data-testid='composer-frame']");
      const composerFrame = composerFrameNode?.getBoundingClientRect();
      const composerLane = document.querySelector("[data-testid='composer-lane']")?.getBoundingClientRect();
      if (!composerFrameNode || !composerFrame || !composerLane) return null;
      const frameStyle = getComputedStyle(composerFrameNode);

      return {
        frameBackground: frameStyle.backgroundColor,
        frameBorderTop: Math.round(Number.parseFloat(frameStyle.borderTopWidth)),
        frameShadow: frameStyle.boxShadow,
        frameBackdrop: frameStyle.backdropFilter || frameStyle.getPropertyValue("-webkit-backdrop-filter"),
        composerTop: Math.round(composerLane.top - composerFrame.top),
        composerBottom: Math.round(composerFrame.bottom - composerLane.bottom),
      };
    });

    expect(frameMetrics).not.toBeNull();
    expect(frameMetrics!.frameBackground).toBe("rgba(0, 0, 0, 0)");
    expect(frameMetrics!.frameBorderTop).toBe(0);
    expect(frameMetrics!.frameShadow).toBe("none");
    expect(frameMetrics!.frameBackdrop).toBe("none");
    expect(frameMetrics!.composerTop).toBe(14);
    expect(frameMetrics!.composerBottom).toBe(24);
  });

  test("conversation shell keeps transcript rhythm while composer floats", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const events = Array.from({ length: 24 }, (_, index) => ([
      { event_type: "text_start" as const, session_id: sessionId, block_id: `rhythm-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `rhythm-${index}`,
        content: `第 ${index + 1} 条输出，用来撑开滚动区域。这里保持足够长度，让对话区出现滚动。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `rhythm-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, events, 1);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.dispatchEvent(new WheelEvent("wheel", { deltaY: -160, bubbles: true }));
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();

    const rhythm = await page.evaluate(() => {
      const root = document.documentElement;
      const scroll = document.querySelector("[data-testid='conversation-scroll']");
      const composerFrame = document.querySelector("[data-testid='composer-frame']");
      const scrollButton = document.querySelector("[data-testid='scroll-to-bottom']");
      if (!scroll || !composerFrame || !scrollButton) return null;
      const scrollRect = scroll.getBoundingClientRect();
      const buttonRect = scrollButton.getBoundingClientRect();
      const token = getComputedStyle(root).getPropertyValue("--forge-conversation-gutter-y").trim();
      const scrollStyle = getComputedStyle(scroll);
      const composerStyle = getComputedStyle(composerFrame);

      return {
        token,
        scrollTop: Math.round(Number.parseFloat(scrollStyle.paddingTop)),
        scrollBottom: Math.round(Number.parseFloat(scrollStyle.paddingBottom)),
        composerTop: Math.round(Number.parseFloat(composerStyle.paddingTop)),
        composerBottom: Math.round(Number.parseFloat(composerStyle.paddingBottom)),
        scrollButtonBottom: Math.round(scrollRect.bottom - buttonRect.bottom),
      };
    });

    expect(rhythm).not.toBeNull();
    expect(rhythm!.token).toBe("18px");
    expect(rhythm!.scrollTop).toBe(18);
    expect(rhythm!.scrollBottom).toBe(18);
    expect(rhythm!.composerTop).toBe(14);
    expect(rhythm!.composerBottom).toBe(24);
    expect(rhythm!.scrollButtonBottom).toBe(18);
  });

  test("streaming output keeps bottom lock without stealing manual scroll", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const filler = Array.from({ length: 32 }, (_, index) => ([
      { event_type: "text_start" as const, session_id: sessionId, block_id: `stream-fill-${index}` },
      {
        event_type: "text_chunk" as const,
        session_id: sessionId,
        block_id: `stream-fill-${index}`,
        content: `第 ${index + 1} 条历史输出，用来撑开真实滚动区域。`,
      },
      { event_type: "text_end" as const, session_id: sessionId, block_id: `stream-fill-${index}` },
    ])).flat();
    await simulateStream(page, sessionId, filler, 1);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.scrollTop = scroller.scrollHeight;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "live-stream" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "正在整理第一段。" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "\n继续补充第二段，让输出变高。" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "\n最后收尾。" },
    ], 20);

    const bottomDistance = await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return null;
      return Math.round(scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight);
    });
    expect(bottomDistance).not.toBeNull();
    expect(bottomDistance!).toBeLessThanOrEqual(2);

    await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      if (!scroller) return;
      scroller.scrollTop = 0;
      scroller.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-stream", content: "\n用户在上方阅读时继续输出。" },
      { event_type: "text_end", session_id: sessionId, block_id: "live-stream" },
    ], 20);

    const manualScrollTop = await page.evaluate(() => {
      const scroller = document.querySelector("[data-testid='conversation-scroll']");
      return scroller ? Math.round(scroller.scrollTop) : null;
    });
    expect(manualScrollTop).toBe(0);
    await expect(page.getByTestId("scroll-to-bottom")).toBeVisible();
  });

  test("streaming chunks update quickly enough to feel live", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "live-cadence" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-cadence", content: "第一段" },
    ], 1);
    await expect(page.getByTestId("assistant-message").last()).toContainText("第一段");

    await simulateStream(page, sessionId, [
      { event_type: "text_chunk", session_id: sessionId, block_id: "live-cadence", content: "\n第二段" },
    ], 1);

    await expect(page.getByTestId("assistant-message").last()).toContainText("第二段", { timeout: 150 });
  });

  test("stopping generation keeps the conversation instead of deleting it", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "session_status", session_id: sessionId, status: "working" },
    ], 1);

    await page.getByTestId("composer-stop").click();

    const stoppedSessionId = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastKilledSessionId;
    });
    expect(stoppedSessionId).toBe(sessionId);
    await expect(page.getByRole("button", { name: "继续会话" })).toBeVisible();
    await expect(page.locator("aside").first().getByRole("button", { name: /删除对话/ })).toHaveCount(1);
  });

  test("deleting a conversation still removes it from history", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: /删除对话/ }).click();

    const deletedSessionId = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastDeletedSessionId;
    });
    expect(deletedSessionId).toBe(sessionId);
    await expect(sidebar.getByRole("button", { name: /删除对话/ })).toHaveCount(0);
    await expect(sidebar.getByText("还没有对话")).toBeVisible();
  });

  test("composer internals use editor rhythm tokens", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const rhythm = await page.evaluate(() => {
      const root = document.documentElement;
      const textareaWrap = document.querySelector("[data-testid='composer-textarea-wrap']");
      const toolbar = document.querySelector("[data-testid='composer-toolbar']");
      const textarea = document.querySelector("textarea");
      if (!textareaWrap || !toolbar || !textarea) return null;
      const textareaWrapStyle = getComputedStyle(textareaWrap);
      const toolbarStyle = getComputedStyle(toolbar);
      const textareaStyle = getComputedStyle(textarea);

      return {
        innerX: getComputedStyle(root).getPropertyValue("--forge-composer-inner-x").trim(),
        innerY: getComputedStyle(root).getPropertyValue("--forge-composer-inner-y").trim(),
        textPadLeft: Math.round(Number.parseFloat(textareaWrapStyle.paddingLeft)),
        textPadTop: Math.round(Number.parseFloat(textareaWrapStyle.paddingTop)),
        toolbarPadLeft: Math.round(Number.parseFloat(toolbarStyle.paddingLeft)),
        toolbarPadBottom: Math.round(Number.parseFloat(toolbarStyle.paddingBottom)),
        textareaLineHeight: Math.round(Number.parseFloat(textareaStyle.lineHeight)),
        textareaScrollbarWidth: textareaStyle.scrollbarWidth,
      };
    });

    expect(rhythm).not.toBeNull();
    expect(rhythm!.innerX).toBe("18px");
    expect(rhythm!.innerY).toBe("16px");
    expect(rhythm!.textPadLeft).toBe(18);
    expect(rhythm!.textPadTop).toBe(16);
    expect(rhythm!.toolbarPadLeft).toBe(18);
    expect(rhythm!.toolbarPadBottom).toBe(8);
    expect(rhythm!.textareaLineHeight).toBe(24);
    expect(rhythm!.textareaScrollbarWidth).toBe("thin");
  });

  test("composer floating menus sit above the editor without overlap", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    await composer.getByRole("button", { name: "常用请求" }).click();
    const menu = page.getByTestId("composer-command-menu");
    await expect(menu).toBeVisible();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const menu = document.querySelector("[data-testid='composer-command-menu']");
      const surface = document.querySelector("[data-testid='composer-surface']");
      const option = document.querySelector("[data-testid='composer-command-menu'] [role='option']");
      if (!menu || !surface || !option) return null;
      const menuRect = menu.getBoundingClientRect();
      const surfaceRect = surface.getBoundingClientRect();
      const menuStyle = getComputedStyle(menu);
      return {
        gapToken: getComputedStyle(root).getPropertyValue("--forge-floating-gap").trim(),
        menuBottomGap: Math.round(surfaceRect.top - menuRect.bottom),
        menuWidth: Math.round(menuRect.width),
        surfaceWidth: Math.round(surfaceRect.width),
        menuShadow: menuStyle.boxShadow,
        optionHeight: Math.round(option.getBoundingClientRect().height),
        radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.gapToken).toBe("8px");
    expect(metrics!.menuBottomGap).toBeGreaterThanOrEqual(7);
    expect(metrics!.menuBottomGap).toBeLessThanOrEqual(8);
    expect(metrics!.menuWidth).toBeLessThanOrEqual(metrics!.surfaceWidth);
    expect(metrics!.menuWidth).toBeLessThanOrEqual(560);
    expect(metrics!.menuShadow).not.toContain("0px 25px");
    expect(metrics!.optionHeight).toBe(34);
    expect(metrics!.radius).toBeLessThanOrEqual(8);
  });

  test("composer model menu uses a grounded desktop picker surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const modelButton = page.getByTestId("composer-model-chip");
    await modelButton.click();
    const menu = page.getByRole("menu");
    await expect(menu).toBeVisible();
    await expect(modelButton).toHaveAttribute("aria-expanded", "true");

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const menu = document.querySelector("[role='menu']");
      const button = document.querySelector("[data-testid='composer-model-chip']");
      const surface = document.querySelector("[data-testid='composer-surface']");
      const active = menu?.querySelector("[role='menuitemradio'][aria-checked='true']");
      const firstOption = menu?.querySelector("[role='menuitemradio']");
      const heading = menu?.querySelector(".forge-menu-heading");
      if (!menu || !button || !surface || !active || !firstOption || !heading) return null;
      const menuRect = menu.getBoundingClientRect();
      const buttonRect = button.getBoundingClientRect();
      const surfaceRect = surface.getBoundingClientRect();
      const menuStyle = getComputedStyle(menu);
      const activeStyle = getComputedStyle(active);
      return {
        gapToken: getComputedStyle(root).getPropertyValue("--forge-floating-gap").trim(),
        menuBottomGap: Math.round(buttonRect.top - menuRect.bottom),
        surfaceBottomGap: Math.round(surfaceRect.top - menuRect.bottom),
        minWidth: Math.round(Number.parseFloat(menuStyle.minWidth)),
        backdrop: menuStyle.backdropFilter || menuStyle.webkitBackdropFilter,
        shadow: menuStyle.boxShadow,
        background: menuStyle.backgroundColor,
        radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
        optionHeight: Math.round(firstOption.getBoundingClientRect().height),
        activeBorder: Math.round(Number.parseFloat(activeStyle.borderTopWidth)),
        activeBackground: activeStyle.backgroundColor,
        headingHeight: Math.round(heading.getBoundingClientRect().height),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.gapToken).toBe("8px");
    expect(metrics!.menuBottomGap).toBeGreaterThan(8);
    expect(metrics!.surfaceBottomGap).toBeGreaterThanOrEqual(7);
    expect(metrics!.surfaceBottomGap).toBeLessThanOrEqual(8);
    expect(metrics!.minWidth).toBeGreaterThanOrEqual(300);
    expect(metrics!.backdrop).toContain("blur");
    expect(metrics!.shadow).not.toContain("0px 10px 24px");
    expect(metrics!.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.radius).toBeLessThanOrEqual(8);
    expect(metrics!.optionHeight).toBeGreaterThanOrEqual(44);
    expect(metrics!.activeBorder).toBe(1);
    expect(metrics!.activeBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.headingHeight).toBeLessThanOrEqual(30);
  });

  test("composer send states stay compact without primary fill", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const send = page.getByTestId("composer-send");
    await expect(send).toBeDisabled();
    await send.hover({ force: true });

    const metrics = await send.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        color: style.color,
        cursor: style.cursor,
      };
    });

    expect(metrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(metrics.borderColor).toBe("rgba(0, 0, 0, 0)");
    expect(metrics.color).toBe("rgba(184, 180, 170, 0.48)");
    expect(metrics.cursor).toBe("default");

    await page.locator("textarea").fill("继续优化当前界面");
    await expect(send).toBeEnabled();
    await expect(send).toHaveAttribute("data-ready", "true");
    await expect.poll(async () => send.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        backgroundReady: style.backgroundColor !== "rgba(0, 0, 0, 0)",
        borderReady: style.borderTopColor !== "rgba(0, 0, 0, 0)",
        shadowReady: style.boxShadow !== "none",
      };
    })).toEqual({ backgroundReady: true, borderReady: true, shadowReady: true });

    const readyMetrics = await send.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        boxShadow: style.boxShadow,
      };
    });

    expect(readyMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(readyMetrics.background).not.toBe("rgb(184, 138, 86)");
    expect(readyMetrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(readyMetrics.boxShadow).not.toBe("none");
  });

  test("composer command menu keeps keyboard selection visible and compact", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    await composer.getByRole("button", { name: "常用请求" }).click();
    const menu = page.getByTestId("composer-command-menu");
    await expect(menu).toBeVisible();
    await page.keyboard.press("ArrowDown");
    await page.waitForFunction(() => {
      const selected = document.querySelector("[data-testid='composer-command-menu'] [role='option'][aria-selected='true']");
      if (!selected) return false;
      const rootStyle = getComputedStyle(document.documentElement);
      const style = getComputedStyle(selected);
      return style.backgroundColor === rootStyle.getPropertyValue("--forge-hover").trim() &&
        Math.round(Number.parseFloat(style.borderTopWidth)) === 1;
    });

    const metrics = await page.evaluate(() => {
      const menu = document.querySelector("[data-testid='composer-command-menu']");
      const selected = menu?.querySelector("[role='option'][aria-selected='true']");
      const options = Array.from(menu?.querySelectorAll("[role='option']") ?? []);
      if (!menu || !selected || options.length === 0) return null;
      const rootStyle = getComputedStyle(document.documentElement);
      const selectedStyle = getComputedStyle(selected);
      return {
        hoverToken: rootStyle.getPropertyValue("--forge-hover").trim(),
        optionCount: options.length,
        selectedText: selected.textContent ?? "",
        selectedHeight: Math.round(selected.getBoundingClientRect().height),
        selectedBackground: selectedStyle.backgroundColor,
        selectedRadius: Number.parseFloat(selectedStyle.borderTopLeftRadius),
        selectedBorder: Math.round(Number.parseFloat(selectedStyle.borderTopWidth)),
        selectedShadow: selectedStyle.boxShadow,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.optionCount).toBeGreaterThanOrEqual(6);
    expect(metrics!.selectedText).toContain("/fix");
    expect(metrics!.selectedHeight).toBe(34);
    expect(metrics!.selectedBackground).toBe(metrics!.hoverToken);
    expect(metrics!.selectedRadius).toBeLessThanOrEqual(8);
    expect(metrics!.selectedBorder).toBe(1);
    expect(metrics!.selectedShadow).not.toBe("none");
  });

  test("assistant prose and user bubbles share readable message primitives", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("整理这个页面");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "primitive-text" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "primitive-text",
        content: "可以。先把可读性收稳，再继续压低 UI 噪音。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "primitive-text" },
    ], 1);

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const assistant = document.querySelector("[data-testid='assistant-message']");
      const user = document.querySelector("[data-testid='user-message']");
      if (!assistant || !user) return null;
      const assistantStyle = getComputedStyle(assistant);
      const userStyle = getComputedStyle(user);
      return {
        assistantLineToken: getComputedStyle(root).getPropertyValue("--forge-assistant-line-height").trim(),
        userLineToken: getComputedStyle(root).getPropertyValue("--forge-user-line-height").trim(),
        assistantLineHeight: Math.round(Number.parseFloat(assistantStyle.lineHeight)),
        userLineHeight: Math.round(Number.parseFloat(userStyle.lineHeight)),
        userShadow: userStyle.boxShadow,
        userRadius: Number.parseFloat(userStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.assistantLineToken).toBe("25px");
    expect(metrics!.userLineToken).toBe("22px");
    expect(metrics!.assistantLineHeight).toBe(25);
    expect(metrics!.userLineHeight).toBe(22);
    expect(metrics!.userShadow).toBe("none");
    expect(metrics!.userRadius).toBeLessThanOrEqual(8);
  });

  test("assistant and user messages expose quiet copy actions", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error test clipboard capture
      window.__clipboardText = "";
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async (text: string) => {
            // @ts-expect-error test clipboard capture
            window.__clipboardText = text;
          },
        },
      });
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const prompt = "请看 `src/App.tsx:1`，然后总结一下。";
    await page.locator("textarea").fill(prompt);
    await page.locator("textarea").press("Enter");

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "copyable-reply" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "copyable-reply",
        content: "## 结论\n\n这个改动可以继续推进。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "copyable-reply" },
    ], 1);

    const userMessage = page.getByTestId("user-message").last();
    const assistantMessage = page.getByTestId("assistant-message").last();
    const userCopy = userMessage.getByRole("button", { name: "复制提问" });
    const assistantCopy = assistantMessage.getByRole("button", { name: "复制回复" });
    const assistantOpacityBeforeHover = await assistantCopy.evaluate((action) => Number.parseFloat(getComputedStyle(action).opacity));
    expect(assistantOpacityBeforeHover).toBe(0);

    await userMessage.hover();
    await expect(userCopy).toBeVisible();
    await userCopy.click();
    await expect(userCopy).toHaveAttribute("aria-label", "已复制提问");
    await expect(page.evaluate(() => {
      // @ts-expect-error test clipboard capture
      return window.__clipboardText;
    })).resolves.toBe(prompt);

    await assistantMessage.hover();
    await expect(assistantCopy).toBeVisible();
    await assistantCopy.click();
    await expect(assistantCopy).toHaveAttribute("aria-label", "已复制回复");
    await expect(page.evaluate(() => {
      // @ts-expect-error test clipboard capture
      return window.__clipboardText;
    })).resolves.toContain("## 结论");

    const metrics = await page.evaluate(() => {
      const action = document.querySelector<HTMLElement>("[data-testid='assistant-message'] [data-testid='message-copy-action']");
      if (!action) return null;
      const style = getComputedStyle(action);
      const styleWithWebkit = style as CSSStyleDeclaration & { webkitBackdropFilter?: string };
      return {
        width: Math.round(Number.parseFloat(style.width)),
        height: Math.round(Number.parseFloat(style.height)),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        position: style.position,
        top: Math.round(Number.parseFloat(style.top)),
        right: Math.round(Number.parseFloat(style.right)),
        background: style.backgroundColor,
        boxShadow: style.boxShadow,
        backdropFilter: style.backdropFilter || styleWithWebkit.webkitBackdropFilter || "",
        transform: style.transform,
        transitionProperty: style.transitionProperty,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.width).toBe(26);
    expect(metrics!.height).toBe(26);
    expect(metrics!.radius).toBeLessThanOrEqual(8);
    expect(metrics!.position).toBe("absolute");
    expect(metrics!.top).toBeGreaterThanOrEqual(0);
    expect(metrics!.top).toBeLessThanOrEqual(4);
    expect(metrics!.right).toBeGreaterThanOrEqual(0);
    expect(metrics!.right).toBeLessThanOrEqual(4);
    expect(metrics!.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.boxShadow).not.toBe("none");
    expect(metrics!.backdropFilter).not.toBe("none");
    expect(metrics!.transform).not.toBe("none");
    expect(metrics!.transitionProperty).toContain("transform");
  });

  test("assistant markdown uses a compact editorial rhythm", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "markdown-rhythm" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "markdown-rhythm",
        content: [
          "先把阅读节奏收稳。",
          "",
          "## 排版目标",
          "",
          "- 文字要安静",
          "- 层级要清楚",
          "",
          "> 过程信息可以轻，结论必须清楚。",
          "",
          "---",
          "",
          "使用 `npm run build` 作为最小验证。",
          "",
          "| 项目 | 状态 |",
          "| --- | --- |",
          "| 预览 | 可用 |",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "markdown-rhythm" },
    ], 1);

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const assistant = document.querySelector("[data-testid='assistant-message']");
      if (!assistant) return null;
      const assistantStyle = getComputedStyle(assistant);
      const paragraph = assistant.querySelector("p");
      const heading = assistant.querySelector("h2");
      const list = assistant.querySelector("ul");
      const listItem = assistant.querySelector("li");
      const quote = assistant.querySelector("blockquote");
      const rule = assistant.querySelector("hr");
      const inlineCode = assistant.querySelector("p code");
      const table = assistant.querySelector("table");
      const tableCell = assistant.querySelector("td");
      if (!paragraph || !heading || !list || !listItem || !quote || !rule || !inlineCode || !table || !tableCell) return null;
      const paragraphStyle = getComputedStyle(paragraph);
      const headingStyle = getComputedStyle(heading);
      const listStyle = getComputedStyle(list);
      const listItemStyle = getComputedStyle(listItem);
      const quoteStyle = getComputedStyle(quote);
      const ruleStyle = getComputedStyle(rule);
      const codeStyle = getComputedStyle(inlineCode);
      const tableStyle = getComputedStyle(table);
      const cellStyle = getComputedStyle(tableCell);

      return {
        paragraphGapToken: getComputedStyle(root).getPropertyValue("--forge-markdown-paragraph-gap").trim(),
        blockGapToken: getComputedStyle(root).getPropertyValue("--forge-markdown-block-gap").trim(),
        assistantMaxWidth: assistantStyle.maxWidth,
        assistantWidth: Math.round(assistant.getBoundingClientRect().width),
        assistantPaddingRight: Math.round(Number.parseFloat(assistantStyle.paddingRight)),
        assistantOverflowWrap: assistantStyle.overflowWrap,
        paragraphMarginBottom: Math.round(Number.parseFloat(paragraphStyle.marginBottom)),
        headingFontSize: Math.round(Number.parseFloat(headingStyle.fontSize)),
        headingLineHeight: Math.round(Number.parseFloat(headingStyle.lineHeight)),
        headingMarginTop: Math.round(Number.parseFloat(headingStyle.marginTop)),
        listPaddingLeft: Math.round(Number.parseFloat(listStyle.paddingLeft)),
        listItemMarginBottom: Math.round(Number.parseFloat(listItemStyle.marginBottom)),
        quoteBorderWidth: Math.round(Number.parseFloat(quoteStyle.borderLeftWidth)),
        quoteBorderTopWidth: Math.round(Number.parseFloat(quoteStyle.borderTopWidth)),
        quoteBorderColor: quoteStyle.borderLeftColor,
        quoteBorderTopColor: quoteStyle.borderTopColor,
        quoteBackground: quoteStyle.backgroundColor,
        quoteRadius: Number.parseFloat(quoteStyle.borderTopLeftRadius),
        quotePaddingLeft: Math.round(Number.parseFloat(quoteStyle.paddingLeft)),
        ruleHeight: Math.round(rule.getBoundingClientRect().height),
        ruleMarginTop: Math.round(Number.parseFloat(ruleStyle.marginTop)),
        ruleBackground: ruleStyle.backgroundColor,
        codeBackground: codeStyle.backgroundColor,
        codePaddingLeft: Math.round(Number.parseFloat(codeStyle.paddingLeft)),
        tableDisplay: tableStyle.display,
        tableBackground: tableStyle.backgroundColor,
        tableMarginTop: Math.round(Number.parseFloat(tableStyle.marginTop)),
        tableMaxWidth: tableStyle.maxWidth,
        tableOverflowX: tableStyle.overflowX,
        tableScrollbarWidth: tableStyle.scrollbarWidth,
        cellPaddingTop: Math.round(Number.parseFloat(cellStyle.paddingTop)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.paragraphGapToken).toBe("9px");
    expect(metrics!.blockGapToken).toBe("14px");
    expect(metrics!.assistantMaxWidth).not.toBe("none");
    expect(metrics!.assistantWidth).toBeLessThanOrEqual(760);
    expect(metrics!.assistantPaddingRight).toBeGreaterThanOrEqual(34);
    expect(metrics!.assistantOverflowWrap).toBe("anywhere");
    expect(metrics!.paragraphMarginBottom).toBe(9);
    expect(metrics!.headingFontSize).toBe(15);
    expect(metrics!.headingLineHeight).toBe(23);
    expect(metrics!.headingMarginTop).toBe(18);
    expect(metrics!.listPaddingLeft).toBe(20);
    expect(metrics!.listItemMarginBottom).toBe(2);
    expect(metrics!.quoteBorderWidth).toBe(1);
    expect(metrics!.quoteBorderTopWidth).toBe(1);
    expect(metrics!.quoteBorderColor).toBe(metrics!.quoteBorderTopColor);
    expect(metrics!.quoteBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.quoteRadius).toBeLessThanOrEqual(8);
    expect(metrics!.quotePaddingLeft).toBe(12);
    expect(metrics!.ruleHeight).toBe(1);
    expect(metrics!.ruleMarginTop).toBe(14);
    expect(metrics!.ruleBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.codeBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.codePaddingLeft).toBeGreaterThanOrEqual(4);
    expect(metrics!.tableDisplay).toBe("block");
    expect(metrics!.tableBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.tableMarginTop).toBe(14);
    expect(metrics!.tableMaxWidth).toBe("100%");
    expect(metrics!.tableOverflowX).toBe("auto");
    expect(metrics!.tableScrollbarWidth).toBe("thin");
    expect(metrics!.cellPaddingTop).toBe(7);
  });

  test("markdown tables fit their content before falling back to horizontal scroll", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "table-compat" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "table-compat",
        content: [
          "这里有一张短表和一张很宽的表。",
          "",
          "| 名称 | 状态 |",
          "| --- | --- |",
          "| buildTool() | 可复用 |",
          "",
          "| 扩展点 | 机制 | 示例 | 备注 |",
          "| --- | --- | --- | --- |",
          "| 自定义 Agent | 项目目录放 .md 文件 | my-project/.claude/agents/reviewer-with-a-very-long-name.md | 这列故意很长用来验证横向滚动不会撑破消息栏 |",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "table-compat" },
    ], 1);

    const metrics = await page.evaluate(() => {
      const assistant = document.querySelector<HTMLElement>("[data-testid='assistant-message']");
      const tables = Array.from(document.querySelectorAll<HTMLElement>("[data-testid='assistant-message'] table"));
      if (!assistant || tables.length < 2) return null;
      const [compact, wide] = tables;
      const compactRect = compact.getBoundingClientRect();
      const wideRect = wide.getBoundingClientRect();
      const assistantRect = assistant.getBoundingClientRect();
      return {
        assistantWidth: Math.round(assistantRect.width),
        compactWidth: Math.round(compactRect.width),
        wideWidth: Math.round(wideRect.width),
        wideClientWidth: Math.round(wide.clientWidth),
        wideScrollWidth: Math.round(wide.scrollWidth),
        compactOverflowX: getComputedStyle(compact).overflowX,
        wideOverflowX: getComputedStyle(wide).overflowX,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.compactWidth).toBeLessThan(metrics!.assistantWidth - 160);
    expect(metrics!.compactWidth).toBeLessThanOrEqual(360);
    expect(metrics!.wideWidth).toBeLessThanOrEqual(metrics!.assistantWidth);
    expect(metrics!.wideScrollWidth).toBeGreaterThan(metrics!.wideClientWidth);
    expect(metrics!.compactOverflowX).toBe("auto");
    expect(metrics!.wideOverflowX).toBe("auto");
  });

  test("inline file references stay quiet and wrap within the message lane", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "inline-file-ref" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "inline-file-ref",
        content: [
          "可以先检查 `src/features/deep-context/components/ProjectArchiveInspectorReallyLongNameForWrap.tsx:128`，再看相邻渲染逻辑。",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "inline-file-ref" },
    ], 1);

    const assistant = page.getByTestId("assistant-message");
    const fileRef = assistant.locator(".forge-inline-code-file .forge-file-ref");
    await expect(fileRef.locator(".forge-file-ref-icon")).toBeVisible();
    await expect(fileRef.locator(".forge-file-ref-name")).toHaveText("ProjectArchiveInspectorReallyLongNameForWrap.tsx");
    await expect(fileRef.locator(".forge-file-ref-line")).toHaveText("line 128");
    await expect(fileRef).toHaveAttribute("title", "src/features/deep-context/components/ProjectArchiveInspectorReallyLongNameForWrap.tsx:128");
    await expect(fileRef).toHaveAttribute("aria-label", "打开 src/features/deep-context/components/ProjectArchiveInspectorReallyLongNameForWrap.tsx:128");

    const metrics = await assistant.evaluate((node) => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const token = node.querySelector<HTMLElement>(".forge-inline-code-file");
      const link = node.querySelector<HTMLElement>(".forge-inline-code-file .forge-file-ref");
      if (!lane || !token || !link) return null;
      const laneRect = lane.getBoundingClientRect();
      const tokenRect = token.getBoundingClientRect();
      const tokenStyle = getComputedStyle(token);
      const linkStyle = getComputedStyle(link);
      return {
        laneWidth: Math.round(laneRect.width),
        tokenWidth: Math.round(tokenRect.width),
        tokenRight: Math.round(tokenRect.right),
        laneRight: Math.round(laneRect.right),
        tokenOverflowWrap: tokenStyle.overflowWrap,
        tokenWordBreak: tokenStyle.wordBreak,
        linkTextDecoration: linkStyle.textDecorationLine,
        linkDisplay: linkStyle.display,
        linkGap: Math.round(Number.parseFloat(linkStyle.gap)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.tokenWidth).toBeLessThan(metrics!.laneWidth);
    expect(metrics!.tokenRight).toBeLessThanOrEqual(metrics!.laneRight);
    expect(metrics!.tokenOverflowWrap).toBe("anywhere");
    expect(metrics!.tokenWordBreak).toBe("normal");
    expect(metrics!.linkTextDecoration).toBe("none");
    expect(metrics!.linkDisplay).toBe("inline-flex");
    expect(metrics!.linkGap).toBeGreaterThanOrEqual(4);
  });

  test("conversation turns keep quiet separation without card framing", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("第一轮问题");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "turn-one" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "turn-one", content: "第一轮回复。" },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-one" },
    ], 1);

    await page.locator("textarea").fill("第二轮问题");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "turn-two" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "turn-two", content: "第二轮回复。" },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-two" },
    ], 1);

    await expect(page.getByTestId("conversation-turn")).toHaveCount(2);

    const metrics = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const turns = Array.from(document.querySelectorAll<HTMLElement>("[data-testid='conversation-turn']"));
      const second = turns[1];
      if (!lane || !second) return null;
      const laneStyle = getComputedStyle(lane);
      const turnStyle = getComputedStyle(second);
      return {
        laneGap: Math.round(Number.parseFloat(laneStyle.rowGap)),
        secondPaddingTop: Math.round(Number.parseFloat(turnStyle.paddingTop)),
        secondBorderRadius: Math.round(Number.parseFloat(turnStyle.borderTopLeftRadius)),
        secondBackground: turnStyle.backgroundColor,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.laneGap).toBe(14);
    expect(metrics!.secondPaddingTop).toBe(16);
    expect(metrics!.secondBorderRadius).toBe(0);
    expect(metrics!.secondBackground).toBe("rgba(0, 0, 0, 0)");
  });

  test("code blocks use a compact reader surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "code-rhythm" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "code-rhythm",
        content: [
          "可以先这样写：",
          "",
          "```ts",
          "export function sum(a: number, b: number) {",
          "  return a + b;",
          "}",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "code-rhythm" },
    ], 1);

    await expect(page.locator(".code-surface")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const surface = document.querySelector(".code-surface");
      const header = surface?.querySelector("figcaption");
      const label = surface?.querySelector("figcaption span:nth-child(2)");
      const code = surface?.querySelector(".shiki-wrapper .shiki code, .code-fallback code");
      if (!surface || !header || !label || !code) return null;
      const surfaceStyle = getComputedStyle(surface);
      const headerStyle = getComputedStyle(header);
      const labelStyle = getComputedStyle(label);
      const codeStyle = getComputedStyle(code);
      return {
        marginTop: Math.round(Number.parseFloat(surfaceStyle.marginTop)),
        marginBottom: Math.round(Number.parseFloat(surfaceStyle.marginBottom)),
        headerHeight: Math.round(header.getBoundingClientRect().height),
        headerBackground: headerStyle.backgroundColor,
        labelFontSize: Math.round(Number.parseFloat(labelStyle.fontSize)),
        codeLineHeight: Math.round(Number.parseFloat(codeStyle.lineHeight)),
        codeFontSize: Number.parseFloat(codeStyle.fontSize),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.marginTop).toBe(14);
    expect(metrics!.marginBottom).toBe(14);
    expect(metrics!.headerHeight).toBe(34);
    expect(metrics!.headerBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.labelFontSize).toBe(10);
    expect(metrics!.codeLineHeight).toBe(20);
    expect(metrics!.codeFontSize).toBeCloseTo(12.5);
  });

  test("reader surface caption actions share quiet desktop material", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "caption-actions" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "caption-actions",
        content: [
          "先看代码，再看架构图。",
          "",
          "```ts",
          "const stable = true;",
          "```",
          "",
          "```text",
          "┌─────────────┐",
          "│ Composer    │",
          "└──────┬──────┘",
          "       ▼",
          "┌─────────────┐",
          "│ Tool Row    │",
          "└─────────────┘",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "caption-actions" },
    ], 1);

    await expect(page.locator(".code-surface")).toBeVisible();
    await expect(page.getByTestId("diagram-surface")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const codeAction = document.querySelector<HTMLElement>(".code-caption button[aria-label='复制代码']");
      const diagramAction = document.querySelector<HTMLElement>(".diagram-caption button[aria-label='复制图示源码']");
      const actions = [codeAction, diagramAction].filter(Boolean) as HTMLElement[];
      if (actions.length !== 2) return null;
      return actions.map((action) => {
        const style = getComputedStyle(action);
        return {
          hasClass: action.classList.contains("forge-caption-action"),
          width: Math.round(action.getBoundingClientRect().width),
          height: Math.round(action.getBoundingClientRect().height),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          background: style.backgroundColor,
          border: style.borderTopColor,
          color: style.color,
          transitionProperty: style.transitionProperty,
        };
      });
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.every((item) => item.hasClass)).toBe(true);
    expect(metrics!.every((item) => item.width === 24 && item.height === 24)).toBe(true);
    expect(metrics!.every((item) => item.radius <= 8)).toBe(true);
    expect(metrics!.every((item) => item.background !== "rgba(0, 0, 0, 0)")).toBe(true);
    expect(metrics!.every((item) => item.border !== "rgba(0, 0, 0, 0)")).toBe(true);
    expect(metrics!.every((item) => item.color !== "rgb(184, 138, 86)")).toBe(true);
    expect(metrics!.every((item) => item.transitionProperty.includes("box-shadow"))).toBe(true);

    await page.getByRole("button", { name: "复制代码" }).hover();
    await expect(page.getByRole("button", { name: "复制代码" })).not.toHaveCSS("box-shadow", "none");
  });

  test("ascii architecture diagrams render as diagram surfaces instead of code blocks", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "ascii-diagram" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "ascii-diagram",
        content: [
          "架构图（简化）",
          "",
          "```text",
          "┌──────────────────────────────────────────┐",
          "│              主 LLM 循环                 │",
          "│  main.tsx -> query.ts -> Anthropic API    │",
          "└──────────────────────────────────────────┘",
          "                    │",
          "                    ▼",
          "┌───────────────────┬──────────────────────┐",
          "│ Agent 工具        │ Coordinator 模式      │",
          "│ 解析定义          │ 并行派发任务          │",
          "│ 返回结果          │ 合成结果              │",
          "└───────────────────┴──────────────────────┘",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "ascii-diagram" },
    ], 1);

    const diagram = page.getByTestId("diagram-surface");
    await expect(diagram).toBeVisible();
    await expect(diagram).toHaveAttribute("data-diagram-kind", "ascii");
    await expect(diagram.getByText("架构图", { exact: true })).toBeVisible();
    await expect(diagram.locator(".diagram-code")).toContainText("主 LLM 循环");
    await expect(page.locator(".code-surface")).toHaveCount(0);
    await expect(diagram.locator(".shiki-wrapper")).toHaveCount(0);

    const metrics = await diagram.evaluate((node) => {
      const viewport = node.querySelector<HTMLElement>("[data-testid='diagram-viewport']");
      const caption = node.querySelector<HTMLElement>(".diagram-caption");
      const code = node.querySelector<HTMLElement>(".diagram-code");
      const style = getComputedStyle(node);
      const captionStyle = caption ? getComputedStyle(caption) : null;
      const viewportStyle = viewport ? getComputedStyle(viewport) : null;
      const codeStyle = code ? getComputedStyle(code) : null;
      const rect = node.getBoundingClientRect();
      return {
        width: Math.round(rect.width),
        marginTop: Math.round(Number.parseFloat(style.marginTop)),
        marginBottom: Math.round(Number.parseFloat(style.marginBottom)),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        border: style.borderTopColor,
        captionHeight: caption ? Math.round(caption.getBoundingClientRect().height) : 0,
        captionBackground: captionStyle?.backgroundColor ?? "",
        viewportDisplay: viewportStyle?.display ?? "",
        viewportJustify: viewportStyle?.justifyContent ?? "",
        viewportPaddingTop: viewportStyle ? Math.round(Number.parseFloat(viewportStyle.paddingTop)) : 0,
        viewportMaxHeight: viewportStyle?.maxHeight ?? "",
        viewportBackground: viewportStyle?.backgroundColor ?? "",
        viewportBackgroundImage: viewportStyle?.backgroundImage ?? "",
        codeColor: codeStyle?.color ?? "",
        codeLineHeight: codeStyle ? Math.round(Number.parseFloat(codeStyle.lineHeight)) : 0,
        codeFontSize: codeStyle ? Number.parseFloat(codeStyle.fontSize) : 0,
      };
    });

    expect(metrics.width).toBeLessThanOrEqual(780);
    expect(metrics.marginTop).toBe(14);
    expect(metrics.marginBottom).toBe(14);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.border).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.captionHeight).toBe(34);
    expect(metrics.captionBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.viewportDisplay).toBe("block");
    expect(metrics.viewportJustify).not.toBe("center");
    expect(metrics.viewportPaddingTop).toBe(16);
    expect(metrics.viewportMaxHeight).not.toBe("none");
    expect(metrics.viewportBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.viewportBackgroundImage).toBe("none");
    expect(metrics.codeColor).not.toBe("rgb(184, 138, 86)");
    expect(metrics.codeLineHeight).toBe(20);
    expect(metrics.codeFontSize).toBeLessThanOrEqual(12.5);
  });

  test("wide ascii diagrams keep their left edge reachable inside the diagram viewport", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const longRule = "─".repeat(220);
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "wide-ascii-diagram" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "wide-ascii-diagram",
        content: [
          "```text",
          `┌${longRule}┐`,
          "│ Planner ───────────────────────────────→ Executor ───────────────────────────────→ Verifier ───────────────────────────────→ Report │",
          `└${longRule}┘`,
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "wide-ascii-diagram" },
    ], 1);

    const metrics = await page.getByTestId("diagram-surface").evaluate((node) => {
      const viewport = node.querySelector<HTMLElement>("[data-testid='diagram-viewport']");
      const code = node.querySelector<HTMLElement>(".diagram-code");
      if (!viewport || !code) return null;
      const viewportRect = viewport.getBoundingClientRect();
      const codeRect = code.getBoundingClientRect();
      const viewportStyle = getComputedStyle(viewport);
      return {
        viewportDisplay: viewportStyle.display,
        viewportJustify: viewportStyle.justifyContent,
        viewportPaddingLeft: Math.round(Number.parseFloat(viewportStyle.paddingLeft)),
        viewportClientWidth: Math.round(viewport.clientWidth),
        viewportScrollWidth: Math.round(viewport.scrollWidth),
        codeLeft: Math.round(codeRect.left),
        viewportLeft: Math.round(viewportRect.left),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.viewportDisplay).toBe("block");
    expect(metrics!.viewportJustify).not.toBe("center");
    expect(metrics!.viewportScrollWidth).toBeGreaterThan(metrics!.viewportClientWidth);
    expect(metrics!.codeLeft).toBeGreaterThanOrEqual(metrics!.viewportLeft + metrics!.viewportPaddingLeft - 1);
  });

  test("unlabelled multiline box diagrams use the diagram renderer", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "unlabelled-diagram" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "unlabelled-diagram",
        content: [
          "```",
          "+-----------+     +-----------+",
          "| Planner   | --> | Executor  |",
          "+-----------+     +-----------+",
          "      |                 |",
          "      v                 v",
          "+-----------+     +-----------+",
          "| Context   | <-- | Result    |",
          "+-----------+     +-----------+",
          "```",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "unlabelled-diagram" },
    ], 1);

    await expect(page.getByTestId("diagram-surface")).toBeVisible();
    await expect(page.getByTestId("diagram-surface").locator(".diagram-code")).toContainText("Planner");
    await expect(page.locator(".code-surface")).toHaveCount(0);
  });

  test("streaming markdown renders structure before the final chunk", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "session_status", session_id: sessionId, status: "working" },
      { event_type: "text_start", session_id: sessionId, block_id: "streaming-markdown" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "streaming-markdown",
        content: [
          "## 正在整理",
          "",
          "- 先保持结构",
          "- 再补充代码",
          "",
          "```ts",
          "const preview = true;",
        ].join("\n"),
      },
    ], 1);

    const assistant = page.getByTestId("assistant-message");
    await expect(assistant.locator("h2")).toContainText("正在整理");
    await expect(assistant.locator("li").first()).toContainText("先保持结构");
    const streamingCode = assistant.locator(".code-surface");
    await expect(streamingCode).toBeVisible();
    await expect(streamingCode).toHaveAttribute("data-renderer", "plain");
    await expect(streamingCode.locator(".code-fallback code")).toContainText("const preview = true;");
    await expect(streamingCode.locator(".shiki-wrapper")).toHaveCount(0);
    await expect(page.getByTestId("composer-surface")).toHaveAttribute("data-state", "running");

    const streamingMetrics = await page.evaluate(() => {
      const assistant = document.querySelector("[data-testid='assistant-message']");
      if (!assistant) return null;
      const heading = assistant.querySelector("h2");
      const listItem = assistant.querySelector("li");
      const codeSurface = assistant.querySelector(".code-surface");
      const highlightedCode = assistant.querySelector(".shiki-wrapper");
      const plaintextWrapper = assistant.querySelector(".whitespace-pre-wrap");
      return {
        hasHeading: Boolean(heading),
        hasListItem: Boolean(listItem),
        hasCodeSurface: Boolean(codeSurface),
        hasHighlightedCode: Boolean(highlightedCode),
        hasPlaintextWrapper: Boolean(plaintextWrapper),
      };
    });

    expect(streamingMetrics).toEqual({
      hasHeading: true,
      hasListItem: true,
      hasCodeSurface: true,
      hasHighlightedCode: false,
      hasPlaintextWrapper: false,
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "streaming-markdown",
        content: "\n```",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "streaming-markdown" },
    ], 1);

    await expect(page.getByTestId("assistant-message").locator("h2")).toContainText("正在整理");
    await expect(page.locator(".code-surface")).toHaveCount(1);
    await expect(page.locator(".code-surface")).toHaveAttribute("data-renderer", "highlighted");
    await expect(page.locator(".code-surface .shiki-wrapper")).toHaveCount(1);
  });

  test("long assistant replies expose a quiet section index for scanning", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "long-scanning" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "long-scanning",
        content: [
          "下面是一段较长的整理，用来验证长回复的扫读体验。".repeat(8),
          "",
          "## 结论",
          "",
          "先把对话阅读面收稳，再继续做更深的执行能力。",
          "",
          "## 改动范围",
          "",
          "- 对话排版",
          "- diff 阅读",
          "- 工具证据",
          "",
          "## 验收方式",
          "",
          "用 demo 文件夹跑一轮小改动，然后观察 diff、工具和总结。",
          "",
          "## 后续",
          "",
          "继续处理复制、打开和定位的一致性。",
        ].join("\n"),
      },
      { event_type: "text_end", session_id: sessionId, block_id: "long-scanning" },
    ], 1);

    const assistant = page.getByTestId("assistant-message");
    const index = assistant.getByTestId("answer-section-index");
    await expect(index).toBeVisible();
    await expect(index.getByText("回复结构")).toBeVisible();
    await expect(index.getByRole("link", { name: "结论" })).toBeVisible();
    await expect(index.getByRole("link", { name: "改动范围" })).toBeVisible();
    await expect(index.getByRole("link", { name: "验收方式" })).toBeVisible();

    const metrics = await index.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        height: Math.round(node.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
      };
    });

    expect(metrics.height).toBeLessThanOrEqual(34);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
  });

  test("message stream uses one gap token without component margins", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "gap-a" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "gap-a", content: "第一条回复。" },
      { event_type: "text_end", session_id: sessionId, block_id: "gap-a" },
      {
        event_type: "tool_call",
        session_id: sessionId,
        block_id: "gap-tool",
        tool_name: "read_file",
        tool_input: { path: "src/App.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "gap-tool",
        result: "ok",
        is_error: false,
        duration_ms: 50,
      },
    ], 1);

    const layout = await page.evaluate(() => {
      const root = document.documentElement;
      const lane = document.querySelector("[data-testid='message-lane']");
      const blocks = [...document.querySelectorAll("[data-testid='message-block']")];
      if (!lane || blocks.length < 2) return null;
      const laneStyle = getComputedStyle(lane);
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-message-gap").trim(),
        gap: Math.round(Number.parseFloat(laneStyle.rowGap)),
        margins: blocks.map((block) => {
          const style = getComputedStyle(block);
          return {
            top: Math.round(Number.parseFloat(style.marginTop)),
            bottom: Math.round(Number.parseFloat(style.marginBottom)),
          };
        }),
      };
    });

    expect(layout).not.toBeNull();
    expect(layout!.token).toBe("14px");
    expect(layout!.gap).toBe(14);
    expect(layout!.margins.every((margin) => margin.top === 0 && margin.bottom === 0)).toBeTruthy();
  });

  test("conversation turns create hidden work structure without workflow chrome", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("把 demo 输入框收得更像正式产品。");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call",
        session_id: sessionId,
        block_id: "turn-tool",
        tool_name: "read_file",
        tool_input: { path: "src/InputBar.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "turn-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
      { event_type: "text_start", session_id: sessionId, block_id: "turn-result-a" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "turn-result-a",
        content: "我先只动 demo。输入框已经收了一版，重点看边框、背景和长文本时的稳定性。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-result-a" },
    ], 1);

    await page.locator("textarea").fill("再看一下失败状态。");
    await page.locator("textarea").press("Enter");
    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "turn-result-b" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "turn-result-b",
        content: "失败状态这轮先保持轻提示，不额外加确认流程。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "turn-result-b" },
    ], 1);

    const turns = page.getByTestId("conversation-turn");
    await expect(turns).toHaveCount(2);
    await expect(turns.nth(0)).toHaveAttribute("data-turn-shape", "with-evidence");
    await expect(turns.nth(1)).toHaveAttribute("data-turn-shape", "direct");
    await expect(turns.nth(0).getByTestId("user-message")).toContainText("把 demo 输入框");
    await expect(turns.nth(0).getByTestId("tool-card-trigger")).toBeVisible();
    await expect(turns.nth(0).getByTestId("assistant-message")).toContainText("我先只动 demo");
    await expect(turns.nth(1).getByTestId("user-message")).toContainText("失败状态");
    await expect(turns.nth(1).getByTestId("assistant-message")).toContainText("轻提示");

    await expect(page.getByText("用户意图", { exact: true })).toHaveCount(0);
    await expect(page.getByText("Forge 理解", { exact: true })).toHaveCount(0);
    await expect(page.getByText("结果与下一步", { exact: true })).toHaveCount(0);

    const metrics = await page.evaluate(() => {
      const turnNodes = [...document.querySelectorAll("[data-testid='conversation-turn']")];
      if (turnNodes.length < 2) return null;
      const firstStyle = getComputedStyle(turnNodes[0]);
      const secondStyle = getComputedStyle(turnNodes[1]);
      return {
        rowGap: Math.round(Number.parseFloat(firstStyle.rowGap)),
        secondPaddingTop: Math.round(Number.parseFloat(secondStyle.paddingTop)),
        firstBackground: firstStyle.backgroundColor,
        firstBorderTop: Math.round(Number.parseFloat(firstStyle.borderTopWidth)),
        firstRadius: Number.parseFloat(firstStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.rowGap).toBe(14);
    expect(metrics!.secondPaddingTop).toBeGreaterThanOrEqual(16);
    expect(metrics!.firstBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.firstBorderTop).toBe(0);
    expect(metrics!.firstRadius).toBe(0);
  });

  test("tool and shell logs share compact row rhythm", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call",
        session_id: sessionId,
        block_id: "compact-tool",
        tool_name: "read_file",
        tool_input: { path: "src/App.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "compact-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
      { event_type: "shell_start", session_id: sessionId, block_id: "compact-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "compact-shell", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "compact-shell", exit_code: 0 },
    ], 1);

    await page.getByTestId("tool-activity-summary").click();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const group = document.querySelector("[data-testid='tool-activity-group']");
      const summary = document.querySelector("[data-testid='tool-activity-summary']");
      const list = document.querySelector(".forge-tool-activity-list");
      const tool = document.querySelector("[data-testid='tool-card-trigger']");
      const shell = document.querySelector("[data-testid='shell-card-trigger']");
      if (!group || !summary || !list || !tool || !shell) return null;
      const shellWrapper = shell.closest(".shell-reel");
      const shellBody = shell.closest(".shell-reel-body");
      const groupStyle = getComputedStyle(group);
      const summaryStyle = getComputedStyle(summary);
      const listStyle = getComputedStyle(list);
      const toolStyle = getComputedStyle(tool);
      const shellWrapperStyle = shellWrapper ? getComputedStyle(shellWrapper) : null;
      const shellBodyStyle = shellBody ? getComputedStyle(shellBody) : null;
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-log-row-height").trim(),
        groupWidth: Math.round(group.getBoundingClientRect().width),
        groupBorderLeft: Math.round(Number.parseFloat(groupStyle.borderLeftWidth)),
        summaryHeight: Math.round(summary.getBoundingClientRect().height),
        summaryDisplay: summaryStyle.display,
        summaryBackground: summaryStyle.backgroundColor,
        summaryBorderTop: Math.round(Number.parseFloat(summaryStyle.borderTopWidth)),
        listGap: Math.round(Number.parseFloat(listStyle.gap)),
        toolHeight: Math.round(tool.getBoundingClientRect().height),
        toolRadius: Number.parseFloat(toolStyle.borderTopLeftRadius),
        toolBorder: toolStyle.borderTopColor,
        toolBackground: toolStyle.backgroundColor,
        shellHeight: Math.round(shell.getBoundingClientRect().height),
        toolMargin: getComputedStyle(tool.parentElement as Element).marginBottom,
        shellMargin: getComputedStyle(shell.parentElement as Element).marginBottom,
        toolMeterCount: document.querySelectorAll(".tool-machine-meter").length,
        toolLedCount: document.querySelectorAll(".tool-machine-led").length,
        shellCapCount: document.querySelectorAll(".shell-reel-cap").length,
        shellWrapperMarginTop: shellWrapperStyle ? Math.round(Number.parseFloat(shellWrapperStyle.marginTop)) : -1,
        shellWrapperBackground: shellWrapperStyle?.backgroundColor ?? "",
        shellBodyRadius: shellBodyStyle ? Number.parseFloat(shellBodyStyle.borderTopLeftRadius) : 0,
        shellBodyBorder: shellBodyStyle?.borderTopColor ?? "",
        shellBodyBackground: shellBodyStyle?.backgroundColor ?? "",
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.token).toBe("22px");
    expect(metrics!.groupWidth).toBeLessThanOrEqual(760);
    expect(metrics!.groupBorderLeft).toBe(0);
    expect(metrics!.summaryHeight).toBeLessThanOrEqual(24);
    expect(metrics!.summaryDisplay).toBe("inline-flex");
    expect(metrics!.summaryBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.summaryBorderTop).toBe(0);
    expect(metrics!.listGap).toBe(2);
    expect(metrics!.toolHeight).toBe(44);
    expect(metrics!.toolRadius).toBeLessThanOrEqual(8);
    expect(metrics!.toolBorder).toBe("rgb(216, 203, 184)");
    expect(metrics!.toolBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.shellHeight).toBe(44);
    expect(metrics!.toolMargin).toBe("0px");
    expect(metrics!.shellMargin).toBe("0px");
    expect(metrics!.toolMeterCount).toBe(0);
    expect(metrics!.toolLedCount).toBe(0);
    expect(metrics!.shellCapCount).toBe(0);
    expect(metrics!.shellWrapperMarginTop).toBe(0);
    expect(metrics!.shellWrapperBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.shellBodyRadius).toBeLessThanOrEqual(8);
    expect(metrics!.shellBodyBorder).toBe("rgb(216, 203, 184)");
    expect(metrics!.shellBodyBackground).not.toBe("rgba(0, 0, 0, 0)");
  });

  test("process rows keep long evidence and status anchored", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.setViewportSize({ width: 900, height: 680 });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call_start",
        session_id: sessionId,
        block_id: "anchored-tool",
        tool_name: "read_file",
        tool_input: {
          path: "src/components/messages/process-feedback/very-long-local-evidence-path-that-should-not-push-status-out-of-view.tsx",
        },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "anchored-tool",
        result: "ok",
        is_error: false,
        duration_ms: 1328,
      },
      {
        event_type: "shell_start",
        session_id: sessionId,
        block_id: "anchored-shell",
        command: "npm run build -- --mode production --workspace src/components/messages/process-feedback/very-long-command-name-with-flags",
      },
      { event_type: "shell_output", session_id: sessionId, block_id: "anchored-shell", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "anchored-shell", exit_code: 0 },
    ], 1);

    await page.getByTestId("tool-activity-summary").click();

    const metrics = await page.evaluate(() => {
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const tool = document.querySelector<HTMLElement>("[data-testid='tool-card-trigger']");
      const shell = document.querySelector<HTMLElement>("[data-testid='shell-card-trigger']");
      const toolInput = tool?.querySelector<HTMLElement>(".forge-log-line-input");
      const toolDuration = tool?.querySelector<HTMLElement>(".forge-log-line-duration");
      const toolStatus = tool?.querySelector<HTMLElement>(".forge-log-line-status");
      const shellCommand = shell?.querySelector<HTMLElement>(".forge-log-line-command");
      const shellStatus = shell?.querySelector<HTMLElement>(".forge-log-line-status");
      if (!lane || !tool || !shell || !toolInput || !toolDuration || !toolStatus || !shellCommand || !shellStatus) return null;

      const toolInputStyle = getComputedStyle(toolInput);
      const shellCommandStyle = getComputedStyle(shellCommand);
      const laneWidth = Math.round(lane.getBoundingClientRect().width);
      const toolRect = tool.getBoundingClientRect();
      const shellRect = shell.getBoundingClientRect();
      const toolStatusRect = toolStatus.getBoundingClientRect();
      const shellStatusRect = shellStatus.getBoundingClientRect();

      return {
        laneWidth,
        toolWidth: Math.round(toolRect.width),
        shellWidth: Math.round(shellRect.width),
        toolInputOverflow: toolInputStyle.overflow,
        toolInputTextOverflow: toolInputStyle.textOverflow,
        toolInputWhiteSpace: toolInputStyle.whiteSpace,
        shellCommandOverflow: shellCommandStyle.overflow,
        shellCommandTextOverflow: shellCommandStyle.textOverflow,
        shellCommandWhiteSpace: shellCommandStyle.whiteSpace,
        toolStatusRightGap: Math.round(toolRect.right - toolStatusRect.right),
        shellStatusRightGap: Math.round(shellRect.right - shellStatusRect.right),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.toolWidth).toBeLessThanOrEqual(metrics!.laneWidth);
    expect(metrics!.shellWidth).toBeLessThanOrEqual(metrics!.laneWidth);
    expect(metrics!.toolInputOverflow).toBe("hidden");
    expect(metrics!.toolInputTextOverflow).toBe("ellipsis");
    expect(metrics!.toolInputWhiteSpace).toBe("nowrap");
    expect(metrics!.shellCommandOverflow).toBe("hidden");
    expect(metrics!.shellCommandTextOverflow).toBe("ellipsis");
    expect(metrics!.shellCommandWhiteSpace).toBe("nowrap");
    expect(metrics!.toolStatusRightGap).toBeLessThanOrEqual(1);
    expect(metrics!.shellStatusRightGap).toBeLessThanOrEqual(12);
  });

  test("delegate task trace uses shared warm process material", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call_start",
        session_id: sessionId,
        block_id: "delegate-trace",
        tool_name: "delegate_task",
        tool_input: { prompt: "检查消息渲染和过程反馈材料一致性" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "delegate-trace",
        result: JSON.stringify({
          result: "完成检查：SubAgentTrace 使用共享过程材料，长结果保持在消息 lane 内部滚动。",
          steps: [
            {
              round: 0,
              thinking: "先确认现有样式是否仍有硬编码冷色。",
              text: "找到了一个可以收进 token 系统的 trace 片段。",
              tool_calls: [
                {
                  name: "read_file",
                  input: "src/components/messages/SubAgentTrace.tsx",
                  result: "SubAgentTrace contains a long delegate result that should wrap quietly inside the raised process material without using cold debug text colors.",
                },
              ],
            },
          ],
        }),
        is_error: false,
        duration_ms: 864,
      },
    ], 1);

    await page.getByTestId("tool-card-trigger").click();
    await expect(page.getByTestId("sub-agent-trace")).toBeVisible();
    await page.getByTestId("sub-agent-round-trigger").click();
    await page.getByTestId("sub-agent-tool-trigger").click();

    const metrics = await page.evaluate(() => {
      const rootStyle = getComputedStyle(document.documentElement);
      const resolveColor = (color: string) => {
        const probe = document.createElement("span");
        probe.style.color = color;
        document.body.append(probe);
        const resolved = getComputedStyle(probe).color;
        probe.remove();
        return resolved;
      };
      const trace = document.querySelector<HTMLElement>("[data-testid='sub-agent-trace']");
      const rounds = document.querySelector<HTMLElement>(".forge-sub-agent-rounds");
      const result = document.querySelector<HTMLElement>("[data-testid='sub-agent-result']");
      const toolResult = document.querySelector<HTMLElement>("[data-testid='sub-agent-tool-result']");
      if (!trace || !rounds || !result || !toolResult) return null;

      const traceStyle = getComputedStyle(trace);
      const roundsStyle = getComputedStyle(rounds);
      const resultStyle = getComputedStyle(result);
      const toolResultStyle = getComputedStyle(toolResult);

      return {
        materialRaised: rootStyle.getPropertyValue("--forge-material-raised").trim(),
        materialSurface: rootStyle.getPropertyValue("--forge-material-surface").trim(),
        materialBorder: resolveColor(rootStyle.getPropertyValue("--forge-material-border").trim()),
        traceBackground: traceStyle.backgroundColor,
        traceBorder: traceStyle.borderTopColor,
        traceRadius: Number.parseFloat(traceStyle.borderTopLeftRadius),
        roundsBorder: roundsStyle.borderBottomColor,
        resultOverflow: resultStyle.overflow,
        resultOverflowWrap: resultStyle.overflowWrap,
        resultMaxHeight: resultStyle.maxHeight,
        toolResultBackground: toolResultStyle.backgroundColor,
        toolResultOverflowWrap: toolResultStyle.overflowWrap,
        inlineStyleCount: trace.querySelectorAll("[style]").length + (trace.hasAttribute("style") ? 1 : 0),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.traceBackground).toBe(metrics!.materialRaised);
    expect(metrics!.traceBorder).toBe(metrics!.materialBorder);
    expect(metrics!.traceRadius).toBeLessThanOrEqual(8);
    expect(metrics!.roundsBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.resultOverflow).toBe("auto");
    expect(metrics!.resultOverflowWrap).toBe("anywhere");
    expect(metrics!.resultMaxHeight).toBe("200px");
    expect(metrics!.toolResultBackground).toBe(metrics!.materialSurface);
    expect(metrics!.toolResultOverflowWrap).toBe("anywhere");
    expect(metrics!.inlineStyleCount).toBe(0);
  });

  test("tool activity summary keeps dense process evidence on one quiet line", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.setViewportSize({ width: 860, height: 720 });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-read-a", tool_name: "read_file", tool_input: { path: "src/components/session/InputBar.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-read-a", result: "ok", is_error: false, duration_ms: 31 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-read-b", tool_name: "read_file", tool_input: { path: "src/styles/globals.css" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-read-b", result: "ok", is_error: false, duration_ms: 38 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-search", tool_name: "search_content", tool_input: { pattern: "forge-composer", path: "src" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-search", result: "ok", is_error: false, duration_ms: 48 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-edit", tool_name: "edit", tool_input: { path: "src/components/messages/ToolActivityGroup.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-edit", result: "ok", is_error: false, duration_ms: 82 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "dense-web", tool_name: "web_fetch", tool_input: { url: "https://example.com/very/long/reference/path" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "dense-web", result: "ok", is_error: false, duration_ms: 93 },
      { event_type: "shell_start", session_id: sessionId, block_id: "dense-check", command: "npm run build -- --mode production" },
      { event_type: "shell_output", session_id: sessionId, block_id: "dense-check", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "dense-check", exit_code: 0 },
    ], 1);

    const metrics = await page.getByTestId("tool-activity-summary").evaluate((node) => {
      const summary = node as HTMLElement;
      const lane = document.querySelector<HTMLElement>("[data-testid='message-lane']");
      const items = Array.from(summary.querySelectorAll<HTMLElement>(".forge-tool-activity-summary-item"));
      const summaryStyle = getComputedStyle(summary);
      const itemStyles = items.map((item) => {
        const style = getComputedStyle(item);
        return {
          overflow: style.overflow,
          textOverflow: style.textOverflow,
          whiteSpace: style.whiteSpace,
          width: Math.round(item.getBoundingClientRect().width),
        };
      });
      return {
        itemCount: items.length,
        summaryHeight: Math.round(summary.getBoundingClientRect().height),
        summaryWidth: Math.round(summary.getBoundingClientRect().width),
        laneWidth: lane ? Math.round(lane.getBoundingClientRect().width) : 0,
        overflow: summaryStyle.overflow,
        whiteSpace: summaryStyle.whiteSpace,
        itemStyles,
      };
    });

    expect(metrics.itemCount).toBeGreaterThanOrEqual(5);
    expect(metrics.summaryHeight).toBeLessThanOrEqual(28);
    expect(metrics.summaryWidth).toBeLessThanOrEqual(metrics.laneWidth);
    expect(metrics.overflow).toBe("hidden");
    expect(metrics.whiteSpace).toBe("nowrap");
    expect(metrics.itemStyles.every((item) => item.overflow === "hidden")).toBe(true);
    expect(metrics.itemStyles.every((item) => item.textOverflow === "ellipsis")).toBe(true);
    expect(metrics.itemStyles.every((item) => item.whiteSpace === "nowrap")).toBe(true);
  });

  test("expanded logs share one detail surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "tool_call",
        session_id: sessionId,
        block_id: "detail-tool",
        tool_name: "read_file",
        tool_input: { path: "src/App.tsx" },
      },
      {
        event_type: "tool_call_result",
        session_id: sessionId,
        block_id: "detail-tool",
        result: "ok",
        is_error: false,
        duration_ms: 42,
      },
      { event_type: "shell_start", session_id: sessionId, block_id: "detail-shell", command: "npm run build" },
      {
        event_type: "shell_output",
        session_id: sessionId,
        block_id: "detail-shell",
        content: "stdout:\n/Users/cabbos/project/forge-test-app/src/components/really-long-output-path-with-build-artifacts.tsx:42: done",
      },
      { event_type: "shell_end", session_id: sessionId, block_id: "detail-shell", exit_code: 0 },
    ], 1);

    await page.getByTestId("tool-activity-summary").click();
    await page.getByTestId("tool-card-trigger").click();
    await page.getByTestId("shell-card-trigger").click();

    const surfaces = await page.evaluate(() => {
      const root = document.documentElement;
      return [...document.querySelectorAll("[data-testid='log-detail-surface']")].map((surface) => {
        const style = getComputedStyle(surface);
        const header = surface.querySelector("[data-testid='log-detail-header']");
        const output = surface.querySelector("[data-testid='log-detail-output']");
        const shellPre = surface.querySelector(".forge-shell-output-section pre");
        const outputStyle = output ? getComputedStyle(output) : null;
        const shellPreStyle = shellPre ? getComputedStyle(shellPre) : null;
        return {
          maxHeightToken: getComputedStyle(root).getPropertyValue("--forge-log-output-max-height").trim(),
          radius: Number.parseFloat(style.borderTopLeftRadius),
          headerHeight: header ? Math.round(header.getBoundingClientRect().height) : 0,
          detailBackground: style.backgroundColor,
          detailBoxShadow: style.boxShadow,
          outputMaxHeight: outputStyle?.maxHeight ?? "",
          outputPaddingTop: outputStyle ? Math.round(Number.parseFloat(outputStyle.paddingTop)) : 0,
          outputFontSize: outputStyle ? Number.parseFloat(outputStyle.fontSize) : 0,
          outputWordBreak: outputStyle?.wordBreak ?? "",
          shellPreWordBreak: shellPreStyle?.wordBreak ?? "",
          shellPreOverflowWrap: shellPreStyle?.overflowWrap ?? "",
        };
      });
    });

    expect(surfaces).toHaveLength(2);
    expect(surfaces.every((surface) => surface.maxHeightToken === "220px")).toBeTruthy();
    expect(surfaces.every((surface) => surface.radius <= 8)).toBeTruthy();
    expect(surfaces.every((surface) => surface.headerHeight === 32)).toBeTruthy();
    expect(surfaces.every((surface) => surface.detailBackground !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(surfaces.every((surface) => surface.detailBoxShadow !== "none")).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputMaxHeight === "220px")).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputPaddingTop === 7)).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputFontSize <= 11.5)).toBeTruthy();
    expect(surfaces.every((surface) => surface.outputWordBreak === "normal")).toBeTruthy();
    const shellSurface = surfaces.find((surface) => surface.shellPreWordBreak);
    expect(shellSurface?.shellPreWordBreak).toBe("normal");
    expect(shellSurface?.shellPreOverflowWrap).toBe("anywhere");
  });

  test("context compaction notice follows message rhythm", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "context_compacted",
        session_id: sessionId,
        block_id: "compact-notice",
        summary: "整理后的上下文摘要",
        compacted_messages: 12,
        retained_messages: 4,
        estimated_tokens_before: 124000,
        estimated_tokens_after: 42000,
      },
    ], 1);

    const metrics = await page.evaluate(() => {
      const trigger = document.querySelector("[data-testid='context-compact-trigger']");
      if (!trigger) return null;
      const wrapper = trigger.closest(".compact-spool");
      const wrapperStyle = wrapper ? getComputedStyle(wrapper) : null;
      const wrapperAfter = wrapper ? getComputedStyle(wrapper, "::after") : null;
      const triggerStyle = getComputedStyle(trigger);
      const meta = trigger.querySelector(".compact-spool-meta");
      const metaStyle = meta ? getComputedStyle(meta) : null;
      return {
        height: Math.round(trigger.getBoundingClientRect().height),
        marginTop: wrapperStyle ? Math.round(Number.parseFloat(wrapperStyle.marginTop)) : -1,
        marginBottom: wrapperStyle ? Math.round(Number.parseFloat(wrapperStyle.marginBottom)) : -1,
        wrapperBackground: wrapperStyle?.backgroundColor ?? "",
        wrapperBorderTop: wrapperStyle?.borderTopWidth ?? "",
        wrapperAfterContent: wrapperAfter?.content ?? "",
        wrapperAfterHeight: wrapperAfter?.height ?? "",
        triggerBackground: triggerStyle.backgroundColor,
        triggerBorderTop: triggerStyle.borderTopWidth,
        triggerRadius: Number.parseFloat(triggerStyle.borderTopLeftRadius),
        metaColor: metaStyle?.color ?? "",
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.height).toBe(28);
    expect(metrics!.marginTop).toBe(0);
    expect(metrics!.marginBottom).toBe(0);
    expect(metrics!.wrapperBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.wrapperBorderTop).toBe("0px");
    expect(metrics!.wrapperAfterContent).toBe("none");
    expect(metrics!.wrapperAfterHeight).toBe("auto");
    expect(metrics!.triggerBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.triggerBorderTop).toBe("1px");
    expect(metrics!.triggerRadius).toBeLessThanOrEqual(8);
    expect(metrics!.metaColor).toBe("rgb(95, 93, 85)");
  });

  test("composer uses a grounded editor surface instead of a plastic card", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const surface = page.getByTestId("composer-surface");
    const send = surface.getByRole("button", { name: "发送" });
    await expect(surface).toBeVisible();
    await expect(send).toBeVisible();
    await expect(surface.getByTestId("composer-tool-cluster")).toBeVisible();
    await expect(surface.getByTestId("composer-control-cluster")).toBeVisible();
    await expect(surface.getByTestId("composer-model-indicator")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const surface = document.querySelector("[data-testid='composer-surface']");
      const toolbar = document.querySelector("[data-testid='composer-toolbar']");
      const toolCluster = document.querySelector("[data-testid='composer-tool-cluster']");
      const controlCluster = document.querySelector("[data-testid='composer-control-cluster']");
      const model = document.querySelector("[data-testid='composer-model-chip']");
      const send = document.querySelector("[data-testid='composer-send']");
      const tools = Array.from(document.querySelectorAll("[data-testid='composer-tool-button']"));
      if (!surface || !toolbar || !toolCluster || !controlCluster || !model || !send) return null;
      const surfaceStyle = getComputedStyle(surface);
      const modelStyle = getComputedStyle(model);
      const sendStyle = getComputedStyle(send);
      return {
        surfaceBackdrop: surfaceStyle.backdropFilter || surfaceStyle.getPropertyValue("-webkit-backdrop-filter"),
        surfaceOverflow: surfaceStyle.overflow,
        surfaceShadow: surfaceStyle.boxShadow,
        surfaceRadius: Number.parseFloat(surfaceStyle.borderTopLeftRadius),
        toolbarHeight: Math.round(toolbar.getBoundingClientRect().height),
        toolClusterHeight: Math.round(toolCluster.getBoundingClientRect().height),
        controlGap: Math.round(Number.parseFloat(getComputedStyle(controlCluster).columnGap)),
        toolSizes: tools.map((item) => ({
          width: Math.round(item.getBoundingClientRect().width),
          height: Math.round(item.getBoundingClientRect().height),
        })),
        modelRadius: Number.parseFloat(modelStyle.borderTopLeftRadius),
        modelHeight: Math.round(model.getBoundingClientRect().height),
        modelBackground: modelStyle.backgroundColor,
        sendRadius: Number.parseFloat(sendStyle.borderTopLeftRadius),
        sendBackground: sendStyle.backgroundColor,
        sendWidth: Math.round(send.getBoundingClientRect().width),
        sendHeight: Math.round(send.getBoundingClientRect().height),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.surfaceBackdrop).not.toBe("none");
    expect(metrics!.surfaceOverflow).toBe("hidden");
    expect(metrics!.surfaceShadow).not.toBe("none");
    expect(metrics!.surfaceRadius).toBeLessThanOrEqual(8);
    expect(metrics!.toolbarHeight).toBeLessThanOrEqual(40);
    expect(metrics!.toolClusterHeight).toBeLessThanOrEqual(32);
    expect(metrics!.controlGap).toBeLessThanOrEqual(8);
    expect(metrics!.toolSizes).toEqual([{ width: 30, height: 30 }, { width: 30, height: 30 }]);
    expect(metrics!.modelRadius).toBeLessThanOrEqual(8);
    expect(metrics!.modelHeight).toBe(30);
    expect(metrics!.modelBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.sendRadius).toBeLessThanOrEqual(8);
    expect(metrics!.sendBackground).not.toBe("rgb(184, 138, 86)");
    expect(metrics!.sendWidth).toBe(30);
    expect(metrics!.sendHeight).toBe(30);
  });

  test("design system materials stay subtle and token-driven", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      if (!composer) return false;
      const rootStyle = getComputedStyle(document.documentElement);
      const composerStyle = getComputedStyle(composer);
      return composerStyle.borderTopColor === rootStyle.getPropertyValue("--forge-material-border-focus").trim();
    });

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const titlebar = document.querySelector("[data-testid='app-titlebar']");
      const sidebar = document.querySelector("aside");
      const composer = document.querySelector("[data-testid='composer-surface']");
      if (!titlebar || !sidebar || !composer) return null;
      const rootStyle = getComputedStyle(root);
      const titlebarStyle = getComputedStyle(titlebar);
      const sidebarStyle = getComputedStyle(sidebar);
      const composerStyle = getComputedStyle(composer);
      return {
        borderSubtle: rootStyle.getPropertyValue("--forge-border-subtle").trim(),
        materialBorder: rootStyle.getPropertyValue("--forge-material-border").trim(),
        materialSurface: rootStyle.getPropertyValue("--forge-material-surface").trim(),
        materialRaised: rootStyle.getPropertyValue("--forge-material-raised").trim(),
        materialPopover: rootStyle.getPropertyValue("--forge-material-popover").trim(),
        materialOverlay: rootStyle.getPropertyValue("--forge-material-overlay").trim(),
        materialShadow: rootStyle.getPropertyValue("--forge-material-shadow").trim(),
        composerBorderToken: rootStyle.getPropertyValue("--forge-composer-border").trim(),
        composerBorderFocusToken: rootStyle.getPropertyValue("--forge-material-border-focus").trim(),
        composerSurface: rootStyle.getPropertyValue("--forge-composer-surface").trim(),
        composerSurfaceFocus: rootStyle.getPropertyValue("--forge-composer-surface-focus").trim(),
        composerShadowToken: rootStyle.getPropertyValue("--forge-composer-shadow").trim(),
        bgRaised: rootStyle.getPropertyValue("--forge-bg-raised").trim(),
        hover: rootStyle.getPropertyValue("--forge-hover").trim(),
        focusRing: rootStyle.getPropertyValue("--forge-focus-ring").trim(),
        titlebarBorder: titlebarStyle.borderBottomColor,
        sidebarBorder: sidebarStyle.borderRightColor,
        composerBorder: composerStyle.borderTopColor,
        composerBg: composerStyle.backgroundColor,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.borderSubtle).toBe("#D8CBB8");
    expect(metrics!.materialBorder).toBe("#D8CBB8");
    expect(metrics!.materialSurface).toBe("rgba(251, 247, 239, 0.96)");
    expect(metrics!.materialRaised).toBe("rgba(255, 252, 246, 0.94)");
    expect(metrics!.materialPopover).toBe("rgba(255, 252, 246, 0.99)");
    expect(metrics!.materialOverlay).toBe("rgba(251, 244, 234, 0.96)");
    expect(metrics!.materialShadow).toContain("0 16px 38px");
    expect(metrics!.composerBorderToken).toBe("#D8CBB8");
    expect(metrics!.composerSurface).toBe("rgba(255, 252, 246, 0.96)");
    expect(metrics!.composerShadowToken).toContain("0 18px 40px");
    expect(metrics!.bgRaised).toBe("#FFFCF6");
    expect(metrics!.hover).toBe("rgba(36, 42, 36, 0.055)");
    expect(metrics!.focusRing).toBe("rgba(196, 138, 58, 0.38)");
    expect(metrics!.titlebarBorder).toBe("rgb(216, 203, 184)");
    expect(metrics!.sidebarBorder).toBe("rgb(216, 203, 184)");
    expect(metrics!.composerBorder).toBe(metrics!.composerBorderFocusToken);
    expect(metrics!.composerBg).toBe(metrics!.composerSurfaceFocus);
  });

  test("desktop material baseline covers composer, process detail, popover, and archive", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "shell_start", session_id: sessionId, block_id: "material-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "material-shell", content: "done" },
      { event_type: "shell_end", session_id: sessionId, block_id: "material-shell", exit_code: 0 },
    ], 1);
    await page.getByTestId("shell-card-trigger").click();
    await openProjectArchive(page);
    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    await expect(page.getByRole("menu")).toBeVisible();
    await expect.poll(async () => page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      const materialBorderFocus = root.getPropertyValue("--forge-material-border-focus").trim();
      const colorProbe = document.createElement("span");
      document.body.append(colorProbe);
      colorProbe.style.color = materialBorderFocus;
      const expected = getComputedStyle(colorProbe).color;
      colorProbe.remove();
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      return composer ? getComputedStyle(composer).borderTopColor === expected : false;
    })).toBe(true);

    const metrics = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      const materialBorder = root.getPropertyValue("--forge-material-border").trim();
      const colorProbe = document.createElement("span");
      document.body.append(colorProbe);
      colorProbe.style.color = materialBorder;
      const materialBorderColor = getComputedStyle(colorProbe).color;
      colorProbe.remove();
      const materialBorderFocus = root.getPropertyValue("--forge-material-border-focus").trim();
      const materialSurface = root.getPropertyValue("--forge-material-surface").trim();
      const materialSurfaceFocus = root.getPropertyValue("--forge-material-surface-focus").trim();
      const materialRaised = root.getPropertyValue("--forge-material-raised").trim();
      const materialPopover = root.getPropertyValue("--forge-material-popover").trim();
      const materialOverlay = root.getPropertyValue("--forge-material-overlay").trim();
      const materialShadow = root.getPropertyValue("--forge-material-shadow").trim();
      const materialShadowStrong = root.getPropertyValue("--forge-material-shadow-strong").trim();
      const composerSurfaceFocus = root.getPropertyValue("--forge-composer-surface-focus").trim();
      const composerShadowFocus = root.getPropertyValue("--forge-composer-shadow-focus").trim();
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const detail = document.querySelector<HTMLElement>("[data-testid='log-detail-surface']");
      const menu = document.querySelector<HTMLElement>(".forge-composer-model-menu");
      const archive = document.querySelector<HTMLElement>("[data-testid='project-archive-panel']");
      const archiveHeader = archive?.querySelector<HTMLElement>(".forge-inspector-header");
      if (!composer || !detail || !menu || !archive || !archiveHeader) return null;
      const composerStyle = getComputedStyle(composer);
      const detailStyle = getComputedStyle(detail);
      const menuStyle = getComputedStyle(menu);
      const archiveStyle = getComputedStyle(archive);
      const archiveHeaderStyle = getComputedStyle(archiveHeader);
      return {
        materialBorder,
        materialBorderColor,
        materialBorderFocus,
        materialSurface,
        materialSurfaceFocus,
        materialRaised,
        materialPopover,
        materialOverlay,
        materialShadow,
        materialShadowStrong,
        composerSurfaceFocus,
        composerShadowFocus,
        composerBorder: composerStyle.borderTopColor,
        composerBackground: composerStyle.backgroundColor,
        composerShadow: composerStyle.boxShadow,
        detailBorder: detailStyle.borderTopColor,
        detailBackground: detailStyle.backgroundColor,
        menuBorder: menuStyle.borderTopColor,
        menuBackground: menuStyle.backgroundColor,
        archiveBorder: archiveStyle.borderLeftColor,
        archiveBackground: archiveStyle.backgroundColor,
        archiveHeaderHeight: Math.round(archiveHeader.getBoundingClientRect().height),
        archiveHeaderBorder: archiveHeaderStyle.borderBottomColor,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.composerBorder).toBe(metrics!.materialBorderFocus);
    expect(metrics!.composerBackground).toBe(metrics!.composerSurfaceFocus);
    expect(metrics!.materialShadowStrong).toContain("0 22px 52px");
    expect(metrics!.composerShadowFocus).toContain("0 22px 48px");
    expect(metrics!.composerShadow).toContain("22px 48px");
    expect(metrics!.detailBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.detailBackground).toBe(metrics!.materialRaised);
    expect(metrics!.menuBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.menuBackground).toBe(metrics!.materialPopover);
    expect(metrics!.archiveBorder).toBe(metrics!.materialBorderColor);
    expect(metrics!.archiveBackground).toBe(metrics!.materialOverlay);
    expect(metrics!.archiveHeaderHeight).toBe(42);
    expect(metrics!.archiveHeaderBorder).toBe(metrics!.materialBorderColor);
  });

  test("V3 color ladder keeps the light workbench readable", async ({ page }) => {
    await page.goto("http://localhost:1420");

    const tokens = await page.evaluate(() => {
      const root = getComputedStyle(document.documentElement);
      return {
        base: root.getPropertyValue("--forge-bg-base").trim(),
        depth: root.getPropertyValue("--forge-bg-depth").trim(),
        surface: root.getPropertyValue("--forge-bg-surface").trim(),
        raised: root.getPropertyValue("--forge-bg-raised").trim(),
        composer: root.getPropertyValue("--forge-bg-composer").trim(),
        muted: root.getPropertyValue("--forge-text-muted").trim(),
      };
    });

    expect(tokens).toEqual({
      base: "#F7F1E8",
      depth: "#E8DDCF",
      surface: "#FBF7EF",
      raised: "#FFFCF6",
      composer: "#FFFCF6",
      muted: "#5F5D55",
    });
  });

  test("core shell surfaces keep a restrained product radius", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.getByTitle("打开项目档案").click();
    const archive = projectArchive(page);
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();

    const radii = await page.evaluate(() => {
      const composer = document.querySelector("[data-testid='composer-lane'] > div:last-child");
      const archivePanel = [...document.querySelectorAll("aside:last-of-type section div")]
        .find((node) => node.textContent?.includes("项目概览"));
      return [composer, archivePanel]
        .filter(Boolean)
        .map((node) => Number.parseFloat(getComputedStyle(node as Element).borderTopLeftRadius));
    });

    expect(radii.length).toBeGreaterThanOrEqual(2);
    expect(radii.every((radius) => radius <= 8)).toBeTruthy();
  });

  test("write confirmation shows project boundary before approving", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "confirm-write-boundary",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          title: "准备修改项目",
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["src/App.tsx"],
          impact: "将修改 1 个文件",
          risk: "high",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: "这一步会覆盖现有文件，请确认路径与恢复点。",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(card.getByText("准备修改项目")).toBeVisible();
    await expect(card.getByText("目标项目", { exact: true })).toBeVisible();
    await expect(card.getByText("forge")).toBeVisible();
    await expect(card).not.toContainText("/Users/cabbos/project/forge");
    await expect(card.getByText("写入文件")).toBeVisible();
    await expect(card.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "继续" })).toBeVisible();
    await expect(card.getByRole("button", { name: "取消" })).toBeVisible();
    await expect(card.getByTestId("confirm-boundary-grid")).toBeVisible();
    await expect(card.getByTestId("confirm-boundary-row")).toHaveCount(5);
    await expect(card.getByTestId("confirm-warning")).toContainText("覆盖现有文件");
    await expect(card.getByTestId("confirm-action-bar")).toBeVisible();
    const confirmMetrics = await card.evaluate((node) => {
      const rows = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='confirm-boundary-row']"));
      const actionBar = node.querySelector<HTMLElement>("[data-testid='confirm-action-bar']");
      const primary = node.querySelector<HTMLElement>("[data-testid='confirm-approve']");
      const secondary = node.querySelector<HTMLElement>("[data-testid='confirm-cancel']");
      const warning = node.querySelector<HTMLElement>("[data-testid='confirm-warning']");
      const warningStyle = warning ? getComputedStyle(warning) : null;
      const before = getComputedStyle(node, "::before");
      const after = getComputedStyle(node, "::after");
      return {
        panelRadius: Number.parseFloat(getComputedStyle(node).borderTopLeftRadius),
        ticketWrappers: document.querySelectorAll(".permission-ticket").length,
        panelBefore: before.content,
        panelAfter: after.content,
        gridGap: Number.parseFloat(getComputedStyle(node.querySelector("[data-testid='confirm-boundary-grid']") as Element).rowGap),
        rowDisplay: rows[0] ? getComputedStyle(rows[0]).display : "",
        rowHeight: rows[0] ? Math.round(rows[0].getBoundingClientRect().height) : 0,
        warningRole: warning?.getAttribute("role") ?? "",
        warningHeight: warning ? Math.round(warning.getBoundingClientRect().height) : 0,
        warningRadius: warningStyle ? Number.parseFloat(warningStyle.borderTopLeftRadius) : 0,
        warningBackground: warningStyle?.backgroundColor ?? "",
        actionHeight: actionBar ? Math.round(actionBar.getBoundingClientRect().height) : 0,
        primaryHeight: primary ? Math.round(primary.getBoundingClientRect().height) : 0,
        secondaryHeight: secondary ? Math.round(secondary.getBoundingClientRect().height) : 0,
      };
    });
    expect(confirmMetrics.panelRadius).toBeLessThanOrEqual(8);
    expect(confirmMetrics.ticketWrappers).toBe(0);
    expect(confirmMetrics.panelBefore).toBe("none");
    expect(confirmMetrics.panelAfter).toBe("none");
    expect(confirmMetrics.gridGap).toBeLessThanOrEqual(2);
    expect(confirmMetrics.rowDisplay).toBe("grid");
    expect(confirmMetrics.rowHeight).toBeLessThanOrEqual(42);
    expect(confirmMetrics.warningRole).toBe("note");
    expect(confirmMetrics.warningHeight).toBeLessThanOrEqual(36);
    expect(confirmMetrics.warningRadius).toBeLessThanOrEqual(8);
    expect(confirmMetrics.warningBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(confirmMetrics.actionHeight).toBeLessThanOrEqual(42);
    expect(confirmMetrics.primaryHeight).toBe(28);
    expect(confirmMetrics.secondaryHeight).toBe(28);
  });

  test("resolved write confirmations collapse into quiet audit summaries", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "resolved-confirm-boundary",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          title: "准备修改项目",
          workspace_name: "forge-live-ops",
          workspace_path: "/Users/cabbos/project/forge-live-ops",
          operation: "write_file",
          affected_files: ["index.html"],
          impact: "1 个文件 · index.html",
          risk: "caution",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: "继续前确认改动范围。",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    const pendingHeight = await card.evaluate((node) => Math.round(node.getBoundingClientRect().height));
    await card.getByRole("button", { name: "继续" }).click();

    await expect(card).toHaveAttribute("data-confirm-state", "resolved");
    await expect(card.getByTestId("confirm-resolved-summary")).toBeVisible();
    await expect(card.getByText("已继续", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: "继续" })).toHaveCount(0);
    await expect(card.getByRole("button", { name: "取消" })).toHaveCount(0);
    await expect(card.getByTestId("confirm-boundary-grid")).toHaveCount(0);
    await expect(card.getByTestId("confirm-warning")).toHaveCount(0);
    await expect(card.getByTestId("confirm-action-bar")).toHaveCount(0);

    const metrics = await card.evaluate((node) => {
      const style = getComputedStyle(node);
      const header = node.querySelector<HTMLElement>(".forge-message-panel-header");
      const summary = node.querySelector<HTMLElement>("[data-testid='confirm-resolved-summary']");
      const status = node.querySelector<HTMLElement>(".forge-confirm-resolved");
      return {
        height: Math.round(node.getBoundingClientRect().height),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        headerHeight: header ? Math.round(header.getBoundingClientRect().height) : 0,
        summaryHeight: summary ? Math.round(summary.getBoundingClientRect().height) : 0,
        summaryDisplay: summary ? getComputedStyle(summary).display : "",
        statusHeight: status ? Math.round(status.getBoundingClientRect().height) : 0,
      };
    });

    expect(metrics.height).toBeLessThan(pendingHeight - 80);
    expect(metrics.height).toBeLessThanOrEqual(68);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.borderColor).not.toBe("rgba(184, 138, 86, 0.22)");
    expect(metrics.headerHeight).toBeLessThanOrEqual(32);
    expect(metrics.summaryHeight).toBeLessThanOrEqual(30);
    expect(metrics.summaryDisplay).toBe("flex");
    expect(metrics.statusHeight).toBeLessThanOrEqual(20);
  });

  test("write confirmation bounds long file paths and commands", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "confirm-long-command-boundary",
        question: "Allow run_shell?",
        kind: "shell_cmd",
        boundary: {
          title: "准备执行命令",
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "run_shell",
          affected_files: [
            "src/features/conversation/surfaces/really-long-generated-output-path-without-natural-breaks/ExtremelyLongComponentNameForRegressionCoverage.tsx",
          ],
          impact: "将检查构建输出",
          risk: "medium",
          recovery: "继续前会保留可检查的交付状态",
          command: "npm run build -- --filter=src/features/conversation/surfaces/really-long-generated-output-path-without-natural-breaks/ExtremelyLongComponentNameForRegressionCoverage.tsx",
          warning: "确认命令仍在当前项目内执行。",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "准备执行命令" });
    await expect(card.getByTestId("confirm-boundary-grid").getByText("执行命令", { exact: true })).toBeVisible();
    const metrics = await card.evaluate((node) => {
      const command = node.querySelector<HTMLElement>(".forge-confirm-command");
      const chip = node.querySelector<HTMLElement>(".forge-confirm-file-chip");
      const commandStyle = command ? getComputedStyle(command) : null;
      const chipStyle = chip ? getComputedStyle(chip) : null;
      return {
        panelScrollWidth: Math.round((node as HTMLElement).scrollWidth),
        panelClientWidth: Math.round((node as HTMLElement).clientWidth),
        commandOverflowX: commandStyle?.overflowX ?? "",
        commandWhiteSpace: commandStyle?.whiteSpace ?? "",
        commandScrollbarWidth: commandStyle?.scrollbarWidth ?? "",
        commandOverscrollX: commandStyle?.overscrollBehaviorX ?? "",
        chipOverflow: chipStyle?.overflowX ?? "",
        chipTextOverflow: chipStyle?.textOverflow ?? "",
        chipMaxWidth: chipStyle?.maxWidth ?? "",
      };
    });

    expect(metrics.panelScrollWidth).toBeLessThanOrEqual(metrics.panelClientWidth + 1);
    expect(metrics.commandOverflowX).toBe("auto");
    expect(metrics.commandWhiteSpace).toBe("pre");
    expect(metrics.commandScrollbarWidth).toBe("thin");
    expect(metrics.commandOverscrollX).toBe("contain");
    expect(metrics.chipOverflow).toBe("hidden");
    expect(metrics.chipTextOverflow).toBe("ellipsis");
    expect(metrics.chipMaxWidth).toBe("100%");
  });

  test("structured message panels use one compact conversation style", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "style-confirm",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          workspace_name: "forge",
          workspace_path: "/Users/cabbos/project/forge",
          operation: "write_file",
          affected_files: ["src/App.tsx"],
          risk_level: "low",
          checkpoint_status: "ready",
          command: null,
        },
      },
      {
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "style-diff",
        file_path: "src/App.tsx",
        old_content: "-old",
        new_content: "diff --git a/src/App.tsx b/src/App.tsx\n@@\n-old\n+new",
      },
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "style-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 5);

    const panels = page.getByTestId("message-panel");
    await expect(panels).toHaveCount(3);
    await expect(panels.filter({ hasText: "准备修改项目" })).toBeVisible();
    await expect(panels.filter({ hasText: "文件改动" })).toContainText("src/App.tsx");
    await expect(panels.filter({ hasText: "文件改动" }).getByRole("button", { name: "复制 diff" })).toBeVisible();
    await expect(panels.filter({ hasText: "本轮交付" })).toBeVisible();

    const widths = await panels.evaluateAll((nodes) =>
      nodes.map((node) => Math.round(node.getBoundingClientRect().width)),
    );
    expect(widths.every((width) => width <= 780)).toBeTruthy();

    const margins = await panels.evaluateAll((nodes) =>
      nodes.map((node) => {
        const style = getComputedStyle(node);
        return {
          top: Math.round(Number.parseFloat(style.marginTop)),
          bottom: Math.round(Number.parseFloat(style.marginBottom)),
        };
      }),
    );
    expect(margins.every((margin) => margin.top === 0 && margin.bottom === 0)).toBeTruthy();

    const deliveryMetrics = await panels.filter({ hasText: "本轮交付" }).evaluate((node) => {
      const grid = node.querySelector<HTMLElement>("[data-testid='delivery-summary-grid']");
      const items = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='delivery-summary-item']"));
      const values = Array.from(node.querySelectorAll<HTMLElement>(".forge-delivery-value"));
      const gridStyle = grid ? getComputedStyle(grid) : null;
      return {
        width: Math.round((node as HTMLElement).getBoundingClientRect().width),
        itemCount: items.length,
        gridColumnCount: gridStyle?.gridTemplateColumns.split(" ").filter(Boolean).length ?? 0,
        kinds: items.map((item) => item.dataset.deliveryKind ?? ""),
        valueText: values.map((value) => value.textContent?.trim() ?? ""),
        valueColors: values.map((value) => getComputedStyle(value).color),
        itemBackgrounds: items.map((item) => getComputedStyle(item).backgroundColor),
        itemBorders: items.map((item) => getComputedStyle(item).borderTopColor),
        minItemHeight: items.length ? Math.min(...items.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
      };
    });
    expect(deliveryMetrics.width).toBeLessThanOrEqual(720);
    expect(deliveryMetrics.itemCount).toBe(3);
    expect(deliveryMetrics.gridColumnCount).toBe(3);
    expect(deliveryMetrics.kinds).toEqual(["preview", "checkpoint", "next"]);
    expect(deliveryMetrics.valueText).toEqual(["预览未运行", "检查点已就绪", "下一步：检查当前版本。"]);
    expect(deliveryMetrics.valueColors.every((color) => color === "rgb(36, 42, 36)")).toBeTruthy();
    expect(deliveryMetrics.itemBackgrounds.every((color) => color !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(deliveryMetrics.itemBorders.every((color) => color !== "rgba(0, 0, 0, 0)")).toBeTruthy();
    expect(deliveryMetrics.minItemHeight).toBeGreaterThanOrEqual(52);
  });

  test("failed delivery check offers continue repair prompt", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "failed-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：先修复检查未通过的问题。",
          verification_label: "检查未通过",
          verification_status: "failed",
          verification_command: "npm run build",
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "本轮交付" });
    await expect(card.getByText("检查未通过", { exact: true })).toBeVisible();
    await expect(card).toHaveCSS("border-color", "rgba(212, 119, 119, 0.3)");
    const metrics = await card.evaluate((node) => {
      const grid = node.querySelector<HTMLElement>("[data-testid='delivery-summary-grid']");
      const items = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='delivery-summary-item']"));
      const actionBar = node.querySelector<HTMLElement>("[data-testid='delivery-action-bar']");
      const action = node.querySelector<HTMLElement>("[data-testid='delivery-primary-action']");
      const gridStyle = grid ? getComputedStyle(grid) : null;
      const actionStyle = action ? getComputedStyle(action) : null;
      return {
        width: Math.round(node.getBoundingClientRect().width),
        gridDisplay: gridStyle?.display ?? "",
        gridColumnCount: gridStyle?.gridTemplateColumns.split(" ").filter(Boolean).length ?? 0,
        itemCount: items.length,
        maxItemHeight: items.length ? Math.max(...items.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
        actionBarHeight: actionBar ? Math.round(actionBar.getBoundingClientRect().height) : 0,
        actionHeight: action ? Math.round(action.getBoundingClientRect().height) : 0,
        actionRadius: actionStyle ? Number.parseFloat(actionStyle.borderTopLeftRadius) : 0,
        actionBackground: actionStyle?.backgroundColor ?? "",
      };
    });
    expect(metrics.width).toBeLessThanOrEqual(720);
    expect(metrics.gridDisplay).toBe("grid");
    expect(metrics.gridColumnCount).toBeLessThanOrEqual(metrics.itemCount);
    expect(metrics.maxItemHeight).toBeLessThanOrEqual(72);
    expect(metrics.actionBarHeight).toBeLessThanOrEqual(42);
    expect(metrics.actionHeight).toBe(28);
    expect(metrics.actionRadius).toBeLessThanOrEqual(8);
    expect(metrics.actionBackground).not.toBe("rgba(0, 0, 0, 0)");
    await card.getByRole("button", { name: "继续修复" }).click();

    await expect(page.locator("textarea")).toHaveValue(/npm run build/);
    await expect(page.locator("textarea")).toHaveValue(/继续修复/);
  });

  test("pending project record delivery opens project archive", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "record-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：交付状态可以继续验收。",
          record_label: "建议更新项目记录",
          record_status: "pending",
          record_target_pages: ["tasks.md", "log.md"],
        },
      },
    ], 5);

    const card = page.getByTestId("message-panel").filter({ hasText: "本轮交付" });
    await expect(card.getByText("自动记录")).toBeVisible();
    await card.getByRole("button", { name: "查看记录" }).click();

    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    await expect(projectArchive(page).getByTestId("archive-disclosure-records").getByRole("button", { name: /项目记录/ }).first()).toHaveAttribute("aria-expanded", "true");
  });

  test("diff views read like a professional patch surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const diffLines = [
      "diff --git a/src/components/App.tsx b/src/components/App.tsx",
      "index 1111111..2222222 100644",
      "--- a/src/components/App.tsx",
      "+++ b/src/components/App.tsx",
      "@@ -10,8 +10,32 @@ export function App() {",
      "-  return <div>demo</div>;",
      "+  return <main className=\"forge-shell\">",
      "+    <h1>Forge</h1>",
      "+  </main>;",
      " }",
      ...Array.from({ length: 34 }, (_, index) => `+  const line${index + 1} = ${index + 1};`),
    ].join("\n");

    await simulateStream(page, sessionId, [
      {
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "reader-diff",
        file_path: "src/components/App.tsx",
        old_content: "",
        new_content: diffLines,
      },
    ], 1);

    const diff = page.getByTestId("diff-card");
    await expect(diff).toBeVisible();
    await expect(diff.getByTestId("diff-file-path")).toHaveText("src/components/App.tsx");
    await expect(diff.getByTestId("diff-stat")).toContainText("+37");
    await expect(diff.getByTestId("diff-stat")).toContainText("-1");
    await expect(diff.getByTestId("diff-summary")).toContainText("1 个变更块");
    await expect(diff.getByTestId("diff-summary")).toContainText("首处第 10 行");
    await expect(diff.getByTestId("diff-summary")).toContainText("44 行");
    await expect(diff.getByRole("button", { name: "复制 diff" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "打开文件" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "定位首处改动" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "查看改动" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "展开完整改动" })).toHaveCount(0);
    await expect(diff.getByText("line34")).toHaveCount(0);

    const collapsedMetrics = await diff.evaluate((node) => {
      const panel = node.querySelector<HTMLElement>("[data-testid='message-panel']");
      const summary = node.querySelector<HTMLElement>("[data-testid='diff-summary']");
      const toggle = node.querySelector<HTMLElement>("[data-testid='diff-body-toggle']");
      const body = node.querySelector(".forge-diff-body");
      if (!panel || !summary || !toggle) return null;
      const panelStyle = getComputedStyle(panel);
      const summaryStyle = getComputedStyle(summary);
      return {
        openState: panel.dataset.diffOpen,
        bodyVisible: Boolean(body),
        summaryBorderBottom: Math.round(Number.parseFloat(summaryStyle.borderBottomWidth)),
        toggleHeight: Math.round(toggle.getBoundingClientRect().height),
        background: panelStyle.backgroundColor,
      };
    });

    expect(collapsedMetrics).not.toBeNull();
    expect(collapsedMetrics!.openState).toBe("false");
    expect(collapsedMetrics!.bodyVisible).toBe(false);
    expect(collapsedMetrics!.summaryBorderBottom).toBe(0);
    expect(collapsedMetrics!.toggleHeight).toBe(24);

    await diff.getByRole("button", { name: "查看改动" }).click();
    await expect(diff.getByRole("button", { name: "隐藏改动" })).toBeVisible();
    await expect(diff.getByRole("button", { name: "展开完整改动" })).toBeVisible();

    const metrics = await diff.evaluate((node) => {
      const panel = node.querySelector("[data-testid='message-panel']");
      const added = node.querySelector("[data-testid='diff-line-added']");
      const removed = node.querySelector("[data-testid='diff-line-removed']");
      const hunk = node.querySelector("[data-testid='diff-line-hunk']");
      const oldNo = node.querySelector("[data-testid='diff-line-old-number']");
      const newNo = node.querySelector("[data-testid='diff-line-new-number']");
      const body = node.querySelector(".forge-diff-body");
      const summary = node.querySelector("[data-testid='diff-summary']");
      const code = node.querySelector(".forge-diff-line-code");
      if (!panel || !added || !removed || !hunk || !oldNo || !newNo || !body || !summary || !code) return null;
      const panelStyle = getComputedStyle(panel);
      const addedStyle = getComputedStyle(added);
      const removedStyle = getComputedStyle(removed);
      const hunkStyle = getComputedStyle(hunk);
      const bodyStyle = getComputedStyle(body);
      const summaryStyle = getComputedStyle(summary);
      const codeStyle = getComputedStyle(code);
      const cardStyle = getComputedStyle(node);
      return {
        openState: (panel as HTMLElement).dataset.diffOpen,
        cardWidth: Math.round(panel.getBoundingClientRect().width),
        cardBackground: cardStyle.backgroundColor,
        perfRows: node.querySelectorAll(".diff-filmstrip-perf").length,
        grid: getComputedStyle(added).display,
        oldNumberWidth: Math.round(oldNo.getBoundingClientRect().width),
        newNumberWidth: Math.round(newNo.getBoundingClientRect().width),
        maxWidth: panelStyle.maxWidth,
        bodyMaxHeight: bodyStyle.maxHeight,
        summaryHeight: Math.round(summary.getBoundingClientRect().height),
        lineMinHeight: Math.round(Number.parseFloat(addedStyle.minHeight)),
        codePaddingLeft: Math.round(Number.parseFloat(codeStyle.paddingLeft)),
        addedBackground: addedStyle.backgroundColor,
        addedBorderLeft: addedStyle.borderLeftWidth,
        removedBackground: removedStyle.backgroundColor,
        removedBorderLeft: removedStyle.borderLeftWidth,
        hunkBorderTop: hunkStyle.borderTopWidth,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.openState).toBe("true");
    expect(metrics!.cardWidth).toBeLessThanOrEqual(760);
    expect(metrics!.cardBackground).toBe("rgba(255, 252, 246, 0.92)");
    expect(metrics!.perfRows).toBe(0);
    expect(metrics!.maxWidth).not.toBe("none");
    expect(metrics!.bodyMaxHeight).toBe("320px");
    expect(metrics!.summaryHeight).toBe(26);
    expect(metrics!.lineMinHeight).toBe(18);
    expect(metrics!.codePaddingLeft).toBeLessThanOrEqual(10);
    expect(metrics!.grid).toBe("grid");
    expect(metrics!.oldNumberWidth).toBeGreaterThanOrEqual(36);
    expect(metrics!.newNumberWidth).toBeGreaterThanOrEqual(36);
    expect(metrics!.addedBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.addedBorderLeft).toBe("0px");
    expect(metrics!.removedBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removedBorderLeft).toBe("0px");
    expect(metrics!.hunkBorderTop).toBe("1px");

    await diff.getByRole("button", { name: "展开完整改动" }).click();
    await expect(diff.getByText("line34")).toBeVisible();
  });

  test("diff file actions stay scoped to the active workspace", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.evaluate(async () => {
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "diff_view",
        session_id: sessionId,
        block_id: "workspace-bound-diff",
        file_path: "src/DemoApp.tsx",
        old_content: "",
        new_content: [
          "diff --git a/src/DemoApp.tsx b/src/DemoApp.tsx",
          "--- a/src/DemoApp.tsx",
          "+++ b/src/DemoApp.tsx",
          "@@ -2,1 +2,1 @@",
          "-old",
          "+new",
        ].join("\n"),
      },
    ], 1);

    const diff = page.getByTestId("diff-card");
    await diff.getByRole("button", { name: "打开文件" }).click();
    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastOpenFileArgs;
    })).toMatchObject({
      path: "src/DemoApp.tsx",
      sessionId,
      workingDir: projectPath,
    });

    await diff.getByRole("button", { name: "定位首处改动" }).click();
    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastPreviewFileArgs;
    })).toMatchObject({
      path: "src/DemoApp.tsx",
      line: 2,
      sessionId,
      workingDir: projectPath,
    });
  });

  test("consecutive tool activity becomes one process evidence group", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "tool-story-a" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "tool-story-a", content: "我先看一下项目结构。" },
      { event_type: "text_end", session_id: sessionId, block_id: "tool-story-a" },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "tool-read", tool_name: "read_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "tool-read", result: "export function App() {}", is_error: false, duration_ms: 32 },
      { event_type: "shell_start", session_id: sessionId, block_id: "tool-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "tool-shell", content: "stdout:\nBuild started\nstderr:\nError: Cannot find module\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "tool-shell", exit_code: 1 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "tool-write", tool_name: "write_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "tool-write", result: "权限不足：无法写入 src/App.tsx", is_error: true, duration_ms: 45 },
      { event_type: "text_start", session_id: sessionId, block_id: "tool-story-b" },
      { event_type: "text_chunk", session_id: sessionId, block_id: "tool-story-b", content: "问题在依赖解析上。" },
      { event_type: "text_end", session_id: sessionId, block_id: "tool-story-b" },
    ], 1);

    const group = page.getByTestId("tool-activity-group");
    await expect(group).toHaveCount(1);
    await expect(group.getByTestId("tool-activity-summary")).toHaveAttribute("aria-expanded", "true");
    await expect(group.getByText("处理遇到问题")).toBeVisible();
    await expect(group.getByText("3 步")).toBeVisible();
    await expect(group.getByText("已读取文件")).toBeVisible();
    await expect(group.getByTestId("shell-exit-code")).toHaveText("exit 1");
    await expect(group.getByTestId("tool-result-summary")).toContainText("权限不足");
    await expect(group.getByTestId("shell-output-section").filter({ hasText: "stderr" })).toContainText("Cannot find module");
    await expect(group.getByText("完成", { exact: true })).toHaveCount(0);

    const failureMetrics = await group.evaluate((node) => {
      const detail = node.querySelector<HTMLElement>("[data-testid='log-detail-surface']");
      const stderr = node.querySelector<HTMLElement>("[data-testid='shell-output-section'][data-tone='error']");
      const detailStyle = detail ? getComputedStyle(detail) : null;
      const stderrStyle = stderr ? getComputedStyle(stderr) : null;
      return {
        detailTone: detail?.getAttribute("data-tone") ?? "",
        detailBorder: detailStyle?.borderTopColor ?? "",
        stderrBackground: stderrStyle?.backgroundColor ?? "",
        stderrBorderLeft: stderrStyle ? Math.round(Number.parseFloat(stderrStyle.borderLeftWidth)) : 0,
        stderrPaddingLeft: stderrStyle ? Math.round(Number.parseFloat(stderrStyle.paddingLeft)) : 0,
        stderrRadius: stderrStyle ? Number.parseFloat(stderrStyle.borderTopLeftRadius) : 0,
      };
    });
    expect(failureMetrics.detailTone).toBe("error");
    expect(failureMetrics.detailBorder).not.toBe("rgba(210, 204, 190, 0.14)");
    expect(failureMetrics.stderrBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(failureMetrics.stderrBorderLeft).toBe(1);
    expect(failureMetrics.stderrPaddingLeft).toBeGreaterThanOrEqual(8);
    expect(failureMetrics.stderrRadius).toBeLessThanOrEqual(8);
  });

  test("successful tool activity collapses into one handled summary", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "tool_call_start", session_id: sessionId, block_id: "success-read", tool_name: "read_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "success-read", result: "export function App() {}", is_error: false, duration_ms: 22 },
      { event_type: "shell_start", session_id: sessionId, block_id: "success-shell", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "success-shell", content: "stdout:\nBuild complete\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "success-shell", exit_code: 0 },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "success-write", tool_name: "write_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "success-write", result: "ok", is_error: false, duration_ms: 31 },
    ], 1);

    const group = page.getByTestId("tool-activity-group");
    await expect(group).toHaveCount(1);
    const summary = group.getByTestId("tool-activity-summary");
    await expect(summary).toBeVisible();
    await expect(summary).toHaveAttribute("aria-expanded", "false");
    await expect(summary).toContainText("过程已收起 · 3 步");
    await expect(summary).toContainText("查看 1 个文件");
    await expect(summary).toContainText("运行 1 次检查");
    await expect(group.getByText("过程证据")).toHaveCount(0);
    await expect(group.getByText("已读取文件")).toHaveCount(0);
    await expect(group.getByText("npm run build")).toHaveCount(0);

    const metrics = await summary.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        height: Math.round(node.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
      };
    });
    expect(metrics.height).toBeLessThanOrEqual(28);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.background).toBe("rgba(0, 0, 0, 0)");

    await summary.click();
    await expect(summary).toHaveAttribute("aria-expanded", "true");
    await expect(group.getByText("已读取文件")).toBeVisible();
    await expect(group.getByText("npm run build")).toBeVisible();
  });

  test("user messages can carry pasted code paths and logs without breaking the lane", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill([
      "看一下 `src/App.tsx:12`，这里报错：",
      "",
      "```bash",
      "Error: Cannot find module '@/components/ReallyLongBrokenComponentNameThatShouldNotStretchTheBubble'",
      "    at src/App.tsx:12:3",
      "```",
    ].join("\n"));
    await page.locator("textarea").press("Enter");

    const userMessage = page.getByTestId("user-message").last();
    await expect(userMessage.locator(".code-surface")).toBeVisible();
    await expect(userMessage.locator(".forge-file-ref-name")).toHaveText("App.tsx");
    await expect(userMessage.locator(".forge-file-ref-line")).toHaveText("line 12");
    await expect(userMessage.locator(".forge-file-ref")).toHaveAttribute("title", "src/App.tsx:12");

    const metrics = await userMessage.evaluate((node) => {
      const bubble = node.getBoundingClientRect();
      const lane = document.querySelector("[data-testid='message-lane']")?.getBoundingClientRect();
      const code = node.querySelector(".code-surface");
      const codeScroll = node.querySelector(".code-scroll");
      if (!lane || !code || !codeScroll) return null;
      return {
        bubbleWidth: Math.round(bubble.width),
        laneWidth: Math.round(lane.width),
        codeWidth: Math.round(code.getBoundingClientRect().width),
        overflowX: getComputedStyle(codeScroll).overflowX,
        whiteSpace: getComputedStyle(node).whiteSpace,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.bubbleWidth).toBeLessThan(metrics!.laneWidth);
    expect(metrics!.codeWidth).toBeLessThanOrEqual(metrics!.bubbleWidth);
    expect(metrics!.overflowX).toBe("auto");
    expect(metrics!.whiteSpace).toBe("normal");
  });

  test("waiting and thinking states stay quiet but specific", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await holdSendInput(page);

    await page.locator("textarea").fill("继续优化等待状态");
    await page.locator("textarea").press("Enter");
    await expectHeldSendInput(page, "继续优化等待状态");

    const pending = page.getByTestId("pending-block");
    await expect(pending).toHaveText(/正在组织回答/);
    await expect(pending).toHaveAttribute("role", "status");
    await expect(pending).toHaveAttribute("aria-live", "polite");
    await expect(pending).toHaveAttribute("data-state", "running");
    await expect(pending.getByTestId("pending-dots")).toBeVisible();
    await expect(pending).toHaveCSS("border-top-width", "0px");
    const pendingMetrics = await pending.evaluate((node) => ({
      height: Math.round(node.getBoundingClientRect().height),
      minHeight: Math.round(Number.parseFloat(getComputedStyle(node).minHeight)),
      color: getComputedStyle(node).color,
      background: getComputedStyle(node).backgroundColor,
      borderTop: Math.round(Number.parseFloat(getComputedStyle(node).borderTopWidth)),
      fontSize: Number.parseFloat(getComputedStyle(node).fontSize),
      gap: Math.round(Number.parseFloat(getComputedStyle(node).columnGap)),
    }));

    await simulateStream(page, sessionId, [
      { event_type: "thinking_start", session_id: sessionId, block_id: "quiet-thinking" },
      { event_type: "thinking_chunk", session_id: sessionId, block_id: "quiet-thinking", content: "Need to inspect the failure before editing." },
    ], 1);

    const thinking = page.getByTestId("thinking-trigger");
    await expect(thinking).toHaveText(/正在梳理思路/);
    await expect(thinking).toHaveAttribute("data-state", "running");
    await expect(thinking.getByTestId("thinking-dots")).toBeVisible();

    const metrics = await page.evaluate(() => {
      const thinking = document.querySelector("[data-testid='thinking-trigger']");
      if (!thinking) return null;
      return {
        thinkingHeight: Math.round(thinking.getBoundingClientRect().height),
        thinkingMinHeight: Math.round(Number.parseFloat(getComputedStyle(thinking).minHeight)),
        thinkingColor: getComputedStyle(thinking).color,
        thinkingBackground: getComputedStyle(thinking).backgroundColor,
        thinkingBorderTop: Math.round(Number.parseFloat(getComputedStyle(thinking).borderTopWidth)),
        thinkingFontSize: Number.parseFloat(getComputedStyle(thinking).fontSize),
        thinkingGap: Math.round(Number.parseFloat(getComputedStyle(thinking).columnGap)),
      };
    });

    expect(metrics).not.toBeNull();
    expect(pendingMetrics.minHeight).toBe(22);
    expect(metrics!.thinkingMinHeight).toBe(22);
    expect(pendingMetrics.height).toBeGreaterThanOrEqual(22);
    expect(pendingMetrics.height).toBeLessThanOrEqual(24);
    expect(metrics!.thinkingHeight).toBeGreaterThanOrEqual(22);
    expect(metrics!.thinkingHeight).toBeLessThanOrEqual(24);
    expect(pendingMetrics.fontSize).toBeCloseTo(10.5);
    expect(metrics!.thinkingFontSize).toBeCloseTo(10.5);
    expect(pendingMetrics.gap).toBe(6);
    expect(metrics!.thinkingGap).toBe(6);
    expect(pendingMetrics.background).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.thinkingBackground).toBe("rgba(0, 0, 0, 0)");
    expect(pendingMetrics.borderTop).toBe(0);
    expect(metrics!.thinkingBorderTop).toBe(0);
    expect(pendingMetrics.color).toBe(metrics!.thinkingColor);
    await releaseHeldSendInput(page);
  });

  test("thinking block expands and shows content", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const thinkingId = crypto.randomUUID();
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "thinking_start", session_id: sessionId, block_id: thinkingId },
      { event_type: "thinking_chunk", session_id: sessionId, block_id: thinkingId, content: "I need to analyze the auth system first." },
      { event_type: "thinking_end", session_id: sessionId, block_id: thinkingId },
      { event_type: "text_start", session_id: sessionId, block_id: crypto.randomUUID() },
      { event_type: "text_chunk", session_id: sessionId, block_id: crypto.randomUUID(), content: "Done analyzing." },
      { event_type: "text_end", session_id: sessionId, block_id: crypto.randomUUID() },
    ], 30);

    // Thinking trigger should be visible
    const thinkingTrigger = page.getByRole("button", { name: /思考已收起/ });
    await expect(thinkingTrigger).toBeVisible({ timeout: 5000 });

    // Click to expand
    await thinkingTrigger.click();

    // Thinking content should be visible
    await expect(page.getByText("I need to analyze the auth system first.")).toBeVisible();
  });

  test("tool card shows running then done status", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const toolId = crypto.randomUUID();
    // Send tool_start first (running state)
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "tool_call_start", session_id: sessionId, block_id: toolId, tool_name: "read_file", tool_input: { path: "test.rs" } },
    ], 30);

    // Should show running status
    const runningTool = page.getByRole("button", { name: /正在读取文件/ });
    await expect(runningTool).toBeVisible({ timeout: 3000 });
    await expect(runningTool).toHaveAttribute("data-state", "running");
    await expect(page.getByText("进行中", { exact: true })).toHaveCount(0);

    // Send tool_result (done state)
    await simulateStream(page, sessionId, [
      { event_type: "tool_call_result", session_id: sessionId, block_id: toolId, result: "fn main() {}", is_error: false, duration_ms: 100 },
    ], 30);

    // Should show done
    const doneTool = page.getByRole("button", { name: /已读取文件/ });
    await expect(doneTool).toBeVisible({ timeout: 3000 });
    await expect(doneTool).toHaveAttribute("data-state", "done");
    await expect(doneTool).toContainText("100ms");
    await expect(page.getByText("完成", { exact: true })).toHaveCount(0);
  });

  test("shell card exposes a restrained running state before exit", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const shellId = crypto.randomUUID();
    await simulateStream(page, sessionId, [
      { event_type: "session_started", session_id: sessionId, agent_type: "deepseek", model: "deepseek-v4-flash" },
      { event_type: "shell_start", session_id: sessionId, block_id: shellId, command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: shellId, content: "stdout:\nbuilding..." },
    ], 30);

    const runningShell = page.getByTestId("shell-card-trigger");
    await expect(runningShell).toBeVisible({ timeout: 3000 });
    await expect(runningShell).toHaveAttribute("data-state", "running");
    const runningMetrics = await runningShell.evaluate((node) => {
      const status = node.querySelector<HTMLElement>(".forge-log-status");
      const style = getComputedStyle(node);
      const statusStyle = status ? getComputedStyle(status) : null;
      return {
        minHeight: Math.round(Number.parseFloat(getComputedStyle(node).minHeight)),
        background: style.backgroundColor,
        borderTop: Math.round(Number.parseFloat(style.borderTopWidth)),
        statusTone: status?.getAttribute("data-tone") ?? "",
        statusTitle: status?.getAttribute("title") ?? "",
        statusColor: statusStyle?.color ?? "",
      };
    });
    expect(runningMetrics.minHeight).toBe(44);
    expect(runningMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(runningMetrics.borderTop).toBe(1);
    expect(runningMetrics.statusTone).toBe("running");
    expect(runningMetrics.statusTitle).toBe("运行中");
    expect(runningMetrics.statusColor).not.toBe("rgb(184, 138, 86)");

    await simulateStream(page, sessionId, [
      { event_type: "shell_end", session_id: sessionId, block_id: shellId, exit_code: 0 },
    ], 30);

    await expect(runningShell).toHaveAttribute("data-state", "done");
    const doneTone = await runningShell.evaluate((node) =>
      node.querySelector<HTMLElement>(".forge-log-status")?.getAttribute("data-tone"),
    );
    expect(doneTone).toBe("success");
  });

  test("sidebar shows persistent navigation", async ({ page }) => {
    const sidebar = page.locator("aside").first();

    const width = (await sidebar.boundingBox())?.width ?? 0;
    expect(width).toBeGreaterThanOrEqual(212);
    expect(width).toBeLessThanOrEqual(240);
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "插件" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "自动化" })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "设置" })).toBeVisible();
    await expect(sidebar.getByText("当前工作空间", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("插件", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("自动化", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("设置", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByTestId("sidebar-primary-nav")).toBeVisible();
    const railMetrics = await sidebar.evaluate((node) => {
      const workspace = node.querySelector<HTMLElement>("[data-testid='workspace-trigger']");
      const primaryNav = node.querySelector<HTMLElement>("[data-testid='sidebar-primary-nav']");
      const primaryActions = Array.from(node.querySelectorAll<HTMLElement>("[data-testid='sidebar-primary-action']"));
      const brand = node.querySelector<HTMLElement>(".forge-sidebar-brand");
      const style = workspace ? getComputedStyle(workspace) : null;
      return {
        workspaceHeight: workspace ? Math.round(workspace.getBoundingClientRect().height) : 0,
        workspaceRadius: style ? Number.parseFloat(style.borderTopLeftRadius) : 0,
        workspaceBorder: style?.borderTopColor ?? "",
        workspaceBackground: style?.backgroundColor ?? "",
        primaryGap: primaryNav ? Math.round(Number.parseFloat(getComputedStyle(primaryNav).rowGap)) : -1,
        primaryActions: primaryActions.map((button) => Math.round(button.getBoundingClientRect().height)),
        primaryTransitions: primaryActions.map((button) => getComputedStyle(button).transitionProperty),
        brandHeight: brand ? Math.round(brand.getBoundingClientRect().height) : 0,
      };
    });
    expect(railMetrics.workspaceHeight).toBe(32);
    expect(railMetrics.workspaceRadius).toBeLessThanOrEqual(8);
    expect(railMetrics.workspaceBorder).toBe("rgb(216, 203, 184)");
    expect(railMetrics.workspaceBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(railMetrics.primaryGap).toBe(3);
    expect(railMetrics.primaryActions).toEqual([28, 28]);
    expect(railMetrics.brandHeight).toBeGreaterThanOrEqual(44);
    expect(railMetrics.brandHeight).toBeLessThanOrEqual(48);
    const utilityMetrics = await sidebar.locator("[data-testid='sidebar-utility-nav']").evaluate((node) => {
      const rect = node.getBoundingClientRect();
      const buttons = Array.from(node.querySelectorAll("button")).map((button) => {
        const item = button.getBoundingClientRect();
        return Math.round(item.width);
      });
      const transitions = Array.from(node.querySelectorAll("button")).map((button) => getComputedStyle(button).transitionProperty);
      return { height: Math.round(rect.height), buttons, transitions };
    });
    expect(utilityMetrics.height).toBeLessThanOrEqual(40);
    expect(utilityMetrics.buttons).toEqual([28, 28, 28]);
    expect(railMetrics.primaryTransitions.every((value) => value === "all" || value.includes("box-shadow"))).toBe(true);
    expect(utilityMetrics.transitions.every((value) => value === "all" || value.includes("box-shadow"))).toBe(true);

    const searchAction = sidebar.getByRole("button", { name: "搜索" });
    await searchAction.hover();
    await expect(searchAction).not.toHaveCSS("box-shadow", "none");
    const pluginsAction = sidebar.getByRole("button", { name: "插件" });
    await pluginsAction.hover();
    await expect(pluginsAction).not.toHaveCSS("box-shadow", "none");

    await sidebar.getByRole("button", { name: "插件" }).click();
    const drawer = page.getByRole("complementary", { name: "插件" });
    await expect(drawer.getByText("插件", { exact: true }).first()).toBeVisible();
    await expect(drawer.getByRole("tab", { name: /插件/ })).toHaveAttribute("aria-selected", "true");
    await expect(drawer.getByRole("textbox", { name: "搜索插件" })).toBeVisible();
    await expect(drawer.getByTestId("capability-drawer-header")).toHaveCSS("height", "42px");
    await expect(page.getByTestId("capability-drawer-surface")).toHaveCSS("width", "320px");
    await expect.poll(async () => {
      const box = await drawer.boundingBox();
      return box ? { x: Math.round(box.x), width: Math.round(box.width) } : null;
    }).toEqual({ x: Math.round(width), width: 320 });
    const drawerMaterial = await page.getByTestId("capability-drawer-surface").evaluate((node) => {
      const root = document.documentElement;
      const style = getComputedStyle(node);
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-sidebar-width").trim(),
        backdrop: style.backdropFilter || style.webkitBackdropFilter,
        background: style.backgroundColor,
      };
    });
    expect(drawerMaterial.token).toBe("232px");
    expect(drawerMaterial.backdrop).toBe("none");
    expect(drawerMaterial.background).toBe("rgba(251, 244, 234, 0.96)");
    await expect(drawer.getByTestId("forge-icon-action").first()).toBeVisible();
    await expect(drawer.getByText(/[☖⎔◈●]/)).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "插件" })).toHaveCount(0);
  });

  test("capability drawer reads as a compact safety console", async ({ page }) => {
    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: "插件" }).click();

    const drawer = page.getByRole("complementary", { name: "插件" });
    await expect(drawer).toBeVisible();
    const manager = drawer.locator(".forge-capability-manager");
    await expect(manager).toBeVisible();
    const firstRow = manager.locator(".forge-capability-row").filter({ hasText: "File Reader" }).first();
    await expect(firstRow).toBeVisible();
    await expect.poll(async () => manager.evaluate((node) => node.querySelectorAll<HTMLElement>("[style]").length)).toBe(0);

    const metrics = await manager.evaluate((node) => {
      const root = document.documentElement;
      const style = getComputedStyle(node);
      const tab = node.querySelector<HTMLElement>(".forge-capability-tab[aria-selected='true']");
      const summary = node.querySelector<HTMLElement>("[data-testid='capability-summary-strip']");
      const summaryItems = Array.from(node.querySelectorAll<HTMLElement>(".forge-capability-summary-item"));
      const search = node.querySelector<HTMLElement>(".forge-capability-search");
      const row = node.querySelector<HTMLElement>(".forge-capability-row");
      const toggle = node.querySelector<HTMLElement>(".forge-capability-toggle[data-state='enabled']");
      const inlineStyled = node.querySelectorAll<HTMLElement>("[style]");
      const toggleStyle = toggle ? getComputedStyle(toggle) : null;
      const tabStyle = tab ? getComputedStyle(tab) : null;
      const summaryStyle = summary ? getComputedStyle(summary) : null;
      const searchStyle = search ? getComputedStyle(search) : null;
      const rowStyle = row ? getComputedStyle(row) : null;
      return {
        accent: getComputedStyle(root).getPropertyValue("--forge-accent").trim(),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        tabHeight: tab ? Math.round(tab.getBoundingClientRect().height) : 0,
        tabBorder: tabStyle?.borderBottomColor ?? "",
        summaryDisplay: summaryStyle?.display ?? "",
        summaryItemCount: summaryItems.length,
        summaryMaxHeight: summaryItems.length ? Math.max(...summaryItems.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
        motionEntryCount: node.querySelectorAll("[data-forge-motion='capability-entry']").length,
        searchHeight: search ? Math.round(search.getBoundingClientRect().height) : 0,
        rowHeight: row ? Math.round(row.getBoundingClientRect().height) : 0,
        rowBackground: rowStyle?.backgroundColor ?? "",
        toggleColor: toggleStyle?.color ?? "",
        toggleBackground: toggleStyle?.backgroundColor ?? "",
        inlineStyledCount: inlineStyled.length,
      };
    });

    expect(metrics.accent).toBe("#C48A3A");
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.tabHeight).toBe(32);
    expect(metrics.tabBorder).toBe("rgb(196, 138, 58)");
    expect(metrics.summaryDisplay).toBe("grid");
    expect(metrics.summaryItemCount).toBe(3);
    expect(metrics.summaryMaxHeight).toBeLessThanOrEqual(44);
    expect(metrics.motionEntryCount).toBeGreaterThanOrEqual(3);
    expect(metrics.searchHeight).toBe(32);
    expect(metrics.rowHeight).toBeGreaterThanOrEqual(44);
    expect(metrics.rowBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.toggleColor).toBe("rgb(196, 138, 58)");
    expect(metrics.toggleBackground).not.toContain("16, 185, 129");
    expect(metrics.toggleBackground).not.toContain("52, 211, 153");
    expect(metrics.inlineStyledCount).toBe(0);
  });

  test("sidebar history rows stay compact and scannable", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.locator("textarea").fill("Build a compact scanner");
    await page.locator("textarea").press("Enter");

    const sidebar = page.locator("aside").first();
    const row = sidebar.getByRole("button", { name: "Build a compact scanner", exact: true });
    await expect(row).toBeVisible();

    const metrics = await row.evaluate((node) => {
      const root = document.documentElement;
      const style = getComputedStyle(node);
      const indicatorStyle = getComputedStyle(node, "::before");
      const label = node.querySelector("span");
      const deleteButton = node.querySelector("button");
      const list = node.closest(".forge-sidebar-history-list");
      const groupLabel = document.querySelector(".forge-sidebar-history-group-label");
      return {
        token: getComputedStyle(root).getPropertyValue("--forge-sidebar-row-height").trim(),
        height: Math.round(node.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        labelInset: label ? Math.round(label.getBoundingClientRect().left - node.getBoundingClientRect().left) : 0,
        indicatorContent: indicatorStyle.content,
        indicatorWidth: indicatorStyle.width,
        borderColor: style.borderTopColor,
        background: style.backgroundColor,
        deleteOpacity: deleteButton ? getComputedStyle(deleteButton).opacity : null,
        listDisplay: list ? getComputedStyle(list).display : "",
        groupLabelHeight: groupLabel ? Math.round(groupLabel.getBoundingClientRect().height) : 0,
      };
    });

    expect(metrics.token).toBe("28px");
    expect(metrics.height).toBe(28);
    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.labelInset).toBeGreaterThanOrEqual(10);
    expect(metrics.indicatorContent).toBe("none");
    expect(metrics.indicatorWidth).toBe("auto");
    expect(metrics.borderColor).toBe("rgb(216, 203, 184)");
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.deleteOpacity).toBe("0");
    expect(metrics.listDisplay).toBe("flex");
    expect(metrics.groupLabelHeight).toBeGreaterThanOrEqual(20);
  });

  test("workspace menu uses the shared compact floating surface", async ({ page }) => {
    const sidebar = page.locator("aside").first();
    const trigger = sidebar.getByRole("button", { name: /forge/ });
    await trigger.click();
    const menu = page.getByRole("menu", { name: "项目文件夹" });
    await expect(menu).toBeVisible();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const trigger = document.querySelector("[data-testid='workspace-trigger']");
      const menu = document.querySelector("#workspace-menu");
      const option = menu?.querySelector("[role='menuitemradio'], [role='menuitem']");
      if (!trigger || !menu || !option) return null;
      const triggerRect = trigger.getBoundingClientRect();
      const menuRect = menu.getBoundingClientRect();
      const menuStyle = getComputedStyle(menu);
      return {
        gapToken: getComputedStyle(root).getPropertyValue("--forge-floating-gap").trim(),
        menuTopGap: Math.round(menuRect.top - triggerRect.bottom),
        optionHeight: Math.round(option.getBoundingClientRect().height),
        shadow: menuStyle.boxShadow,
        radius: Number.parseFloat(menuStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.gapToken).toBe("8px");
    expect(metrics!.menuTopGap).toBe(8);
    expect(metrics!.optionHeight).toBe(34);
    expect(metrics!.shadow).not.toContain("0px 25px");
    expect(metrics!.radius).toBeLessThanOrEqual(8);
  });

  test("project archive opens from the keyboard as a quiet inspector", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.keyboard.down("Control");
    await page.keyboard.press("i");
    await page.keyboard.up("Control");
    const archive = page.getByRole("complementary", { name: "项目档案" });
    await expect(archive).toBeVisible();
    await expect(archive.getByText("Project Status")).toHaveCount(0);
    await expect(archive.getByText("Context Activation")).toHaveCount(0);

    const metrics = await archive.evaluate((node) => {
      const style = getComputedStyle(node);
      return {
        width: Math.round(node.getBoundingClientRect().width),
        bg: style.backgroundColor,
      };
    });
    expect(metrics.width).toBeLessThanOrEqual(320);
    expect(metrics.bg).not.toBe("rgba(0, 0, 0, 0)");

    await page.keyboard.press("Escape");
    await expect(archive).toHaveCount(0);
  });

  test("project archive disclosure rows use inspector rhythm tokens", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.setViewportSize({ width: 900, height: 720 });
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.keyboard.down("Control");
    await page.keyboard.press("i");
    await page.keyboard.up("Control");

    const archive = page.getByRole("complementary", { name: "项目档案" });
    await expect(archive).toBeVisible();

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const archive = document.querySelector<HTMLElement>("[data-testid='project-archive-panel']");
      const body = document.querySelector<HTMLElement>("[data-testid='project-archive-body']");
      const disclosure = document.querySelector<HTMLElement>("[data-testid='archive-disclosure-records'] button");
      const main = document.querySelector<HTMLElement>("[data-testid='main-workbench']");
      const composer = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const modelChip = document.querySelector<HTMLElement>("[data-testid='composer-model-chip']");
      const title = document.querySelector<HTMLElement>(".forge-inspector-title");
      const subtitle = document.querySelector<HTMLElement>(".forge-inspector-subtitle");
      const summaryLabel = document.querySelector<HTMLElement>(".forge-archive-summary-label");
      const summaryValue = document.querySelector<HTMLElement>(".forge-archive-summary-value");
      if (!archive || !body || !disclosure || !main || !composer || !modelChip || !title || !subtitle || !summaryLabel || !summaryValue) return null;
      const archiveRect = archive.getBoundingClientRect();
      const composerRect = composer.getBoundingClientRect();
      const modelChipRect = modelChip.getBoundingClientRect();
      const archiveStyle = getComputedStyle(archive);
      return {
        widthToken: getComputedStyle(root).getPropertyValue("--forge-inspector-width").trim(),
        gapToken: getComputedStyle(root).getPropertyValue("--forge-inspector-gap").trim(),
        rowToken: getComputedStyle(root).getPropertyValue("--forge-disclosure-row-height").trim(),
        width: Math.round(archiveRect.width),
        background: archiveStyle.backgroundColor,
        backdropFilter: archiveStyle.backdropFilter,
        bodyGap: Math.round(Number.parseFloat(getComputedStyle(body).rowGap)),
        rowHeight: Math.round(disclosure.getBoundingClientRect().height),
        archiveLeft: Math.round(archiveRect.left),
        composerRight: Math.round(composerRect.right),
        modelChipRight: Math.round(modelChipRect.right),
        mainPaddingRight: getComputedStyle(main).paddingRight,
        titleFontSize: getComputedStyle(title).fontSize,
        subtitleFontSize: getComputedStyle(subtitle).fontSize,
        subtitleColor: getComputedStyle(subtitle).color,
        summaryLabelFontSize: getComputedStyle(summaryLabel).fontSize,
        summaryValueColor: getComputedStyle(summaryValue).color,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.widthToken).toBe("300px");
    expect(metrics!.gapToken).toBe("10px");
    expect(metrics!.rowToken).toBe("28px");
    expect(metrics!.width).toBe(300);
    expect(metrics!.background).toBe("rgb(251, 244, 234)");
    expect(metrics!.backdropFilter).toBe("none");
    expect(metrics!.bodyGap).toBe(10);
    expect(metrics!.rowHeight).toBe(28);
    expect(metrics!.mainPaddingRight).toBe(metrics!.widthToken);
    expect(metrics!.composerRight).toBeLessThanOrEqual(metrics!.archiveLeft - 12);
    expect(metrics!.modelChipRight).toBeLessThanOrEqual(metrics!.archiveLeft - 12);
    expect(metrics!.titleFontSize).toBe("14px");
    expect(metrics!.subtitleFontSize).toBe("11px");
    expect(metrics!.subtitleColor).toBe("rgb(95, 93, 85)");
    expect(metrics!.summaryLabelFontSize).toBe("11px");
    expect(metrics!.summaryValueColor).toBe("rgb(36, 42, 36)");
  });

  test("project archive summary gives quick project, context, and record state", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.getByTitle("打开项目档案").click();

    const archive = page.getByRole("complementary", { name: "项目档案" });
    await expect(archive).toBeVisible();
    const summary = archive.getByTestId("project-archive-summary-strip");
    await expect(summary).toBeVisible();
    await expect(summary.getByText("项目", { exact: true })).toBeVisible();
    await expect(summary.getByText("上下文", { exact: true })).toBeVisible();
    await expect(summary.getByText("记录", { exact: true })).toBeVisible();

    const metrics = await summary.evaluate((node) => {
      const items = Array.from(node.querySelectorAll<HTMLElement>(".forge-archive-summary-item"));
      const firstIcon = node.querySelector<HTMLElement>(".forge-archive-summary-icon");
      const firstValue = node.querySelector<HTMLElement>(".forge-archive-summary-value");
      return {
        display: getComputedStyle(node).display,
        itemCount: items.length,
        maxItemHeight: items.length ? Math.max(...items.map((item) => Math.round(item.getBoundingClientRect().height))) : 0,
        iconWidth: firstIcon ? Math.round(firstIcon.getBoundingClientRect().width) : 0,
        valueOverflow: firstValue ? getComputedStyle(firstValue).textOverflow : "",
      };
    });

    expect(metrics.display).toBe("grid");
    expect(metrics.itemCount).toBe(3);
    expect(metrics.maxItemHeight).toBeLessThanOrEqual(50);
    expect(metrics.iconWidth).toBe(24);
    expect(metrics.valueOverflow).toBe("ellipsis");
  });

  test("global new conversation shortcut starts from the active workspace", async ({ page }) => {
    await page.keyboard.down("Control");
    await page.keyboard.press("n");
    await page.keyboard.up("Control");

    await expect(page.locator("textarea")).toBeVisible();
    await expect(page.getByRole("main").getByText("选择一个项目开始")).toHaveCount(0);
  });

  test("command palette shows compact desktop shortcuts", async ({ page }) => {
    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");

    const palette = page.getByRole("dialog");
    await expect(palette.getByTestId("command-palette-surface")).toBeVisible();
    const paletteMetrics = await palette.evaluate((node) => {
      const motionRoot = node.querySelector<HTMLElement>(".forge-command-motion-root");
      const surface = node.querySelector<HTMLElement>("[data-testid='command-palette-surface']");
      const input = node.querySelector<HTMLElement>("[data-slot='command-input-wrapper']");
      const inputControl = node.querySelector<HTMLElement>("[data-slot='command-input']");
      const item = node.querySelector<HTMLElement>("[data-slot='command-item']");
      const shortcut = node.querySelector<HTMLElement>("[data-testid='command-shortcut']");
      const style = surface ? getComputedStyle(surface) : null;
      const inputStyle = inputControl ? getComputedStyle(inputControl) : null;
      const motionStyle = motionRoot ? getComputedStyle(motionRoot) : null;
      return {
        width: Math.round(node.getBoundingClientRect().width),
        motionRootWidth: motionRoot ? Math.round(motionRoot.getBoundingClientRect().width) : 0,
        motionWillChange: motionStyle?.willChange ?? "",
        motionEntryCount: node.querySelectorAll("[data-forge-motion='command-entry']").length,
        radius: style ? Number.parseFloat(style.borderTopLeftRadius) : 0,
        inputHeight: input ? Math.round(input.getBoundingClientRect().height) : 0,
        inputBackground: inputStyle?.backgroundColor ?? "",
        inputOutline: inputStyle?.outlineStyle ?? "",
        itemHeight: item ? Math.round(item.getBoundingClientRect().height) : 0,
        shortcutRadius: shortcut ? Number.parseFloat(getComputedStyle(shortcut).borderTopLeftRadius) : 0,
      };
    });
    expect(paletteMetrics.width).toBeGreaterThanOrEqual(540);
    expect(paletteMetrics.width).toBeLessThanOrEqual(600);
    expect(paletteMetrics.motionRootWidth).toBeGreaterThanOrEqual(540);
    expect(paletteMetrics.motionWillChange).toContain("transform");
    expect(paletteMetrics.motionEntryCount).toBeGreaterThanOrEqual(3);
    expect(paletteMetrics.radius).toBeLessThanOrEqual(8);
    expect(paletteMetrics.inputHeight).toBeLessThanOrEqual(42);
    expect(paletteMetrics.inputBackground).toBe("rgba(0, 0, 0, 0)");
    expect(paletteMetrics.inputOutline).toBe("none");
    expect(paletteMetrics.itemHeight).toBeLessThanOrEqual(34);
    expect(paletteMetrics.shortcutRadius).toBeLessThanOrEqual(6);
    await expect(palette.getByRole("option", { name: /新建对话/ })).toContainText("⌘N");
    await expect(palette.getByRole("option", { name: /设置/ })).toContainText("⌘,");

    await page.keyboard.press("Escape");
    await page.keyboard.down("Control");
    await page.keyboard.press(",");
    await page.keyboard.up("Control");
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
  });
});

test.describe("Browser dev fallback", () => {
  test("new conversation opens an input without the Tauri runtime", async ({ page }) => {
    const dialogs: string[] = [];
    page.on("dialog", async (dialog) => {
      dialogs.push(dialog.message());
      await dialog.dismiss();
    });

    await page.goto("http://localhost:1420");
    await page.evaluate(() => {
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge-playground");
    });
    await page.reload();
    await page.waitForSelector("[class*=sidebar]", { timeout: 10000 });
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(page.locator("textarea")).toBeVisible();
    expect(dialogs).toEqual([]);
  });
});

test.describe("Workspace Safety v0", () => {
  test("first launch asks the user to choose a workspace before creating a conversation", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    await expect(page.getByRole("main").getByTestId("empty-entry-new-tool")).toBeVisible();
    await expect(page.getByRole("main").getByTestId("empty-entry-existing-project")).toBeVisible();
    await expect(page.getByRole("main").getByRole("button", { name: /做个新工具/ })).toBeVisible();
    await expect(page.getByRole("main").getByRole("button", { name: /打开已有项目/ })).toBeVisible();
    await expect(page.getByRole("button", { name: "新对话", exact: true })).toBeDisabled();
  });

  test("conversation list follows the active workspace", async ({ page }) => {
    const workspaceA = "/Users/cabbos/project/app-one";
    const workspaceB = "/Users/cabbos/project/app-two";
    const sessionA = crypto.randomUUID();
    const sessionB = crypto.randomUUID();

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspaceA, workspaceB, sessionA, sessionB }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: workspaceA, name: "app-one", path: workspaceA, lastOpenedAt: 2 },
        { id: workspaceB, name: "app-two", path: workspaceB, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(workspaceA, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionA,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
          workingDir: workspaceA,
          workspaceId: workspaceA,
        },
        {
          id: sessionB,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
          workingDir: workspaceB,
          workspaceId: workspaceB,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put([
        {
          block_id: "workspace-a-message",
          event_type: "user_message",
          content: "Build A timer",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionA}`);
      tx.objectStore("keyval").put([
        {
          block_id: "workspace-b-message",
          event_type: "user_message",
          content: "Build B dashboard",
          isComplete: true,
          metadata: {},
        },
      ], `forge-blocks:${sessionB}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspaceA, workspaceB, sessionA, sessionB });

    await page.reload();

    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("button", { name: /app-one/ })).toBeVisible();
    await expect(sidebar.getByText(workspaceA, { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("对话", { exact: true })).toBeVisible();
    await expect(sidebar.getByText("任务", { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("Build A timer")).toBeVisible();
    await expect(sidebar.getByText("Build B dashboard")).toHaveCount(0);
    await expect(sidebar.getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);

    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");
    await expect(page.getByRole("option", { name: /Build A timer/ })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("当前项目 · app-one")).toBeVisible();
    await expect(page.getByRole("dialog").getByText("最近对话")).toBeVisible();
    await expect(page.getByRole("dialog").getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);
    await page.keyboard.press("Escape");

    const workspaceTrigger = sidebar.getByRole("button", { name: /app-one/ });
    await expect(workspaceTrigger).toHaveAttribute("aria-haspopup", "menu");
    await workspaceTrigger.click();
    const workspaceMenu = page.getByRole("menu", { name: "项目文件夹" });
    await expect(workspaceMenu).toBeVisible();
    await expect(workspaceMenu.getByRole("menuitemradio", { name: /app-one/ })).toHaveAttribute("aria-checked", "true");
    await expect(workspaceMenu.getByRole("menuitemradio", { name: /app-two/ })).toHaveAttribute("aria-checked", "false");
    await expect(sidebar.getByText(workspaceA, { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText(workspaceB, { exact: true })).toHaveCount(0);
    await workspaceMenu.getByRole("menuitemradio", { name: /app-two/ }).click();

    await expect(sidebar.getByRole("button", { name: /app-two/ })).toBeVisible();
    await expect(sidebar.getByText("Build B dashboard")).toBeVisible();
    await expect(sidebar.getByText("Build A timer")).toHaveCount(0);
    await expect(sidebar.getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);
  });

  test("conversation list supports keyboard navigation", async ({ page }) => {
    const workspace = "/Users/cabbos/project/forge";
    const sessions = [
      { id: "keyboard-a", title: "Build alpha tool" },
      { id: "keyboard-b", title: "Build beta tool" },
      { id: "keyboard-c", title: "Build gamma tool" },
    ];

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspace, sessions }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([{ id: workspace, name: "forge", path: workspace, lastOpenedAt: 3 }], "forge-workspaces");
      tx.objectStore("keyval").put(workspace, "forge-active-workspace");
      tx.objectStore("keyval").put(sessions.map((session) => ({
        id: session.id,
        agentType: "deepseek",
        model: "deepseek-v4-flash[1m]",
        contextWindowTokens: 1_000_000,
        status: "stopped",
        workflowState: null,
        workingDir: workspace,
        workspaceId: workspace,
      })), "forge-sessions");
      for (const session of sessions) {
        tx.objectStore("keyval").put([
          {
            block_id: `${session.id}-message`,
            event_type: "user_message",
            content: session.title,
            isComplete: true,
            metadata: {},
          },
        ], `forge-blocks:${session.id}`);
      }
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspace, sessions });

    await page.reload();
    const sidebar = page.locator("aside").first();
    const first = sidebar.getByRole("button", { name: "Build alpha tool", exact: true });
    const second = sidebar.getByRole("button", { name: "Build beta tool", exact: true });
    const third = sidebar.getByRole("button", { name: "Build gamma tool", exact: true });
    await expect(first).toBeVisible();
    await first.focus();

    await page.keyboard.press("ArrowDown");
    await expect(second).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(third).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(second).toBeFocused();
    await page.keyboard.press("Enter");

    await expect(page.getByRole("main").getByText("Build beta tool").last()).toBeVisible();
  });

  test("conversation list groups sessions by recency", async ({ page }) => {
    const workspace = "/Users/cabbos/project/forge";
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    const sessions = [
      { id: "recent-today", title: "Today build", updatedAt: Date.now() },
      { id: "recent-yesterday", title: "Yesterday build", updatedAt: todayStart.getTime() - 12 * 60 * 60 * 1000 },
      { id: "recent-older", title: "Older build", updatedAt: todayStart.getTime() - 8 * 24 * 60 * 60 * 1000 },
    ];

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspace, sessions }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([{ id: workspace, name: "forge", path: workspace, lastOpenedAt: 3 }], "forge-workspaces");
      tx.objectStore("keyval").put(workspace, "forge-active-workspace");
      tx.objectStore("keyval").put(sessions.map((session) => ({
        id: session.id,
        agentType: "deepseek",
        model: "deepseek-v4-flash[1m]",
        contextWindowTokens: 1_000_000,
        status: "stopped",
        workflowState: null,
        workingDir: workspace,
        workspaceId: workspace,
        createdAt: session.updatedAt,
        updatedAt: session.updatedAt,
      })), "forge-sessions");
      for (const session of sessions) {
        tx.objectStore("keyval").put([
          {
            block_id: `${session.id}-message`,
            event_type: "user_message",
            content: session.title,
            isComplete: true,
            metadata: {},
          },
        ], `forge-blocks:${session.id}`);
      }
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspace, sessions });

    await page.reload();
    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByText("今天", { exact: true })).toBeVisible();
    await expect(sidebar.getByText("昨天", { exact: true })).toBeVisible();
    await expect(sidebar.getByText("更早", { exact: true })).toBeVisible();

    const order = await sidebar.getByRole("button").evaluateAll((nodes) =>
      nodes
        .map((node) => node.getAttribute("aria-label") ?? "")
        .filter((label) => ["Today build", "Yesterday build", "Older build"].includes(label)),
    );
    expect(order).toEqual(["Today build", "Yesterday build", "Older build"]);
  });

  test("folder picker activates new conversations", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockDirectoryPicker = async () => "/Users/cabbos/project/demo-tool";
    });
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: /选择项目/ }).click();
    await page.getByRole("menuitem", { name: "选择文件夹" }).click();

    await expect(sidebar.getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeEnabled();
  });

  test("empty entry opens the folder picker before the first conversation", async ({ page }) => {
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockDirectoryPicker = async () => "/Users/cabbos/project/demo-tool";
    });
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    const main = page.getByRole("main");
    await main.getByTestId("empty-entry-new-tool").click();

    await expect(page.locator("aside").first().getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(main.getByTestId("empty-start-composer")).toBeVisible();
    await expect(main.getByTestId("empty-start-composer").getByRole("textbox")).toBeFocused();
  });

  test("workspace menu can remove the current project from the recent list", async ({ page }) => {
    const workspaceA = "/Users/cabbos/project/remove-one";
    const workspaceB = "/Users/cabbos/project/remove-two";
    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ workspaceA, workspaceB }) => {
      const openDb = () => new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const db = await openDb();
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: workspaceA, name: "remove-one", path: workspaceA, lastOpenedAt: 2 },
        { id: workspaceB, name: "remove-two", path: workspaceB, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(workspaceA, "forge-active-workspace");
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { workspaceA, workspaceB });

    await page.reload();
    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("button", { name: /remove-one/ })).toBeVisible();
    await sidebar.getByRole("button", { name: /remove-one/ }).click();
    await page.getByRole("menuitem", { name: "从列表移除当前项目" }).click();

    await expect(sidebar.getByRole("button", { name: /remove-two/ })).toBeVisible();
    await sidebar.getByRole("button", { name: /remove-two/ }).click();
    await expect(page.getByRole("menuitemradio", { name: /remove-one/ })).toHaveCount(0);
  });

  test("manual workspace path entry remains available as fallback", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: /选择项目/ }).click();
    await page.getByRole("menuitem", { name: "手动输入路径" }).click();

    const pathInput = page.getByLabel("项目文件夹路径");
    await expect(pathInput).toBeVisible();
    await pathInput.fill("/Users/cabbos/project/demo-tool");
    await page.getByRole("button", { name: "添加" }).click();

    await expect(sidebar.getByRole("button", { name: /demo-tool/ })).toBeVisible();
    await expect(sidebar.getByRole("button", { name: "新对话", exact: true })).toBeEnabled();
  });

  test("manual workspace path rejects broad user directory", async ({ page }) => {
    await page.goto("http://localhost:1420");
    await page.evaluate(async () => {
      window.localStorage.removeItem("forge-working-dir");
      window.localStorage.removeItem("tui-to-gui-working-dir");
      await new Promise<void>((resolve) => {
        const request = indexedDB.deleteDatabase("keyval-store");
        request.onsuccess = () => resolve();
        request.onerror = () => resolve();
        request.onblocked = () => resolve();
      });
    });
    await page.reload();
    await page.waitForSelector("aside", { timeout: 10000 });

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: /选择项目/ }).click();
    await page.getByRole("menuitem", { name: "手动输入路径" }).click();

    const pathInput = page.getByLabel("项目文件夹路径");
    for (const path of ["/Users", "/Users/cabbos", "/home"]) {
      await pathInput.fill(path);
      await page.getByRole("button", { name: "添加" }).click();
      await expect(page.getByText("请选择具体项目文件夹，不要直接使用用户主目录。")).toBeVisible();
    }

    await expect(sidebar.getByRole("button", { name: /^Users$/ })).toHaveCount(0);
    await expect(sidebar.getByRole("button", { name: /^home$/ })).toHaveCount(0);
    await expect(sidebar.getByRole("button", { name: /cabbos/ })).toHaveCount(0);
  });

  test("create session broad project failure is visible and manual project selection can recover", async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockCreateSessionError = "请选择具体项目文件夹，不要直接使用用户主目录。";
    });
    await page.goto("http://localhost:1420");

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(sidebar.getByRole("status")).toContainText("请选择具体项目文件夹，不要直接使用用户主目录。");

    await sidebar.getByRole("button", { name: /forge/ }).click();
    await page.getByRole("menuitem", { name: "手动输入路径" }).click();
    await page.getByLabel("项目文件夹路径").fill("/Users/cabbos/project/recovered-app");
    await page.getByRole("button", { name: "添加" }).click();
    await expect(sidebar.getByRole("button", { name: /recovered-app/ })).toBeVisible();

    await page.evaluate(() => {
      // @ts-expect-error mock
      window.__mockCreateSessionError = "";
    });
    await sidebar.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(page.locator("textarea")).toBeVisible();
    await expect(sidebar.getByRole("status")).toHaveCount(0);
  });

  test("create session inaccessible project failure is visible", async ({ page }) => {
    await setup(page);
    await page.addInitScript(() => {
      // @ts-expect-error mock
      window.__mockCreateSessionError = "无法打开项目文件夹：No such file or directory";
    });
    await page.goto("http://localhost:1420");

    const sidebar = page.locator("aside").first();
    await sidebar.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(sidebar.getByRole("status")).toContainText("这个项目文件夹打不开。请重新选择一个具体项目文件夹。");
  });

  test("workspace identity stays visible when starting a sandbox conversation", async ({ page }) => {
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    await setup(page);
    await page.addInitScript((path) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", path);
    }, sandboxPath);

    await page.goto("http://localhost:1420");

    const sidebar = page.locator("aside").first();
    await expect(sidebar.getByRole("button", { name: /forge-test-app/ })).toBeVisible();
    await expect(sidebar.getByText(sandboxPath, { exact: true })).toHaveCount(0);
    await expect(sidebar.getByText("新对话会创建在 forge-test-app")).toHaveCount(0);

    const workspaceBoundary = page.getByLabel("当前项目边界");
    await expect(workspaceBoundary.getByText("当前项目")).toBeVisible();
    await expect(workspaceBoundary.getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(workspaceBoundary.getByText(sandboxPath)).toHaveCount(0);
    await expect(workspaceBoundary.getByText(/DeepSeek|上下文|条记录/)).toHaveCount(0);

    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    expect(createArgs.workingDir).toBe(sandboxPath);
    const main = page.getByRole("main");
    await expect(main.getByText(`本轮会作用于 forge-test-app · ${sandboxPath}`)).toHaveCount(0);
    await expect(main.getByText("准备开始")).toBeVisible();
  });
});

test.describe("InputBar", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
  });

  test("enter key sends message and clears input", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("Hello DeepSeek");
    await textarea.press("Enter");

    // User bubble should appear
    await expect(page.getByRole("main").getByText("Hello DeepSeek", { exact: true }).last()).toBeVisible({ timeout: 3000 });
  });

  test("empty start readiness checks the active workspace explicitly", async ({ page }) => {
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript((projectPath) => {
      window.localStorage.setItem("forge-working-dir", projectPath);
    }, projectPath);

    await page.goto("http://localhost:1420");
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();

    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastProjectRuntimeStatusArgs;
    })).toMatchObject({ sessionId: null, workingDir: projectPath });
    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastProjectCheckpointStatusArgs;
    })).toMatchObject({ sessionId: null, workingDir: projectPath });
  });

  test("composer checkpoint is created inside the active session workspace", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("帮我检查这个 demo 页面");
    await textarea.press("Enter");

    await expect.poll(async () => page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateProjectCheckpointArgs;
    })).toMatchObject({ sessionId, workingDir: projectPath });
  });

  test("send failures show an inline recovery message and clear pending state", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.evaluate(() => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "send_input") {
          throw new Error("Session not found: send-failure-test");
        }
        return original?.(cmd, args);
      };
    });

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("帮我继续做这个页面");
    await textarea.press("Enter");

    await expect(page.getByTestId("user-message").last()).toContainText("帮我继续做这个页面");
    const errorCard = page.getByTestId("message-panel").filter({ hasText: "发送失败" });
    await expect(errorCard).toHaveAttribute("role", "status");
    await expect(errorCard.getByTestId("error-card-body")).toContainText("当前会话暂时不可用");
    const errorMetrics = await errorCard.evaluate((node) => {
      const body = node.querySelector<HTMLElement>("[data-testid='error-card-body']");
      const style = getComputedStyle(node);
      const after = getComputedStyle(node, "::after");
      return {
        width: Math.round(node.getBoundingClientRect().width),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        border: style.borderTopColor,
        bodyHeight: body ? Math.round(body.getBoundingClientRect().height) : 0,
        afterContent: after.content,
        afterWidth: after.width,
      };
    });
    expect(errorMetrics.width).toBeLessThanOrEqual(620);
    expect(errorMetrics.radius).toBeLessThanOrEqual(8);
    expect(errorMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(errorMetrics.border).not.toBe("rgba(0, 0, 0, 0)");
    expect(errorMetrics.bodyHeight).toBeLessThanOrEqual(38);
    expect(errorMetrics.afterContent).toBe("none");
    expect(errorMetrics.afterWidth).toBe("auto");
    await expect(page.getByTestId("pending-block")).toHaveCount(0);
    await expect(textarea).toBeEnabled();
  });

  test("shift+enter creates newline without sending", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("line1");
    await textarea.press("Shift+Enter");
    await textarea.pressSequentially("line2");

    // Should still be in the textarea, not sent
    await expect(textarea).toContainText("line1\nline2");
  });

  test("long prompts stay inside a bounded editor scroll area", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    const longPrompt = Array.from({ length: 24 }, (_, index) => `第 ${index + 1} 行：继续描述这个小工具的细节。`).join("\n");
    await textarea.fill(longPrompt);

    const metrics = await page.evaluate(() => {
      const root = document.documentElement;
      const textarea = document.querySelector("textarea");
      if (!textarea) return null;
      const rect = textarea.getBoundingClientRect();
      const style = getComputedStyle(textarea);

      return {
        token: getComputedStyle(root).getPropertyValue("--forge-composer-max-input-height").trim(),
        height: Math.round(rect.height),
        maxHeight: Math.round(Number.parseFloat(style.maxHeight)),
        overflowY: style.overflowY,
        canScrollInside: textarea.scrollHeight > textarea.clientHeight,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.token).toBe("128px");
    expect(metrics!.height).toBeLessThanOrEqual(128);
    expect(metrics!.maxHeight).toBe(128);
    expect(metrics!.overflowY).toBe("auto");
    expect(metrics!.canScrollInside).toBe(true);
  });

  test("enter during IME composition does not send the draft", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const textarea = page.locator("textarea");
    await textarea.fill("正在组词");
    await textarea.focus();
    await textarea.evaluate((node) => {
      node.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true, data: "zheng" }));
      node.dispatchEvent(new KeyboardEvent("keydown", {
        key: "Enter",
        code: "Enter",
        bubbles: true,
        cancelable: true,
      }));
    });

    await expect(textarea).toHaveValue("正在组词");
    await expect(page.getByTestId("user-message")).toHaveCount(0);
  });

  test("composer command surface stays compact but exposes structured controls", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const slash = composer.getByRole("button", { name: "常用请求" });
    await expect(slash).toHaveAttribute("aria-expanded", "false");
    await slash.click();
    await expect(slash).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByTestId("composer-command-menu")).toHaveAttribute("role", "listbox");
    await expect(page.getByRole("option", { name: /\/code-review/ })).toBeVisible();

    await page.keyboard.press("Escape");
    const model = composer.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ });
    await expect(model).toHaveAttribute("aria-expanded", "false");
    await model.click();
    await expect(model).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("menuitemradio", { name: /DeepSeek V4 Flash 1M/ })).toHaveAttribute("aria-checked", "true");
  });

  test("composer capability rows use semantic icon tones", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    await composer.getByRole("button", { name: "常用请求" }).click();
    await expect(page.getByTestId("composer-command-menu").getByTestId("forge-icon-action").first()).toBeVisible();

    await page.keyboard.press("Escape");
    const textarea = page.locator("textarea");
    await textarea.fill("@src");
    await expect(page.getByTestId("composer-command-menu").getByTestId("forge-icon-context").first()).toBeVisible();
  });

  test("composer command menu supports keyboard selection", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("/");
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();
    await expect(page.getByRole("option", { name: /\/code-review/ })).toHaveAttribute("aria-selected", "true");

    await textarea.press("ArrowDown");
    await expect(page.getByRole("option", { name: /\/fix/ })).toHaveAttribute("aria-selected", "true");
    await textarea.press("Enter");

    const composer = page.getByTestId("composer-lane");
    await expect(composer.getByText("/fix", { exact: true })).toBeVisible();
    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expectNoSendInput(page);
  });

  test("composer file suggestions can be accepted without leaving the keyboard", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toBeVisible();
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toHaveAttribute("aria-selected", "true");

    await textarea.press("Tab");

    const composer = page.getByTestId("composer-lane");
    await expect(composer.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expect(textarea).toHaveValue("");
  });

  test("composer keyboard selection ignores a stationary pointer when file suggestions open", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const composer = page.getByTestId("composer-lane");
    const textarea = composer.locator("textarea");
    await expect(textarea).toBeEnabled();

    await textarea.fill("@src");
    const secondOption = page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ });
    await expect(secondOption).toBeVisible();
    const secondOptionBox = await secondOption.boundingBox();
    expect(secondOptionBox).not.toBeNull();

    await textarea.fill("");
    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await page.mouse.move(secondOptionBox!.x + 8, secondOptionBox!.y + secondOptionBox!.height / 2);

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toHaveAttribute("aria-selected", "true");
    await textarea.press("Tab");
    await expect(composer.getByText("src/App.tsx", { exact: true })).toBeVisible();
  });

  test("composer file search is scoped to the active session project", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();

    const args = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSearchWorkspaceFilesArgs;
    });

    expect(args).toMatchObject({ query: "src", sessionId, workingDir: "/Users/cabbos/project/forge" });
  });

  test("composer file search sends the explicit workspace path for restored sessions", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    const projectPath = "/Users/cabbos/project/forge-test-app";
    await page.addInitScript(({ sessionId, projectPath }) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      window.localStorage.setItem("forge-working-dir", projectPath);
    }, { sessionId, projectPath });
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/DemoApp\.tsx/ })).toBeVisible();
    await expect(page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ })).toHaveCount(0);

    const args = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastSearchWorkspaceFilesArgs;
    });

    expect(args).toMatchObject({ query: "src", sessionId, workingDir: projectPath });
  });

  test("composer sends selected capabilities as structured intent", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("/");
    await textarea.press("ArrowDown");
    await textarea.press("Enter");
    await expect(page.getByTestId("composer-lane").getByText("/fix", { exact: true })).toBeVisible();

    await textarea.fill("@src");
    await expect(page.getByRole("option", { name: /src\/App\.tsx/ })).toBeVisible();
    await textarea.press("Tab");
    await expect(page.getByTestId("composer-lane").getByText("src/App.tsx", { exact: true })).toBeVisible();

    await textarea.fill("按钮没有反应");
    await textarea.press("Enter");

    const sendArgs = await expectLastSendInputArgs(page, {
      sessionId,
      capabilities: [
        { kind: "slash_command", command: "/fix" },
        { kind: "file_reference", path: "src/App.tsx" },
      ],
    });
    const sentText = String(sendArgs.text);
    expect(sentText).toContain("按钮没有反应");
    expect(sentText).not.toContain("/fix");
  });

  test("composer keeps active tool state quiet and explicit", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const surface = page.getByTestId("composer-surface");
    const fileButton = composer.getByRole("button", { name: "引用文件" });
    const modelButton = composer.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ });

    await expect(surface).toHaveAttribute("data-menu-open", "false");
    await expect(fileButton).toHaveAttribute("data-active", "false");

    await fileButton.click();
    await expect(surface).toHaveAttribute("data-menu-open", "true");
    await expect(fileButton).toHaveAttribute("data-active", "true");
    await expect(fileButton).toHaveText("");

    await page.keyboard.press("Escape");
    await modelButton.click();
    await expect(surface).toHaveAttribute("data-menu-open", "true");
    await expect(modelButton).toHaveAttribute("data-active", "true");
  });

  test("composer surface uses claude-style conversation proportions", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const textarea = surface.querySelector<HTMLTextAreaElement>("textarea");
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const send = surface.querySelector<HTMLElement>("[data-testid='composer-send']");
      const style = getComputedStyle(surface);
      const textareaStyle = textarea ? getComputedStyle(textarea) : null;
      const toolbarStyle = toolbar ? getComputedStyle(toolbar) : null;
      const sendRect = send?.getBoundingClientRect();
      return {
        surfaceHeight: Math.round(surface.getBoundingClientRect().height),
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        shadow: style.boxShadow,
        minInputHeight: textareaStyle ? Number.parseFloat(textareaStyle.minHeight) : 0,
        lineHeight: textareaStyle ? Number.parseFloat(textareaStyle.lineHeight) : 0,
        toolbarHeight: toolbar ? Math.round(toolbar.getBoundingClientRect().height) : 0,
        toolbarBorderTopWidth: toolbarStyle ? Math.round(Number.parseFloat(toolbarStyle.borderTopWidth)) : -1,
        toolbarBackground: toolbarStyle?.backgroundColor ?? "",
        toolbarPaddingBottom: toolbarStyle ? Number.parseFloat(toolbarStyle.paddingBottom) : 0,
        sendWidth: sendRect ? Math.round(sendRect.width) : 0,
        sendHeight: sendRect ? Math.round(sendRect.height) : 0,
      };
    });

    expect(metrics.radius).toBeLessThanOrEqual(8);
    expect(metrics.surfaceHeight).toBeGreaterThanOrEqual(102);
    expect(metrics.surfaceHeight).toBeLessThanOrEqual(110);
    expect(metrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics.shadow).not.toBe("none");
    expect(metrics.minInputHeight).toBeGreaterThanOrEqual(44);
    expect(metrics.minInputHeight).toBeLessThanOrEqual(46);
    expect(metrics.lineHeight).toBe(24);
    expect(metrics.toolbarHeight).toBeGreaterThanOrEqual(34);
    expect(metrics.toolbarHeight).toBeLessThanOrEqual(38);
    expect(metrics.toolbarBorderTopWidth).toBe(0);
    expect(metrics.toolbarBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics.toolbarPaddingBottom).toBe(8);
    expect(metrics.sendWidth).toBe(30);
    expect(metrics.sendHeight).toBe(30);
  });

  test("composer controls feel like a quiet conversation composer rail", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("@src/components/session");
    const fileOption = page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ });
    await expect(fileOption).toBeVisible();
    await fileOption.click();

    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    await expect(page.getByRole("menu")).toBeVisible();

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const chip = surface.querySelector<HTMLElement>(".forge-composer-chip");
      const remove = surface.querySelector<HTMLElement>(".forge-composer-chip-remove");
      const menu = document.querySelector<HTMLElement>(".forge-composer-model-menu");
      const activeOption = menu?.querySelector<HTMLElement>("[role='menuitemradio'][aria-checked='true']");
      const currentBadge = menu?.querySelector<HTMLElement>("[data-testid='composer-model-current-badge']");
      if (!toolbar || !chip || !remove || !menu || !activeOption || !currentBadge) return null;
      const toolbarStyle = getComputedStyle(toolbar);
      const chipStyle = getComputedStyle(chip);
      const removeStyle = getComputedStyle(remove);
      const activeStyle = getComputedStyle(activeOption);
      const badgeStyle = getComputedStyle(currentBadge);
      const removeRect = remove.getBoundingClientRect();
      const badgeRect = currentBadge.getBoundingClientRect();
      return {
        toolbarBorderTopWidth: Math.round(Number.parseFloat(toolbarStyle.borderTopWidth)),
        toolbarBackground: toolbarStyle.backgroundColor,
        chipBackground: chipStyle.backgroundColor,
        chipBorder: chipStyle.borderTopColor,
        removeWidth: Math.round(removeRect.width),
        removeHeight: Math.round(removeRect.height),
        removeBorder: removeStyle.borderTopColor,
        removeRadius: Number.parseFloat(removeStyle.borderTopLeftRadius),
        activeOptionBackground: activeStyle.backgroundColor,
        activeOptionBorder: activeStyle.borderTopColor,
        badgeHeight: Math.round(badgeRect.height),
        badgeBorder: badgeStyle.borderTopColor,
        badgeRadius: Number.parseFloat(badgeStyle.borderTopLeftRadius),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.toolbarBorderTopWidth).toBe(0);
    expect(metrics!.toolbarBackground).toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.chipBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.chipBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removeWidth).toBe(18);
    expect(metrics!.removeHeight).toBe(18);
    expect(metrics!.removeBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.removeRadius).toBeLessThanOrEqual(6);
    expect(metrics!.activeOptionBackground).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.activeOptionBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.badgeHeight).toBe(18);
    expect(metrics!.badgeBorder).not.toBe("rgba(0, 0, 0, 0)");
    expect(metrics!.badgeRadius).toBeLessThanOrEqual(6);
  });

  test("composer suggestion menu and selected references stay visually bounded", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    await textarea.fill("/");
    const commandMenu = page.getByTestId("composer-command-menu");
    await expect(commandMenu).toBeVisible();

    const menuMetrics = await commandMenu.evaluate((node) => {
      const option = node.querySelector<HTMLElement>('[role="option"]');
      const style = getComputedStyle(node);
      return {
        radius: Number.parseFloat(style.borderTopLeftRadius),
        background: style.backgroundColor,
        borderColor: style.borderTopColor,
        optionHeight: option ? Math.round(option.getBoundingClientRect().height) : 0,
      };
    });

    expect(menuMetrics.radius).toBeLessThanOrEqual(8);
    expect(menuMetrics.background).not.toBe("rgba(0, 0, 0, 0)");
    expect(menuMetrics.borderColor).not.toBe("rgba(0, 0, 0, 0)");
    expect(menuMetrics.optionHeight).toBeGreaterThanOrEqual(32);

    await page.keyboard.press("Escape");
    await textarea.fill("@src/components/session");
    const fileOption = page.getByRole("option", { name: /src\/components\/session\/InputBar\.tsx/ });
    await expect(fileOption).toBeVisible();
    await fileOption.click();

    const chipMetrics = await page.locator(".forge-composer-chip").first().evaluate((node) => {
      const style = getComputedStyle(node);
      const label = node.querySelector<HTMLElement>(".forge-composer-chip-label");
      const labelStyle = label ? getComputedStyle(label) : null;
      return {
        chipWidth: Math.round(node.getBoundingClientRect().width),
        maxWidth: Number.parseFloat(style.maxWidth),
        overflow: labelStyle?.overflow ?? "",
        textOverflow: labelStyle?.textOverflow ?? "",
        whiteSpace: labelStyle?.whiteSpace ?? "",
      };
    });

    expect(chipMetrics.chipWidth).toBeLessThanOrEqual(300);
    expect(chipMetrics.maxWidth).toBeLessThanOrEqual(300);
    expect(chipMetrics.overflow).toBe("hidden");
    expect(chipMetrics.textOverflow).toBe("ellipsis");
    expect(chipMetrics.whiteSpace).toBe("nowrap");
  });

  test("composer chip tray caps dense long references inside the input surface", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    const denseReferencePaths = [
      "src/features/deep-context/adapters/anthropic-session-stream-router.ts",
      "src/features/deep-context/adapters/openai-compatible-stream-router.ts",
      "src/features/deep-context/components/RunEvidenceTimeline.tsx",
      "src/features/deep-context/components/ProjectArchiveInspector.tsx",
      "src/features/deep-context/lib/workspace-boundary-policy.ts",
      "src/features/deep-context/lib/markdown-diagram-normalizer.ts",
    ];

    for (const path of denseReferencePaths) {
      await textarea.fill("@deep-context");
      await expect(page.getByTestId("composer-command-menu")).toBeVisible();
      const option = page.getByRole("option", { name: new RegExp(path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")) });
      await expect(option).toBeVisible();
      await option.scrollIntoViewIfNeeded();
      await option.click();
      await expect(textarea).toHaveValue("");
    }

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const chips = surface.querySelector<HTMLElement>(".forge-composer-chips");
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      if (!chips || !toolbar) return null;
      const chipsStyle = getComputedStyle(chips);
      const surfaceRect = surface.getBoundingClientRect();
      const chipsRect = chips.getBoundingClientRect();
      const toolbarRect = toolbar.getBoundingClientRect();
      return {
        chipCount: chips.querySelectorAll(".forge-composer-chip").length,
        overflowY: chipsStyle.overflowY,
        maxHeight: Math.round(Number.parseFloat(chipsStyle.maxHeight)),
        chipsClientHeight: Math.round(chips.clientHeight),
        chipsScrollHeight: Math.round(chips.scrollHeight),
        chipsWidth: Math.round(chipsRect.width),
        surfaceWidth: Math.round(surfaceRect.width),
        surfaceHeight: Math.round(surfaceRect.height),
        toolbarBottom: Math.round(toolbarRect.bottom),
        surfaceBottom: Math.round(surfaceRect.bottom),
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.chipCount).toBe(6);
    expect(metrics!.overflowY).toBe("auto");
    expect(metrics!.maxHeight).toBeLessThanOrEqual(68);
    expect(metrics!.chipsScrollHeight).toBeGreaterThan(metrics!.chipsClientHeight);
    expect(metrics!.chipsWidth).toBeLessThanOrEqual(metrics!.surfaceWidth);
    expect(metrics!.surfaceHeight).toBeLessThanOrEqual(196);
    expect(metrics!.toolbarBottom).toBeLessThanOrEqual(metrics!.surfaceBottom);
  });

  test("composer remains bounded in a narrow desktop window with dense context", async ({ page }) => {
    await page.setViewportSize({ width: 760, height: 620 });
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    const textarea = page.locator("textarea");
    await expect(textarea).toBeVisible();

    const denseReferencePaths = [
      "src/features/deep-context/adapters/anthropic-session-stream-router.ts",
      "src/features/deep-context/adapters/openai-compatible-stream-router.ts",
      "src/features/deep-context/components/RunEvidenceTimeline.tsx",
      "src/features/deep-context/components/ProjectArchiveInspector.tsx",
      "src/features/deep-context/lib/workspace-boundary-policy.ts",
      "src/features/deep-context/lib/markdown-diagram-normalizer.ts",
    ];

    for (const path of denseReferencePaths) {
      await textarea.fill("@deep-context");
      await expect(page.getByTestId("composer-command-menu")).toBeVisible();
      const option = page.getByRole("option", { name: new RegExp(path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")) });
      await expect(option).toBeVisible();
      await option.scrollIntoViewIfNeeded();
      await option.click();
    }
    await textarea.fill(Array.from({ length: 18 }, (_, index) => `第 ${index + 1} 行：继续描述细节。`).join("\n"));

    const metrics = await page.getByTestId("composer-surface").evaluate((node) => {
      const surface = node as HTMLElement;
      const toolbar = surface.querySelector<HTMLElement>("[data-testid='composer-toolbar']");
      const controlCluster = surface.querySelector<HTMLElement>("[data-testid='composer-control-cluster']");
      const toolCluster = surface.querySelector<HTMLElement>("[data-testid='composer-tool-cluster']");
      const model = surface.querySelector<HTMLElement>("[data-testid='composer-model-chip']");
      const send = surface.querySelector<HTMLElement>("[data-testid='composer-send']");
      const textarea = surface.querySelector<HTMLTextAreaElement>("textarea");
      if (!toolbar || !controlCluster || !toolCluster || !model || !send || !textarea) return null;
      const surfaceRect = surface.getBoundingClientRect();
      const toolbarRect = toolbar.getBoundingClientRect();
      const controlRect = controlCluster.getBoundingClientRect();
      const toolRect = toolCluster.getBoundingClientRect();
      const modelRect = model.getBoundingClientRect();
      const sendRect = send.getBoundingClientRect();
      const toolbarStyle = getComputedStyle(toolbar);
      return {
        surfaceWidth: Math.round(surfaceRect.width),
        surfaceHeight: Math.round(surfaceRect.height),
        toolbarWrap: toolbarStyle.flexWrap,
        toolbarHeight: Math.round(toolbarRect.height),
        controlRight: Math.round(controlRect.right - surfaceRect.right),
        toolLeft: Math.round(toolRect.left - surfaceRect.left),
        modelWidth: Math.round(modelRect.width),
        sendRight: Math.round(sendRect.right - surfaceRect.right),
        inputHeight: Math.round(textarea.getBoundingClientRect().height),
        canScrollInside: textarea.scrollHeight > textarea.clientHeight,
      };
    });

    expect(metrics).not.toBeNull();
    expect(metrics!.surfaceWidth).toBeLessThanOrEqual(500);
    expect(metrics!.surfaceHeight).toBeLessThanOrEqual(250);
    expect(metrics!.toolbarWrap).toBe("wrap");
    expect(metrics!.toolbarHeight).toBeLessThanOrEqual(76);
    expect(metrics!.toolLeft).toBeGreaterThanOrEqual(16);
    expect(metrics!.controlRight).toBeLessThanOrEqual(-16);
    expect(metrics!.sendRight).toBeLessThanOrEqual(-16);
    expect(metrics!.modelWidth).toBeLessThanOrEqual(188);
    expect(metrics!.inputHeight).toBeLessThanOrEqual(128);
    expect(metrics!.canScrollInside).toBe(true);
  });

  test("composer model menu floats above the composer instead of being clipped by it", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ }).click();
    const menu = page.getByRole("menu");
    await expect(menu).toBeVisible();

    const metrics = await menu.evaluate((node) => {
      const menuRect = node.getBoundingClientRect();
      const surface = document.querySelector<HTMLElement>("[data-testid='composer-surface']");
      const surfaceRect = surface?.getBoundingClientRect();
      const hit = document.elementFromPoint(menuRect.left + 12, menuRect.top + 12);
      return {
        menuBottom: Math.round(menuRect.bottom),
        surfaceTop: surfaceRect ? Math.round(surfaceRect.top) : 0,
        topHitIsMenu: hit === node || Boolean(hit?.closest("[role='menu']")),
      };
    });

    expect(metrics.menuBottom).toBeLessThanOrEqual(metrics.surfaceTop - 6);
    expect(metrics.topHitIsMenu).toBe(true);
  });

  test("composer menus close when focus moves back to the transcript", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const surface = page.getByTestId("composer-surface");
    const slash = composer.getByRole("button", { name: "常用请求" });

    await slash.click();
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();
    await page.getByTestId("message-lane").click();

    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expect(surface).toHaveAttribute("data-menu-open", "false");
    await expect(slash).toHaveAttribute("data-active", "false");
  });

  test("composer only keeps one floating menu open at a time", async ({ page }) => {
    const sessionId = crypto.randomUUID();
    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);
    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const composer = page.getByTestId("composer-lane");
    const slash = composer.getByRole("button", { name: "常用请求" });
    const model = composer.getByRole("button", { name: /模型：DeepSeek V4 Flash 1M/ });

    await slash.click();
    await expect(page.getByTestId("composer-command-menu")).toBeVisible();
    await model.click();

    await expect(page.getByTestId("composer-command-menu")).toHaveCount(0);
    await expect(model).toHaveAttribute("aria-expanded", "true");
    await expect(slash).toHaveAttribute("data-active", "false");
  });
});

test.describe("First loop v0", () => {
  test("supports the first small-tool loop skeleton", async ({ page }) => {
    const sessionId = "first-loop-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const request = "我想做一个番茄钟小工具，可以开始、暂停、重置。";
    await page.locator("textarea").fill(request);
    await page.locator("textarea").press("Enter");

    await expect(page.getByRole("main").getByText(request, { exact: true }).last()).toBeVisible();

    await page.getByTitle("打开项目档案").click();
    const archive = page.locator("aside").last();

    await expect(archive.getByText("项目档案", { exact: true }).first()).toBeVisible();
    const firstVersion = archive.locator("section").filter({ hasText: "第一版" });
    await expect(firstVersion.getByRole("heading", { name: "第一版" })).toBeVisible();
    await expect(firstVersion.getByText("可见、可点、可继续")).toBeVisible();
    await expect(firstVersion.getByText("番茄钟小工具").first()).toBeVisible();
    await expect(firstVersion.getByText("开始、暂停、重置").first()).toBeVisible();
    await expect(firstVersion.getByText("下一步", { exact: true }).first()).toBeVisible();
    await expect(archive.getByRole("heading", { name: "本轮参考" })).toHaveCount(0);
    await expect(archive.getByText("工作台", { exact: true })).toHaveCount(0);
  });

  test("shows a delivery summary after sending a first-loop request", async ({ page }) => {
    const sessionId = "first-loop-delivery-summary";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-v0-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

    const main = page.getByRole("main");
    await expect(main.getByText("本轮交付")).toBeVisible();
    await expect(main.getByText("预览未运行")).toBeVisible();
    await expect(main.getByText("下一步", { exact: true })).toBeVisible();
  });
});

test.describe("First loop v1", () => {
  test("empty session shows start readiness", async ({ page }) => {
    const sessionId = "first-loop-readiness";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const main = page.getByRole("main");
    await expect(main.getByText("准备开始")).toBeVisible();
    const readiness = main.getByTestId("start-readiness");
    await expect(readiness).toBeVisible();
    await expect(readiness).toHaveCSS("border-top-width", "0px");
    await expect(main.getByText("工作空间")).toHaveCount(0);
    await expect(main.getByText("模型密钥")).toHaveCount(0);
    await expect(main.getByText("预览", { exact: true })).toHaveCount(0);
    await expect(main.getByText("检查点", { exact: true })).toHaveCount(0);
    await expect(main.getByText("理解目标")).toHaveCount(0);
    await expect(main.getByText("准备修改")).toHaveCount(0);
  });

  test("start readiness surfaces missing provider setup before the first prompt", async ({ page }) => {
    const sessionId = "first-loop-missing-provider";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge-test-app");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "get_api_key_status") return [{ provider: "deepseek", set: false, preview: "" }];
        return original?.(cmd, args);
      };
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    const readiness = page.getByTestId("start-readiness");
    await expect(readiness).toBeVisible();
    await expect(readiness.getByText("需要配置模型密钥")).toBeVisible();
    await expect(readiness.getByText("还没有配置 DeepSeek")).toBeVisible();
    await expect(readiness.getByText("forge-test-app")).toBeVisible();
    await expect(readiness.getByText("/Users/cabbos/project/forge-test-app")).toHaveCount(0);
    await expect(readiness.getByText("工作空间")).toHaveCount(0);
    await expect(readiness.getByText("检查点")).toHaveCount(0);

    await readiness.getByRole("button", { name: "打开设置" }).click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "模型服务" })).toBeVisible();
  });

  test("first loop keeps progress implicit in the conversation", async ({ page }) => {
    const sessionId = "first-loop-progress";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    await expect(page.getByText("理解目标")).toHaveCount(0);

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-progress-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

    await expect(page.getByText("正在制作")).toHaveCount(0);
    await expect(page.getByText("等你验收")).toHaveCount(0);
    await expect(page.getByText("本轮交付")).toBeVisible();
  });

  test("delivery summary offers follow-up actions", async ({ page }) => {
    const sessionId = "first-loop-delivery-actions";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-actions-delivery",
        summary: {
          project_path: "/Users/cabbos/project/forge",
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

    await expect(page.getByText("验收提示", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "检查风险" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "开始验收" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "继续优化" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "检查这版" })).toBeVisible();

    await page.getByRole("button", { name: "检查这版" }).click();
    await expect(page.locator("textarea")).toHaveValue(/检查当前版本有没有明显问题/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
  });

  test("first loop binds to the active test app without exposing the full path", async ({ page }) => {
    const sessionId = "first-loop-test-app";
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    await setup(page);
    await page.addInitScript(({ sessionId, sandboxPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", sandboxPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, sandboxPath });

    await page.goto("http://localhost:1420");
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill("我想做一个番茄钟小工具，可以开始、暂停、重置。");
    await page.locator("textarea").press("Enter");
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await simulateStream(page, sessionId, [
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "first-loop-test-app-delivery",
        summary: {
          project_path: sandboxPath,
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：检查当前版本。",
        },
      },
    ], 1);

    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    const sendArgs = await expectLastSendInputArgs(page, { sessionId });
    const sentText = String(sendArgs.text);
    expect(createArgs.workingDir).toBe(sandboxPath);
    expect(sentText).toContain("Forge 第一闭环提示");
    expect(sentText).toContain("可见、可点、可继续");
    expect(sentText).not.toContain("目标项目：");

    const main = page.getByRole("main");
    const delivery = main.locator("div").filter({ hasText: "本轮交付" }).filter({ hasText: "预览未运行" }).last();
    await expect(delivery).toBeVisible();
    await expect(delivery.getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(delivery.getByText(sandboxPath, { exact: true })).toHaveCount(0);
  });

  test("demo ledger first loop reaches repair, delivery, and project archive", async ({ page }) => {
    const sessionId = "demo-ledger-first-loop";
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    const request = "请为收支记录工具做第一版：支持新增收入或支出、展示明细列表，并在页面顶部汇总当前结余。";
    const proposal = {
      id: "demo-ledger-record-proposal",
      project_path: sandboxPath,
      session_id: sessionId,
      target_pages: ["tasks.md", "log.md"],
      title: "记录收支小工具第一版",
      summary: "补充收支记录第一版、检查结果和下一步验收事项。",
      patch_preview: "追加本轮第一版验收记录。",
      status: "pending" as const,
      created_at: "2026-05-17T00:00:00.000Z",
    };

    await setup(page);
    await page.addInitScript(({ sessionId, sandboxPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", sandboxPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "confirm_response") {
          // @ts-expect-error mock
          window.__lastConfirmResponseArgs = args;
          return undefined;
        }
        return original?.(cmd, args);
      };
    }, { sessionId, sandboxPath });

    await page.goto("http://localhost:1420");
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(page.getByLabel("当前项目边界").getByText(sandboxPath, { exact: true })).toHaveCount(0);

    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.locator("textarea").fill(request);
    await page.locator("textarea").press("Enter");
    await expect(page.getByRole("main").getByText(request, { exact: true }).last()).toBeVisible();

    const createArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastCreateSessionArgs;
    });
    expect(createArgs.workingDir).toBe(sandboxPath);

    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "demo-ledger-progress" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "demo-ledger-progress",
        content: "我先把收支记录的最小闭环接起来，再跑一次构建检查。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "demo-ledger-progress" },
      {
        event_type: "confirm_ask",
        session_id: sessionId,
        block_id: "demo-ledger-confirm",
        question: "Allow write_file?",
        kind: "file_write",
        boundary: {
          title: "准备修改项目",
          workspace_name: "forge-test-app",
          workspace_path: sandboxPath,
          operation: "write_file",
          affected_files: ["src/App.tsx", "src/App.css"],
          impact: "将修改 2 个文件",
          risk: "caution",
          recovery: "交付区会显示预览和检查点状态。",
          command: null,
          warning: null,
        },
      },
      { event_type: "tool_call_start", session_id: sessionId, block_id: "demo-ledger-read", tool_name: "read_file", tool_input: { path: "src/App.tsx" } },
      { event_type: "tool_call_result", session_id: sessionId, block_id: "demo-ledger-read", result: "找到现有入口。", is_error: false, duration_ms: 24 },
      { event_type: "shell_start", session_id: sessionId, block_id: "demo-ledger-failed-build", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "demo-ledger-failed-build", content: "src/App.tsx: 收支金额字段类型需要修复\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "demo-ledger-failed-build", exit_code: 1 },
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "demo-ledger-failed-delivery",
        summary: {
          project_path: sandboxPath,
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：先修复构建检查未通过的问题。",
          verification_label: "检查未通过",
          verification_status: "failed",
          verification_command: "npm run build",
        },
      },
    ], 1);

    const confirmCard = page.getByTestId("message-panel").filter({ hasText: "准备修改项目" });
    await expect(confirmCard.getByText("forge-test-app")).toBeVisible();
    await expect(confirmCard).not.toContainText(sandboxPath);
    await expect(confirmCard).not.toContainText("/Users/");
    await expect(confirmCard.getByText("src/App.tsx", { exact: true })).toBeVisible();
    await expect(confirmCard.getByText(/ConfirmAsk|permission/i)).toHaveCount(0);
    await expect(confirmCard.getByText("forge", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("main")).not.toContainText(sandboxPath);
    await confirmCard.getByRole("button", { name: "继续" }).click();
    const confirmArgs = await page.evaluate(() => {
      // @ts-expect-error mock
      return window.__lastConfirmResponseArgs;
    });
    expect(confirmArgs).toEqual({ blockId: "demo-ledger-confirm", approved: true });

    const failedDelivery = page.getByTestId("message-panel").filter({ hasText: "本轮交付" }).filter({ hasText: "检查未通过" });
    await expect(failedDelivery.getByText("forge-test-app", { exact: true })).toBeVisible();
    await failedDelivery.getByRole("button", { name: "继续修复" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续修复/);
    await expect(page.locator("textarea")).toHaveValue(/npm run build/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);

    await page.locator("textarea").press("Enter");
    const repairSendArgs = await expectLastSendInputArgs(page, { sessionId });
    const repairPrompt = String(repairSendArgs.text);
    expect(repairPrompt).toContain("继续修复");
    expect(repairPrompt).toContain("npm run build");
    expect(repairPrompt).not.toContain("目标项目：");

    await simulateStream(page, sessionId, [
      { event_type: "text_start", session_id: sessionId, block_id: "demo-ledger-repair-progress" },
      {
        event_type: "text_chunk",
        session_id: sessionId,
        block_id: "demo-ledger-repair-progress",
        content: "金额字段已经收窄，收支合计可以继续验收。",
      },
      { event_type: "text_end", session_id: sessionId, block_id: "demo-ledger-repair-progress" },
      { event_type: "shell_start", session_id: sessionId, block_id: "demo-ledger-success-build", command: "npm run build" },
      { event_type: "shell_output", session_id: sessionId, block_id: "demo-ledger-success-build", content: "✓ built in 640ms\n" },
      { event_type: "shell_end", session_id: sessionId, block_id: "demo-ledger-success-build", exit_code: 0 },
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
      {
        event_type: "delivery_summary",
        session_id: sessionId,
        block_id: "demo-ledger-success-delivery",
        summary: {
          project_path: sandboxPath,
          preview_label: "预览未运行",
          checkpoint_label: "检查点已就绪",
          next_action: "下一步：验收添加收支和合计展示。",
          verification_label: "检查通过",
          verification_status: "passed",
          verification_command: "npm run build",
          record_label: "建议更新项目记录",
          record_status: "pending",
          record_target_pages: ["tasks.md", "log.md"],
        },
      },
    ], 1);

    const successfulDelivery = page.getByTestId("message-panel").filter({ hasText: "本轮交付" }).filter({ hasText: "检查通过" });
    await expect(successfulDelivery.getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(successfulDelivery.getByText("预览未运行")).toBeVisible();
    await expect(successfulDelivery.getByText("检查点已就绪")).toBeVisible();
    await expect(successfulDelivery.getByText("检查通过", { exact: true })).toBeVisible();
    await expect(successfulDelivery.getByText("自动记录")).toBeVisible();
    await expect(page.getByRole("main").getByText(sandboxPath, { exact: true })).toHaveCount(0);
    await expect(page.getByRole("main").getByText(/Workflow Router|Task Mode|Living Wiki|Forge Wiki|writeback|ConfirmAsk|permission/i)).toHaveCount(0);
    await expect(page.getByRole("main").getByText(/示例|玩具|临时/)).toHaveCount(0);

    await successfulDelivery.getByRole("button", { name: "查看记录" }).click();

    const archive = projectArchive(page);
    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("forge-test-app", { exact: true }).first()).toBeVisible();
    await expect(archive.getByText(sandboxPath, { exact: true })).toHaveCount(0);

    const records = await expandArchiveRecords(page);
    await expect(records.getByRole("heading", { name: "建议更新记录" })).toBeVisible();
    await expect(records.getByText(proposal.summary)).toBeVisible();
    await expect(records.getByText("保存位置")).toBeVisible();
    await expect(records.getByText("项目记录页面")).toBeVisible();
    await expect(records.getByText("tasks.md, log.md")).toBeVisible();
    await expect(records.getByRole("button", { name: "接受" })).toBeVisible();
    await expect(records.getByRole("button", { name: "丢弃" })).toBeVisible();
    await expect(records.getByText(/Workflow Router|Task Mode|Living Wiki|Forge Wiki|writeback/)).toHaveCount(0);
  });

  test("demo workspace resume returns to project overview without path leakage", async ({ page }) => {
    const sessionId = "demo-ledger-return-session";
    const sandboxPath = "/Users/cabbos/project/forge-test-app";
    const summary = {
      project_path: sandboxPath,
      preview_label: "预览未运行",
      checkpoint_label: "检查点已就绪",
      next_action: "下一步：验收添加收支和合计展示。",
      verification_label: "检查通过",
      verification_status: "passed",
      verification_command: "npm run build",
      record_label: "建议更新项目记录",
      record_status: "pending",
      record_target_pages: ["tasks.md", "log.md"],
    };

    await setup(page);
    await page.addInitScript((sandboxPath) => {
      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "get_project_runtime_status") {
          return {
            working_dir: sandboxPath,
            has_package_json: true,
            package_manager: "npm",
            dev_script: "dev",
            command: "npm run dev",
            port: 1420,
            url: "http://localhost:1420",
            running: false,
            managed: false,
            pid: null,
            can_start: true,
            can_stop: false,
            can_open: true,
            message: "Preview not running",
            logs: [],
          };
        }
        if (cmd === "get_project_checkpoint_status") {
          return {
            working_dir: sandboxPath,
            is_git_repo: true,
            dirty: false,
            last_checkpoint: null,
            message: "No checkpoint yet",
          };
        }
        return original?.(cmd, args);
      };
    }, sandboxPath);

    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, sandboxPath, summary }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", sandboxPath);

      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: sandboxPath, name: "forge-test-app", path: sandboxPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(sandboxPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: sandboxPath,
          workspaceId: sandboxPath,
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
          deliverySummary: summary,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "demo-return-user-message",
          event_type: "user_message",
          content: "请为收支记录工具做第一版：支持新增收入或支出、展示明细列表，并在页面顶部汇总当前结余。",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "demo-return-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: { summary },
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, sandboxPath, summary });

    await page.reload();
    await expect(page.getByLabel("当前项目边界").getByText("forge-test-app", { exact: true })).toBeVisible();
    await expect(page.getByLabel("当前项目边界").getByText(sandboxPath, { exact: true })).toHaveCount(0);

    await page.getByTitle("打开项目档案").click();

    const archive = projectArchive(page);
    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("forge-test-app", { exact: true }).first()).toBeVisible();
    await expect(archive.getByText("收支记录工具")).toBeVisible();
    await expect(archive.getByText("预览未运行 · 检查点已就绪")).toBeVisible();
    await expect(archive.getByText("下一步：验收添加收支和合计展示。")).toBeVisible();
    await expect(archive.getByText(sandboxPath, { exact: true })).toHaveCount(0);

    await archive.getByRole("button", { name: "继续上次任务" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续上次任务/);
    await expect(page.locator("textarea")).toHaveValue(/收支记录工具/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
    await expect(page.locator("textarea")).not.toHaveValue(new RegExp(sandboxPath.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  });
});

test.describe("Project Archive v1", () => {
  test("project archive hides empty loop and low-level metadata by default", async ({ page }) => {
    const sessionId = "project-archive-quiet-empty";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    await page.getByTitle("打开项目档案").click();
    const archive = projectArchive(page);
    await expect(page.getByRole("complementary", { name: "项目档案" })).toBeVisible();
    const archiveWidth = (await archive.boundingBox())?.width ?? 0;
    expect(archiveWidth).toBeLessThanOrEqual(304);
    const modalBackdropCount = await page.evaluate(() =>
      [...document.querySelectorAll("div")].filter((node) => {
        const className = String(node.getAttribute("class") ?? "");
        return className.includes("fixed inset-0") && className.includes("bg-black/20");
      }).length,
    );
    expect(modalBackdropCount).toBe(0);

    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("forge", { exact: true }).first()).toBeVisible();
    await expect(archive.getByTestId("archive-disclosure-records")).toBeVisible();
    await expect(archive.getByTestId("archive-disclosure-files")).toBeVisible();
    await expect(archive.getByText("还没有项目记录", { exact: true })).toHaveCount(0);
    await expect(archive.getByText("文件名", { exact: true })).toHaveCount(0);
    await expect(archive.getByRole("heading", { name: "第一版" })).toHaveCount(0);
    await expect(archive.getByText("小工具闭环")).toHaveCount(0);
    await expect(archive.getByText(projectPath, { exact: true })).toHaveCount(0);
    await expect(archive.getByText("上下文长度")).toHaveCount(0);
    await expect(archive.getByText("$0.00")).toHaveCount(0);
    await expect(archive.getByText("工作方式")).toHaveCount(0);

    await expandArchiveFiles(page);
    await expect(archive.getByText("文件名", { exact: true })).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(page.getByRole("complementary", { name: "项目档案" })).toHaveCount(0);
  });

  test("project archive lists available connector materials without reading them", async ({ page }) => {
    const sessionId = "project-archive-connector-materials";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.getByTitle("打开项目档案").click();

    const archive = projectArchive(page);
    const materials = await expandArchiveFiles(page);

    await expect(materials.getByText("Forge 研发记录")).toBeVisible();
    await expect(materials.getByText("summarize_issue")).toBeVisible();
    await expect(materials.getByText("连接资料 · obsidian")).toBeVisible();
    await expect(materials.getByText("连接提示词 · linear")).toBeVisible();
    await expect(materials.getByText("可用")).toHaveCount(2);
    await expect(materials.getByText("未加入")).toHaveCount(2);
    await expect(archive.getByText("还没有添加资料")).toHaveCount(0);
  });

  test("selected connector material is sent as hidden turn context", async ({ page }) => {
    const sessionId = "project-archive-selected-material";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.getByTitle("打开项目档案").click();

    const materials = await expandArchiveFiles(page);
    await materials.getByRole("button", { name: /Forge 研发记录/ }).click();
    await expect(materials.getByText("已加入")).toHaveCount(1);

    await page.locator("textarea").fill("根据资料总结下一步");
    await page.locator("textarea").press("Enter");

    await expectLastSendInputArgs(page, {
      sessionId,
      mcpContext: [
        {
          kind: "resource",
          server_id: "obsidian",
          uri: "file:///notes/forge.md",
          name: "Forge 研发记录",
        },
      ],
    });
    const userMessage = page.getByTestId("user-message").last();
    await expect(userMessage.getByText("根据资料总结下一步", { exact: true })).toBeVisible();
    await expect(userMessage.getByText("file:///notes/forge.md", { exact: true })).toHaveCount(0);
  });

  test("connector prompt arguments are collected before joining context", async ({ page }) => {
    const sessionId = "project-archive-prompt-arguments";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.getByTitle("打开项目档案").click();

    const materials = await expandArchiveFiles(page);
    await materials.getByRole("button", { name: /summarize_issue/ }).click();
    await materials.getByLabel("focus").fill("安全风险");
    await materials.getByRole("button", { name: "加入本轮" }).click();
    await expect(materials.getByText("已加入")).toHaveCount(1);

    await page.locator("textarea").fill("按提示词整理一下");
    await page.locator("textarea").press("Enter");

    await expectLastSendInputArgs(page, {
      sessionId,
      mcpContext: [
        {
          kind: "prompt",
          server_id: "linear",
          name: "summarize_issue",
          arguments: {
            focus: "安全风险",
          },
        },
      ],
    });
  });

  test("connector material row shows read failure status", async ({ page }) => {
    const sessionId = "project-archive-connector-read-failed";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.getByTitle("打开项目档案").click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    const materials = await expandArchiveFiles(page);
    const row = materials.getByRole("button", { name: /Forge 研发记录/ });
    await row.click();

    await page.evaluate((sessionId) => {
      // @ts-expect-error listeners
      for (const listener of window.__tauriListeners?.["session-output"] ?? []) {
        listener({
          payload: {
            event_type: "mcp_context_status",
            session_id: sessionId,
            source_id: "mcp-resource:obsidian:file:///notes/forge.md",
            status: "failed",
            message: "连接资料读取失败",
          },
        });
      }
    }, sessionId);

    await expect(row.getByText("读取失败", { exact: true })).toBeVisible();
  });

  test("restored project archive shows overview and continuation actions", async ({ page }) => {
    const sessionId = "project-archive-return-session";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.goto("http://localhost:1420");
    await page.evaluate(async ({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);

      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
      });
      const tx = db.transaction("keyval", "readwrite");
      tx.objectStore("keyval").put([
        { id: projectPath, name: "forge", path: projectPath, lastOpenedAt: 1 },
      ], "forge-workspaces");
      tx.objectStore("keyval").put(projectPath, "forge-active-workspace");
      tx.objectStore("keyval").put([
        {
          id: sessionId,
          agentType: "deepseek",
          model: "deepseek-v4-flash[1m]",
          workingDir: projectPath,
          workspaceId: projectPath,
          contextWindowTokens: 1_000_000,
          status: "stopped",
          workflowState: null,
        },
      ], "forge-sessions");
      tx.objectStore("keyval").put(sessionId, "forge-active-session");
      tx.objectStore("keyval").put([
        {
          block_id: "return-user-message",
          event_type: "user_message",
          content: "我想做一个番茄钟小工具，可以开始、暂停、重置。",
          isComplete: true,
          metadata: {},
        },
        {
          block_id: "return-delivery-summary",
          event_type: "delivery_summary",
          content: "本轮交付",
          isComplete: true,
          metadata: {
            summary: {
              project_path: projectPath,
              preview_label: "预览可打开",
              checkpoint_label: "检查点已就绪",
              next_action: "下一步：继续调整计时器的视觉反馈。",
              record_label: "建议更新项目记录",
              record_status: "pending",
              record_target_pages: ["tasks.md", "decisions.md"],
            },
          },
        },
      ], `forge-blocks:${sessionId}`);
      await new Promise<void>((resolve, reject) => {
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
      db.close();
    }, { sessionId, projectPath });

    await page.reload();
    await page.getByTitle("打开项目档案").click();

    const archive = projectArchive(page);
    await expect(archive.getByRole("heading", { name: "项目概览" })).toBeVisible();
    await expect(archive.getByText("番茄钟小工具")).toBeVisible();
    await expect(archive.getByText("预览可打开 · 检查点已就绪")).toBeVisible();
    await expect(archive.getByText("下一步：继续调整计时器的视觉反馈。")).toBeVisible();
    const overview = archive.locator("section").filter({ has: page.getByRole("heading", { name: "项目概览" }) });
    await expect(overview.getByText("自动记录", { exact: true })).toBeVisible();
    await expect(overview.getByText("建议更新项目记录")).toBeVisible();
    await expect(overview.getByText("tasks.md, decisions.md")).toBeVisible();
    await expect(overview.getByRole("button", { name: "查看记录" })).toBeVisible();
    await expect(archive.getByRole("button", { name: "继续上次任务" })).toBeVisible();

    await archive.getByRole("button", { name: "继续上次任务" }).click();
    await expect(page.locator("textarea")).toHaveValue(/继续上次任务/);
    await expect(page.locator("textarea")).not.toHaveValue(/目标项目：/);
  });
});

test.describe("Project records context panel", () => {
  test("project records panel initializes records and shows selected pages", async ({ page }) => {
    const sessionId = "forge-wiki-session";
    const projectPath = "/Users/cabbos/project/forge";
    const now = "2026-05-13T00:00:00.000Z";
    const selectedPage = {
      page_id: "tasks",
      title: "当前任务",
      path: "tasks.md",
      kind: "tasks" as const,
      summary: "覆盖当前 e2e 任务和验收步骤。",
      score: 0.97,
      reason: "和当前任务最相关",
      injected: true,
    };
    const proposal = {
      id: "proposal-1",
      project_path: projectPath,
      session_id: sessionId,
      target_pages: ["tasks.md"],
      title: "记录项目进展覆盖",
      summary: "补充上下文面板初始化、带入页面和更新建议的测试记录。",
      patch_preview: "追加 e2e 覆盖说明。",
      status: "pending" as const,
      created_at: now,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await openProjectArchive(page, "records");
    const recordsDisclosure = await expandArchiveRecords(page);
    const projectRecords = recordsDisclosure.locator("section").filter({ has: page.getByRole("heading", { name: "项目记录" }) });
    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });
    const updateProposals = recordsDisclosure.locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

    await expect(projectRecords.getByText("还没有项目记录", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "建立项目记录" }).click();
    await expect(projectRecords.getByText(/当前任务|项目概览/).first()).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await expect(selectedContext.getByText(selectedPage.summary)).toBeVisible();
    await expect(selectedContext.getByText("已参考 1 条档案")).toBeVisible();

    await simulateStream(page, sessionId, [
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    await expect(updateProposals.getByText(proposal.summary)).toBeVisible();
  });

  test("shows selected context, project records, delivery status, and scoped saved background", async ({ page }) => {
    const sessionId = "living-wiki-session";
    const projectPath = "/Users/cabbos/project/forge";
    const otherProjectPath = "/Users/cabbos/project/elsewhere";
    const now = "2026-05-13T00:00:00.000Z";
    const selectedMemory = {
      id: "memory-selected-1",
      category: "preference",
      scope: "user_profile",
      status: "accepted",
      title: "使用项目记录",
      body: "Selected background should travel with the next prompt.",
      project_path: null,
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.91,
      created_at: now,
      updated_at: now,
      last_used_at: now,
      use_count: 2,
      tags: ["context"],
    };
    const projectMemory = {
      id: "memory-project-1",
      category: "project_fact",
      scope: "project",
      status: "pinned",
      title: "项目档案",
      body: "当前项目使用项目档案查看本轮参考。",
      project_path: projectPath,
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.88,
      created_at: now,
      updated_at: now,
      last_used_at: null,
      use_count: 1,
      tags: ["wiki"],
    };
    const otherProjectMemory = {
      ...projectMemory,
      id: "memory-other-project",
      title: "Other project fact",
      body: "这条背景属于另一个项目，不应该显示。",
      project_path: otherProjectPath,
    };
    const candidateMemory = {
      ...projectMemory,
      id: "memory-candidate-1",
      category: "decision",
      status: "candidate",
      title: "建议记录项目档案变化",
      body: "This candidate should be visible before it is accepted.",
      confidence: 0.72,
      tags: ["candidate"],
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath, memories }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);

      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        switch (cmd) {
          case "create_session":
            return { session_id: sessionId };
          case "get_default_working_dir":
            return projectPath;
          case "get_project_runtime_status":
            return {
              working_dir: projectPath,
              has_package_json: true,
              package_manager: "npm",
              dev_script: "dev",
              command: "npm run dev",
              port: 1420,
              url: "http://localhost:1420",
              running: true,
              managed: true,
              pid: 4242,
              can_start: false,
              can_stop: true,
              can_open: true,
              message: "Preview running",
              logs: [],
            };
          case "get_project_checkpoint_status":
            return {
              working_dir: projectPath,
              is_git_repo: true,
              dirty: false,
              last_checkpoint: null,
              message: "No checkpoint yet",
            };
          case "list_memories":
            return memories;
          default:
            return original?.(cmd, args);
        }
      };
    }, { sessionId, projectPath, memories: [selectedMemory, projectMemory, otherProjectMemory, candidateMemory] });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "memory_updated", session_id: sessionId, memory: selectedMemory },
      { event_type: "memory_updated", session_id: sessionId, memory: projectMemory },
      { event_type: "memory_updated", session_id: sessionId, memory: otherProjectMemory },
      { event_type: "memory_candidate", session_id: sessionId, memory: candidateMemory },
      {
        event_type: "memory_selection",
        session_id: sessionId,
        selected: [
          {
            memory_id: selectedMemory.id,
            title: selectedMemory.title,
            body: selectedMemory.body,
            category: selectedMemory.category,
            scope: selectedMemory.scope,
            score: 0.96,
            reason: "Relevant to the active task",
            injected: true,
          },
        ],
      },
    ], 5);

    await expect(page.getByRole("main").getByText("本轮已参考 1 条档案")).toHaveCount(0);

    await page.getByTitle("打开项目档案").click();
    const recordsDisclosure = await expandArchiveRecords(page);

    const selectedContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });
    const projectMemories = recordsDisclosure.locator("section").filter({ has: page.getByRole("heading", { name: "已保存背景" }) });

    await expect(selectedContext.getByText(selectedMemory.body)).toBeVisible();
    await expect(recordsDisclosure.getByRole("heading", { name: "建议更新记录" })).toBeVisible();
    await expect(page.getByText(candidateMemory.body)).toBeVisible();
    await expect(page.getByTitle("接受")).toBeVisible();
    await expect(recordsDisclosure.getByRole("heading", { name: "项目记录", exact: true })).toBeVisible();
    await expect(projectMemories.getByText(projectMemory.body)).toBeVisible();
    await expect(projectMemories.getByText("项目信息", { exact: true })).toBeVisible();
    await expect(projectMemories.getByText("项目事实", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByText("最近状态")).toBeVisible();
    await expect(projectArchive(page).getByText("预览运行中")).toBeVisible();
    await expect(page.getByText(otherProjectMemory.body)).toHaveCount(0);
  });

  test("delivery shows preview action and checkpoint next step", async ({ page }) => {
    const sessionId = "delivery-action-session";
    const projectPath = "/Users/cabbos/project/forge";

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);

      // @ts-expect-error mock
      const original = window.__tauriMockIPC;
      // @ts-expect-error mock
      window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
        switch (cmd) {
          case "create_session":
            return { session_id: sessionId };
          case "get_default_working_dir":
            return projectPath;
          case "get_project_runtime_status":
            return {
              working_dir: projectPath,
              has_package_json: true,
              package_manager: "npm",
              dev_script: "dev",
              command: "npm run dev",
              port: 1420,
              url: "http://localhost:1420",
              running: false,
              managed: false,
              pid: null,
              can_start: true,
              can_stop: false,
              can_open: true,
              message: "Preview not running",
              logs: [],
            };
          case "get_project_checkpoint_status":
            return {
              working_dir: projectPath,
              is_git_repo: true,
              dirty: false,
              last_checkpoint: null,
              message: "No checkpoint yet",
            };
          case "start_project_dev_server":
          case "create_project_checkpoint":
            return undefined;
          case "list_memories":
            return [];
          default:
            return original?.(cmd, args);
        }
      };
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();

    const delivery = await openProjectArchive(page);

    await expect(delivery.getByText("预览未运行")).toBeVisible();
    await expect(delivery.getByRole("button", { name: "启动预览" })).toBeVisible();
    await expect(delivery.getByText("还没有检查点")).toBeVisible();
    await expect(delivery.getByRole("button", { name: "创建检查点" })).toBeVisible();
  });

  test("groups suggested background and project record updates", async ({ page }) => {
    const sessionId = "memory-inbox-session";
    const projectPath = "/Users/cabbos/project/forge";
    const now = "2026-05-13T00:00:00.000Z";
    const candidateMemory = {
      id: "candidate-1",
      category: "decision" as const,
      scope: "project" as const,
      status: "candidate" as const,
      title: "项目已定方案：项目档案优先",
      body: "右侧面板优先展示当前任务和本轮参考。",
      project_path: projectPath,
      source_session_id: sessionId,
      source_message_ids: [],
      confidence: 0.76,
      created_at: now,
      updated_at: now,
      last_used_at: null,
      use_count: 0,
      tags: ["decision"],
    };
    const proposal = {
      id: "proposal-1",
      project_path: projectPath,
      session_id: sessionId,
      target_pages: ["tasks.md"],
      title: "记录本轮参考计划",
      summary: "补充工作方式和本轮参考的下一步。",
      patch_preview: "追加任务记录。",
      status: "pending" as const,
      created_at: now,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });
    await page.getByTitle("打开项目档案").click();
    await expandArchiveRecords(page);

    await simulateStream(page, sessionId, [
      { event_type: "memory_candidate", session_id: sessionId, memory: candidateMemory },
      { event_type: "forge_wiki_update_proposed", session_id: sessionId, proposal },
    ], 5);

    const inbox = projectArchive(page).locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });

    await expect(inbox.getByText("确认后会进入项目记录或已保存背景")).toBeVisible();
    await expect(inbox.getByText("建议保存为已保存背景")).toBeVisible();
    await expect(inbox.getByText("建议写入项目记录")).toBeVisible();
    await expect(inbox.getByText("保存位置").first()).toBeVisible();
    await expect(inbox.getByText("项目记录页面")).toBeVisible();
    await expect(inbox.getByText("tasks.md")).toBeVisible();
    await expect(inbox.getByText("写入预览")).toBeVisible();
    await expect(inbox.getByText(proposal.patch_preview)).toBeVisible();
    await expect(inbox.getByText(candidateMemory.body)).toBeVisible();
    await expect(inbox.getByText(proposal.summary)).toBeVisible();
    await expect(inbox.getByRole("button", { name: "接受" }).first()).toBeVisible();
    await expect(inbox.getByRole("button", { name: /忘记|丢弃/ }).first()).toBeVisible();

    await inbox.getByRole("button", { name: "接受" }).last().click();
    await expect(inbox.getByText("已写入项目记录")).toBeVisible();
  });
});

test.describe("Work style controls", () => {
  test.beforeEach(async ({ page }) => {
    await setup(page);
  });

  test("shows soft workflow state and allows command palette override", async ({ page }) => {
    const sessionId = "workflow-router-session";
    const softWorkflow: WorkflowState = {
      session_id: sessionId,
      route: "workflow",
      phase: "clarifying",
      beginner_label: "先梳理想法",
      developer_label: "workflow",
      matched_signals: ["multi-part request"],
      reason: "这个需求会影响多个部分。",
      gate: "soft",
      override_actions: ["direct", "plan_first", "debug", "verify"],
      spec_path: null,
      plan_path: null,
      checkpoint_id: null,
      updated_at: Date.now(),
    };

    await page.addInitScript((sessionId) => {
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await expect(page.locator("textarea")).toBeVisible();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "workflow_updated", session_id: sessionId, state: softWorkflow },
    ], 5);

    await expect(page.getByTestId("workflow-status-pill")).toHaveCount(0);
    await expect(page.getByText("这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。", { exact: true })).toHaveCount(0);

    await page.keyboard.down("Control");
    await page.keyboard.press("k");
    await page.keyboard.up("Control");
    await expect(page.getByRole("option", { name: "打开项目档案" })).toBeVisible();
    await expect(page.getByRole("option", { name: "设置" })).toBeVisible();
    await expect(page.getByRole("dialog").getByText("工作方式", { exact: true })).toHaveCount(0);
    await page.getByRole("option", { name: "排查问题" }).click();

    await page.getByTitle("打开项目档案").click();
    const currentTask = page.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });
    await expect(currentTask.getByText("排查问题", { exact: true })).toBeVisible();
    await expect(currentTask.getByText("正在定位问题")).toBeVisible();
  });
});

test.describe("Current task work style", () => {
  test("shows stable mode copy without inline override controls", async ({ page }) => {
    const sessionId = "task-mode-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "workflow_updated",
        session_id: sessionId,
        state: {
          session_id: sessionId,
          route: "workflow",
          phase: "planning",
          beginner_label: "router raw label should not be the final UI label",
          developer_label: "workflow/planning",
          matched_signals: ["new feature", "multi component"],
          reason: "用户正在规划一个新能力。",
          gate: "soft",
          override_actions: ["direct", "plan_first", "debug", "verify"],
          spec_path: null,
          plan_path: null,
          checkpoint_id: null,
          updated_at: Date.now(),
        },
      },
    ], 5);

    await page.getByTitle("打开项目档案").click();
    const currentTask = page.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });

    await expect(currentTask.getByText("拆成步骤")).toBeVisible();
    await expect(currentTask.getByText("正在拆成可执行步骤")).toBeVisible();
    await expect(currentTask.getByText("这个需求会影响多个部分")).toBeVisible();
    await expect(currentTask.getByRole("button", { name: "直接回答" })).toHaveCount(0);
    await expect(currentTask.getByRole("button", { name: "先拆方案" })).toHaveCount(0);
    await expect(currentTask.getByRole("button", { name: "排查问题" })).toHaveCount(0);
    await expect(currentTask.getByRole("button", { name: "检查结果" })).toHaveCount(0);
    await expect(currentTask.getByText("开发者详情")).toHaveCount(0);
    await expect(currentTask.getByText("workflow/planning")).toHaveCount(0);
    await expect(currentTask.getByText("route")).toHaveCount(0);
    await expect(currentTask.getByText("phase")).toHaveCount(0);
  });

  test("keeps current task out of the top bar and shows it in Project Archive", async ({ page }) => {
    const sessionId = "top-level-mode-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      {
        event_type: "workflow_updated",
        session_id: sessionId,
        state: {
          session_id: sessionId,
          route: "workflow",
          phase: "clarifying",
          beginner_label: "raw",
          developer_label: "workflow/clarifying",
          matched_signals: ["idea"],
          reason: "用户正在描述一个新工具。",
          gate: "soft",
          override_actions: ["direct", "plan_first", "debug", "verify"],
          spec_path: null,
          plan_path: null,
          checkpoint_id: null,
          updated_at: Date.now(),
        },
      },
      {
        event_type: "forge_wiki_context_selected",
        session_id: sessionId,
        selected: [{
          page_id: "tasks",
          title: "当前任务",
          path: "tasks.md",
          kind: "tasks",
          summary: "正在收拢项目档案。",
          score: 0.9,
          reason: "这页项目记录与本轮请求相关",
          injected: true,
        }],
      },
    ], 5);

    await expect(page.getByTestId("workflow-status-pill")).toHaveCount(0);
    await page.getByTitle("打开项目档案").click();

    const workbench = page.locator("aside").last();
    await expect(workbench.getByText("项目档案", { exact: true }).first()).toBeVisible();
    const currentTask = workbench.locator("section").filter({ has: page.getByRole("heading", { name: "当前任务" }) });
    await expect(currentTask.getByText("梳理想法", { exact: true })).toBeVisible();
    await expect(currentTask.getByText("已参考 1 条档案")).toHaveCount(0);
    await expect(page.getByRole("heading", { name: "当前任务" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "交付", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "本轮参考" })).toBeVisible();
    const resources = await expandArchiveFiles(page);
    await expect(resources.getByText("文件名", { exact: true })).toBeVisible();
    await expect(resources.getByText("类型", { exact: true })).toBeVisible();
    await expect(resources.getByText("解析状态", { exact: true })).toBeVisible();
    await expect(resources.getByText("参考", { exact: true })).toBeVisible();
    await expect(workbench.getByTitle("刷新交付状态")).toBeVisible();
    const legacyProjectStatusLabel = ["项目", "状态"].join("");
    await expect(workbench.getByText(legacyProjectStatusLabel)).toHaveCount(0);
  });
});

test.describe("Turn context", () => {
  test("shows saved background and project records for the current turn", async ({ page }) => {
    const sessionId = "context-activation-session";
    const projectPath = "/Users/cabbos/project/forge";
    const selectedMemory = {
      memory_id: "memory-1",
      title: "中文优先",
      body: "用户偏好中文沟通，英文能力稍弱。",
      category: "preference" as const,
      scope: "user_profile" as const,
      score: 0.93,
      reason: "这是你固定的偏好",
      injected: true,
    };
    const selectedPage = {
      page_id: "tasks",
      title: "当前任务",
      path: "tasks.md",
      kind: "tasks" as const,
      summary: "当前正在收拢工作方式和本轮参考。",
      score: 0.91,
      reason: "这页项目记录与本轮请求相关",
      injected: true,
    };

    await setup(page);
    await page.addInitScript(({ sessionId, projectPath }) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", projectPath);
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, { sessionId, projectPath });

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await simulateStream(page, sessionId, [
      { event_type: "memory_selection", session_id: sessionId, selected: [selectedMemory] },
      { event_type: "forge_wiki_context_selected", session_id: sessionId, selected: [selectedPage] },
    ], 5);

    await page.getByTitle("打开项目档案").click();
    const activeContext = page.locator("section").filter({ has: page.getByRole("heading", { name: "本轮参考" }) });

    await expect(activeContext.getByText("已参考 2 条档案")).toBeVisible();
    await expect(activeContext.getByText("中文优先")).toBeVisible();
    await expect(activeContext.getByText("当前任务")).toBeVisible();
    await expect(activeContext.getByText("偏好", { exact: true })).toBeVisible();
    await expect(activeContext.getByText("项目记录 · tasks.md")).toBeVisible();
    await expect(activeContext.getByText("为什么参考")).toHaveCount(0);
    await expect(activeContext.getByText("本轮状态")).toHaveCount(0);
  });

  test("does not suggest saved background when user says not to remember", async ({ page }) => {
    const sessionId = "no-memory-session";
    await setup(page);
    await page.addInitScript((sessionId) => {
      window.localStorage.clear();
      window.localStorage.setItem("forge-working-dir", "/Users/cabbos/project/forge");
      // @ts-expect-error mock
      window.__mockSessionId = sessionId;
    }, sessionId);

    await page.goto("http://localhost:1420");
    await page.getByRole("button", { name: "新对话", exact: true }).click();
    await page.waitForFunction(() => {
      // @ts-expect-error Tauri listener registry installed by setup()
      return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
    });

    await page.locator("textarea").fill("不要记住这个，只是临时测试：以后默认用亮色主题。");
    await page.locator("textarea").press("Enter");
    await page.getByTitle("打开项目档案").click();
    await expandArchiveRecords(page);

    const inbox = projectArchive(page).locator("section").filter({ has: page.getByRole("heading", { name: "建议更新记录" }) });
    await expect(inbox.getByText("没有待确认的记录更新")).toBeVisible();
    await expect(inbox.getByText("以后默认用亮色主题")).not.toBeVisible();
  });
});
