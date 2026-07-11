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

const PROVIDER_EVIDENCE_RECORDED_AT_MS = 1_717_891_200_000;
const PROVIDER_EVIDENCE_FRESH_NOW_MS = PROVIDER_EVIDENCE_RECORDED_AT_MS + 86_400_000;
const PROVIDER_EVIDENCE_STALE_NOW_MS = PROVIDER_EVIDENCE_RECORDED_AT_MS + 16 * 86_400_000;

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
    const evidenceRow = readiness.rows.find((row) => row.label === "Provider 证据");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 1);
    assert.equal(keyRow?.tone, "ready");
    assert.equal(keyRow?.value, "Local OpenAI 不需要密钥");
    assert.equal(keyRow?.action, null);
    assert.equal(evidenceRow?.tone, "warning");
    assert.equal(evidenceRow?.value, "需要手动检测：尚未手动检测 · 目录未验证");
    assert.equal(evidenceRow?.action, null);
  });

  it("warns and links to Settings when catalog evidence is stale without probe evidence", () => {
    const provider: ProviderDefinition = {
      id: "deepseek",
      label: "DeepSeek",
      shortLabel: "DeepSeek",
      keyPlaceholder: "sk-...",
      defaultModel: "deepseek-v4-flash[1m]",
      models: [{ id: "deepseek-v4-flash[1m]", name: "DeepSeek V4 Flash 1M" }],
      requiresApiKey: true,
      modelCatalogSource: "live_endpoint",
      modelCatalogRecordedAtMs: PROVIDER_EVIDENCE_RECORDED_AT_MS,
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "deepseek-v4-flash[1m]",
      keyStatuses: [{ provider: "deepseek", configured: true, source: "system_store", status: "available", error: null }],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
      nowMs: PROVIDER_EVIDENCE_STALE_NOW_MS,
    });

    const evidenceRow = readiness.rows.find((row) => row.label === "Provider 证据");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 1);
    assert.equal(evidenceRow?.tone, "warning");
    assert.equal(
      evidenceRow?.value,
      "需要手动检测：尚未手动检测 · 目录 Live /models · 目录刷新 2024-06-09 · 目录刷新已超过 14 天",
    );
    assert.equal(evidenceRow?.action, "open_settings");
    assert.equal(evidenceRow?.actionLabel, "打开设置");
  });

  it("surfaces strong provider evidence when manual probe and live catalog passed", () => {
    const provider: ProviderDefinition = {
      id: "nvidia",
      label: "NVIDIA NIM",
      shortLabel: "NVIDIA",
      keyPlaceholder: "sk-...",
      defaultModel: "nvidia/llama-3.1-nemotron",
      models: [{ id: "nvidia/llama-3.1-nemotron", name: "NVIDIA Nemotron" }],
      requiresApiKey: true,
      modelCatalogSource: "live_endpoint",
      modelCatalogRecordedAtMs: PROVIDER_EVIDENCE_RECORDED_AT_MS,
      probeEvidence: {
        source: "manual_probe",
        status: "passed",
        recorded_at_ms: PROVIDER_EVIDENCE_RECORDED_AT_MS,
        model: "nvidia/llama-3.1-nemotron",
        base_url: "https://integrate.api.nvidia.com/v1",
        checks: [
          { id: "streaming_accepted", label: "Streaming accepted", status: "passed" },
          { id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed" },
        ],
      },
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "nvidia/llama-3.1-nemotron",
      keyStatuses: [{ provider: "nvidia", configured: true, source: "system_store", status: "available", error: null }],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
      nowMs: PROVIDER_EVIDENCE_FRESH_NOW_MS,
    });

    const evidenceRow = readiness.rows.find((row) => row.label === "Provider 证据");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 0);
    assert.equal(evidenceRow?.tone, "ready");
    assert.equal(
      evidenceRow?.value,
      "证据较强：手动检测通过 · 检测 2024-06-09 · 目录 Live /models · 目录刷新 2024-06-09",
    );
    assert.equal(evidenceRow?.action, null);
  });

  it("warns and links to Settings when passed provider evidence is stale", () => {
    const provider: ProviderDefinition = {
      id: "nvidia",
      label: "NVIDIA NIM",
      shortLabel: "NVIDIA",
      keyPlaceholder: "sk-...",
      defaultModel: "nvidia/llama-3.1-nemotron",
      models: [{ id: "nvidia/llama-3.1-nemotron", name: "NVIDIA Nemotron" }],
      requiresApiKey: true,
      modelCatalogSource: "live_endpoint",
      modelCatalogRecordedAtMs: PROVIDER_EVIDENCE_RECORDED_AT_MS,
      probeEvidence: {
        source: "manual_probe",
        status: "passed",
        recorded_at_ms: PROVIDER_EVIDENCE_RECORDED_AT_MS,
        model: "nvidia/llama-3.1-nemotron",
        base_url: "https://integrate.api.nvidia.com/v1",
        checks: [{ id: "tool_schema_accepted", label: "Tool schema accepted", status: "passed" }],
      },
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "nvidia/llama-3.1-nemotron",
      keyStatuses: [{ provider: "nvidia", configured: true, source: "system_store", status: "available", error: null }],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
      nowMs: PROVIDER_EVIDENCE_STALE_NOW_MS,
    });

    const evidenceRow = readiness.rows.find((row) => row.label === "Provider 证据");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 1);
    assert.equal(evidenceRow?.tone, "warning");
    assert.equal(
      evidenceRow?.value,
      "证据需复核：手动检测通过 · 检测 2024-06-09 · 检测已超过 14 天 · 目录 Live /models · 目录刷新 2024-06-09 · 目录刷新已超过 14 天",
    );
    assert.equal(evidenceRow?.action, "open_settings");
    assert.equal(evidenceRow?.actionLabel, "打开设置");
  });

  it("blocks start readiness when the cached manual provider probe failed", () => {
    const provider: ProviderDefinition = {
      id: "openai",
      label: "OpenAI",
      shortLabel: "GPT",
      keyPlaceholder: "sk-...",
      defaultModel: "gpt-4o",
      models: [{ id: "gpt-4o", name: "GPT-4o" }],
      requiresApiKey: true,
      probeEvidence: {
        source: "manual_probe",
        status: "failed",
        recorded_at_ms: null,
        model: "gpt-4o",
        base_url: "https://api.openai.com/v1",
        checks: [
          { id: "streaming_accepted", label: "Streaming accepted", status: "failed" },
        ],
      },
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "gpt-4o",
      keyStatuses: [{ provider: "openai", configured: true, source: "system_store", status: "available", error: null }],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
    });

    const evidenceRow = readiness.rows.find((row) => row.label === "Provider 证据");
    assert.equal(readiness.title, "Provider 检测失败");
    assert.equal(readiness.subtitle, "打开设置重新检测 provider。");
    assert.equal(readiness.issueCount, 1);
    assert.equal(evidenceRow?.tone, "blocked");
    assert.equal(evidenceRow?.value, "检测失败：手动检测失败 · 检测时间未知 · 目录未验证");
    assert.equal(evidenceRow?.action, "open_settings");
    assert.equal(evidenceRow?.actionLabel, "打开设置");
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
      modelCatalogSource: "live_endpoint",
      modelCatalogRecordedAtMs: PROVIDER_EVIDENCE_RECORDED_AT_MS,
      probeEvidence: {
        source: "manual_probe",
        status: "passed",
        recorded_at_ms: PROVIDER_EVIDENCE_RECORDED_AT_MS,
        model: "deepseek-v4-flash[1m]",
        base_url: "https://api.deepseek.com/anthropic",
        checks: [
          { id: "streaming_accepted", label: "Streaming accepted", status: "passed" },
        ],
      },
    };

    const readiness = deriveStartReadiness({
      workspace,
      providerId: provider.id,
      providerLabel: provider.label,
      provider,
      model: "gpt-4o",
      keyStatuses: [{ provider: "deepseek", configured: true, source: "system_store", status: "available", error: null }],
      runtime: runtimeReady,
      checkpoint: checkpointReady,
      nowMs: PROVIDER_EVIDENCE_FRESH_NOW_MS,
    });

    const modelRow = readiness.rows.find((row) => row.label === "模型");
    assert.equal(readiness.title, "准备开始");
    assert.equal(readiness.issueCount, 1);
    assert.equal(modelRow?.tone, "warning");
    assert.equal(modelRow?.value, "gpt-4o 不在 DeepSeek 模型目录");
  });
});
