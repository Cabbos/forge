import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  setup,
  holdSendInput,
  expectHeldSendInput,
  getLastSendInputArgs,
  expectLastSendInputArgs,
  expectNoSendInput,
  releaseHeldSendInput,
  projectArchive,
  openProjectArchive,
  expandArchiveRecords,
  expandArchiveFiles,
} from "./fixtures/app";
import { simulateStream, fullConversation } from "./mock-ipc";
import type { WorkflowState } from "../src/lib/protocol";

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
    const summaryStrip = readFileSync(resolve(process.cwd(), "src/components/layout/archive/ArchiveSummaryStrip.tsx"), "utf8");
    const archiveStyles = readFileSync(resolve(process.cwd(), "src/styles/archive.css"), "utf8");

    expect(summaryStrip).toContain("project-archive-summary-strip");
    expect(summaryStrip).toContain("ArchiveSummaryStrip");
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
    const emptyWorkbench = readFileSync(resolve(process.cwd(), "src/styles/empty-workbench.css"), "utf8");

    expect(appShell).not.toContain("@/lib/forgeMotion");
    expect(appShell).not.toContain("useGSAP");
    expect(appShell).not.toContain("emptyShellRef");
    expect(appShell).toContain("data-forge-motion=\"empty-entry\"");
    expect(appShell).toContain("data-forge-motion=\"empty-composer\"");
    expect(emptyWorkbench).toContain("[data-forge-motion=\"empty-entry\"]");
    expect(emptyWorkbench).toContain("will-change: transform, opacity");
    expect(globals).not.toContain(".forge-empty-entry-card::before");
    expect(emptyWorkbench).toContain(".forge-empty-entry-card[data-active=\"true\"] .forge-empty-entry-icon");
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
