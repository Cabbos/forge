import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";
import ts from "typescript";

async function importContextUsageView() {
  const source = await readFile(new URL("./contextUsageView.ts", import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2020,
    },
    fileName: "contextUsageView.ts",
  });

  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
}

const { buildComposerContextUsageView } = await importContextUsageView();

function usage(overrides = {}) {
  return {
    usedTokens: 142_000,
    contextWindowTokens: 1_000_000,
    percentUsed: 14,
    source: "provider_usage",
    lastUpdatedAt: 1,
    ...overrides,
  };
}

describe("buildComposerContextUsageView", () => {
  it("surfaces context remaining in the compact context label and auto-compact distance in the title", () => {
    const view = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: false,
      usage: usage(),
    });

    assert.equal(view.label, "142K / 1M · 余 858K");
    assert.match(view.title, /剩余上下文约 858K tokens/);
    assert.match(view.title, /自动压缩阈值 967K/);
    assert.match(view.title, /距离自动压缩还有约 825K tokens/);
    assert.equal(view.compactButton.disabled, false);
  });

  it("does not round tiny usage against a 1M window down to the auto-compact threshold", () => {
    const view = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: false,
      usage: usage({
        usedTokens: 411,
        percentUsed: 0,
      }),
    });

    assert.equal(view.label, "411 / 1M · 余 999.5K");
    assert.match(view.title, /剩余上下文约 999.5K tokens/);
    assert.match(view.title, /自动压缩阈值 967K/);
  });

  it("treats zero provider usage as known context usage", () => {
    const view = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: false,
      usage: usage({
        usedTokens: 0,
        contextWindowTokens: 1_000_000,
        percentUsed: 0,
      }),
    });

    assert.equal(view.label, "0 / 1M · 余 1M");
    assert.match(view.title, /上下文 0 \/ 1M/);
    assert.match(view.title, /剩余上下文约 1M tokens/);
    assert.match(view.title, /自动压缩阈值 967K/);
    assert.match(view.title, /距离自动压缩还有约 967K tokens/);
  });

  it("leaves unknown provider usage out of the compact context label", () => {
    const view = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: false,
      usage: usage({
        usedTokens: null,
        contextWindowTokens: 1_000_000,
        percentUsed: null,
      }),
    });

    assert.equal(view.label, "");
    assert.equal(view.title, "压缩当前上下文");
    assert.equal(view.compactButton.title, "压缩当前上下文");
  });

  it("clamps exhausted context remaining to zero", () => {
    const view = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: false,
      usage: usage({
        usedTokens: 1_005_000,
        percentUsed: 100,
      }),
    });

    assert.equal(view.label, "1M / 1M · 余 0");
    assert.match(view.title, /剩余上下文约 0 tokens/);
    assert.match(view.title, /已达到自动压缩阈值/);
  });

  it("explains the disabled compact action while a turn is streaming", () => {
    const view = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: true,
      usage: usage(),
    });

    assert.equal(view.label, "生成中 · 142K / 1M");
    assert.equal(view.compactButton.disabled, true);
    assert.equal(view.compactButton.ariaLabel, "生成中，暂不能压缩上下文");
    assert.match(view.compactButton.title, /生成中，完成后可手动压缩/);
    assert.match(view.compactButton.title, /距离自动压缩还有约 825K tokens/);
  });

  it("surfaces compacting and last compact state without backend schema changes", () => {
    const compactedUsage = usage({
      usedTokens: 32_000,
      percentUsed: 3,
      source: "local_estimate",
      lastCompactedAt: 2,
      compactedFromTokens: 142_000,
      compactedToTokens: 32_000,
    });

    const idleView = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: false,
      isStreaming: false,
      usage: compactedUsage,
    });
    assert.equal(idleView.label, "32K / 1M · 已压缩");
    assert.match(idleView.title, /上次压缩 142K -> 32K/);

    const compactingView = buildComposerContextUsageView({
      fallbackContextWindowTokens: null,
      isCompacting: true,
      isStreaming: false,
      usage: compactedUsage,
    });
    assert.equal(compactingView.label, "压缩中 · 32K / 1M");
    assert.equal(compactingView.compactButton.disabled, true);
    assert.equal(compactingView.compactButton.state, "compacting");
  });
});
