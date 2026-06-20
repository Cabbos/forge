import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { deriveStartReadiness } from "./start-readiness.ts";
import type { ProviderDefinition } from "./providers.ts";
import type { ProjectCheckpointStatus, ProjectRuntimeStatus } from "./ipc/types.ts";
import type { Workspace } from "./workspaces.ts";

const workspace: Workspace = {
  id: "/Users/cabbos/project/forge",
  name: "forge",
  path: "/Users/cabbos/project/forge",
  lastOpenedAt: 1,
};

const runtimeReady: ProjectRuntimeStatus = {
  working_dir: workspace.path,
  has_package_json: true,
  package_manager: "npm",
  dev_script: "dev",
  command: "npm run dev",
  port: 1420,
  url: "http://localhost:1420",
  running: true,
  managed: true,
  pid: 123,
  can_start: false,
  can_stop: true,
  can_open: true,
  message: "Preview running",
  logs: [],
};

const checkpointReady: ProjectCheckpointStatus = {
  working_dir: workspace.path,
  is_git_repo: true,
  dirty: false,
  last_checkpoint: {
    id: "checkpoint-1",
    created_at: 1,
    head: "abc123",
    status: "ready",
    restorable: true,
    untracked_file_count: 0,
    skipped_untracked_file_count: 0,
  },
  restorable: true,
  snapshot_warning: null,
  message: "Checkpoint ready",
};

describe("deriveStartReadiness", () => {
  it("does not block no-auth provider profiles on missing API key status", () => {
    const provider: ProviderDefinition = {
      id: "local-openai",
      label: "Local OpenAI",
      shortLabel: "Local",
      keyPlaceholder: "not required",
      defaultModel: "local-model",
      models: [{ id: "local-model", name: "local-model" }],
      requiresApiKey: false,
      customModels: true,
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "local-model",
      keyStatuses: [],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
    });

    const keyRow = readiness.rows.find((row) => row.label === "模型密钥");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 0);
    assert.equal(keyRow?.tone, "ready");
    assert.equal(keyRow?.value, "Local OpenAI 不需要密钥");
    assert.equal(keyRow?.action, null);
  });

  it("warns when the selected model is outside the provider catalog", () => {
    const provider: ProviderDefinition = {
      id: "deepseek",
      label: "DeepSeek",
      shortLabel: "DeepSeek",
      keyPlaceholder: "sk-...",
      defaultModel: "deepseek-v4-flash[1m]",
      models: [{ id: "deepseek-v4-flash[1m]", name: "DeepSeek V4 Flash 1M" }],
      requiresApiKey: true,
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "gpt-4o",
      keyStatuses: [{ provider: "deepseek", set: true, preview: "sk-...1234" }],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
    });

    const modelRow = readiness.rows.find((row) => row.label === "模型");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 1);
    assert.equal(modelRow?.tone, "warning");
    assert.equal(modelRow?.value, "gpt-4o 不在 DeepSeek 模型目录");
  });
});
