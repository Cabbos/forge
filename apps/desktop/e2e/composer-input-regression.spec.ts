import { expect, test, type Page } from "@playwright/test";
import type { StreamEvent } from "../src/lib/protocol";

const WORKSPACE = "/Users/cabbos/project/forge-test-app";
const USER_MESSAGE = "检查这个页面按钮为什么没有反馈";

async function setupComposerRegression(page: Page, sessionId: string) {
  await page.addInitScript(({ sessionId, workspace }) => {
    let callbackId = 0;
    const callbacks = new Map<number, (data: unknown) => void>();

    // @ts-expect-error test shim
    window.__tauriListeners = {};
    const sentInputs: Record<string, unknown>[] = [];
    const sendInputResolvers: Array<() => void> = [];

    // @ts-expect-error test shim
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "plugin:event|listen") {
          const event = args.event as string;
          // @ts-expect-error test shim
          window.__tauriListeners[event] ??= [];
          const callback = callbacks.get(args.handler as number);
          if (callback) {
            // @ts-expect-error test shim
            window.__tauriListeners[event].push(callback);
          }
          return args.handler;
        }
        if (cmd === "plugin:event|unlisten") {
          callbacks.delete(args.eventId as number);
          return undefined;
        }
        // @ts-expect-error test shim
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

    // @ts-expect-error test shim
    window.__TAURI__ = {
      event: {
        listen: (event: string, fn: (data: unknown) => void) => {
          // @ts-expect-error test shim
          window.__tauriListeners[event] ??= [];
          // @ts-expect-error test shim
          window.__tauriListeners[event].push(fn);
          return () => {};
        },
      },
    };

    // @ts-expect-error test shim
    window.__composerRegression = {
      sentInputs,
      releaseNextSendInput: () => {
        const resolve = sendInputResolvers.shift();
        resolve?.();
      },
    };

    const workspaceRecord = {
      id: workspace,
      name: "forge-test-app",
      path: workspace,
      lastOpenedAt: Date.now(),
    };

    // @ts-expect-error test shim
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
      switch (cmd) {
        case "load_app_metadata":
          return {
            workspaces: [workspaceRecord],
            activeWorkspaceId: workspace,
            activeSessionId: null,
            selectedProvider: "deepseek",
            selectedModel: "deepseek-v4-flash[1m]",
          };
        case "save_app_metadata":
          return undefined;
        case "list_sessions":
          return [];
        case "create_session":
          return {
            session_id: sessionId,
            provider: "deepseek",
            model: "deepseek-v4-flash[1m]",
            missing_api_key: false,
          };
        case "load_session_transcript":
          return [];
        case "create_project_checkpoint":
        case "list_mcp_context_sources":
          return cmd === "list_mcp_context_sources" ? { resources: [], prompts: [] } : {};
        case "send_input":
          sentInputs.push(args);
          await new Promise<void>((resolve) => {
            sendInputResolvers.push(resolve);
          });
          return undefined;
        case "get_project_runtime_status":
          return {
            working_dir: workspace,
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
            working_dir: workspace,
            is_git_repo: true,
            dirty: false,
            last_checkpoint: null,
            message: "No checkpoint yet",
          };
        case "list_memories":
        case "list_capabilities":
          return [];
        case "get_forge_wiki_state":
          return {
            project_path: workspace,
            exists: false,
            wiki_dir: `${workspace}/.forge/wiki`,
            pages: [],
            message: "还没有项目记录",
          };
        case "list_forge_wiki_pages":
        case "select_context_memories":
        case "select_forge_wiki_context":
          return [];
        case "get_workflow_state":
          return null;
        default:
          return undefined;
      }
    };
  }, { sessionId, workspace: WORKSPACE });
}

async function openConversation(page: Page) {
  await page.goto("/");
  await page.getByRole("button", { name: "新对话", exact: true }).click();
  await page.waitForFunction(() => {
    // @ts-expect-error test shim
    return (window.__tauriListeners?.["session-output"]?.length ?? 0) > 0;
  });
}

async function emitSessionOutput(page: Page, event: StreamEvent) {
  await page.evaluate((event) => {
    // @ts-expect-error test shim
    const listeners = window.__tauriListeners?.["session-output"] ?? [];
    for (const listener of listeners) {
      listener({ payload: event });
    }
  }, event);
}

async function submitComposerMessage(page: Page, message = USER_MESSAGE) {
  const composer = page.getByTestId("composer-lane");
  const textbox = composer.locator("textarea");
  await textbox.fill(message);
  await textbox.press("Enter");
  return textbox;
}

async function waitForSentInput(page: Page, expected: { sessionId: string; textIncludes: string }) {
  await expect.poll(async () => getSentInputs(page)).toHaveLength(1);
  const [sentInput] = await getSentInputs(page);
  expect(sentInput).toMatchObject({ sessionId: expected.sessionId });
  expect(String(sentInput.text)).toContain(expected.textIncludes);
  return sentInput;
}

async function getSentInputs(page: Page): Promise<Record<string, unknown>[]> {
  return page.evaluate(() => {
    // @ts-expect-error test shim
    return window.__composerRegression?.sentInputs ?? [];
  });
}

async function releaseNextSendInput(page: Page) {
  await page.evaluate(() => {
    // @ts-expect-error test shim
    window.__composerRegression?.releaseNextSendInput();
  });
}

function agentTurnEvent(
  sessionId: string,
  status: "calling_model" | "completed",
  stepLabel: string,
): StreamEvent {
  return {
    event_type: "agent_turn_updated",
    session_id: sessionId,
    state: {
      session_id: sessionId,
      status,
      step_label: stepLabel,
      workspace_path: WORKSPACE,
      compact_count: 0,
      verification_status: "not_needed",
    },
  };
}

test("composer clears submitted draft immediately while the agent turn is still running", async ({ page }) => {
  const sessionId = crypto.randomUUID();
  await setupComposerRegression(page, sessionId);

  await openConversation(page);
  const textbox = await submitComposerMessage(page);

  await waitForSentInput(page, { sessionId, textIncludes: USER_MESSAGE });
  await expect(textbox).toHaveValue("");
  await releaseNextSendInput(page);
});

test("composer keeps the stop action stable until the agent turn completes", async ({ page }) => {
  const sessionId = crypto.randomUUID();
  await setupComposerRegression(page, sessionId);

  await openConversation(page);
  await submitComposerMessage(page);
  await waitForSentInput(page, { sessionId, textIncludes: USER_MESSAGE });

  await emitSessionOutput(page, agentTurnEvent(sessionId, "calling_model", "正在请求模型"));
  await emitSessionOutput(page, {
    event_type: "session_status",
    session_id: sessionId,
    status: "working",
  });
  const textBlockId = crypto.randomUUID();
  await emitSessionOutput(page, {
    event_type: "text_start",
    session_id: sessionId,
    block_id: textBlockId,
  });
  await emitSessionOutput(page, {
    event_type: "text_end",
    session_id: sessionId,
    block_id: textBlockId,
  });
  await expect(page.getByTestId("composer-stop")).toBeVisible();

  await emitSessionOutput(page, {
    event_type: "session_status",
    session_id: sessionId,
    status: "idle",
  });
  await expect(page.getByTestId("composer-stop")).toBeVisible();
  await expect(page.getByTestId("composer-send")).toHaveCount(0);

  await emitSessionOutput(page, agentTurnEvent(sessionId, "completed", "已完成"));
  await expect(page.getByTestId("composer-send")).toBeVisible();
  await releaseNextSendInput(page);
});
