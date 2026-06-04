import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname;

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertIncludes(source, needle, label) {
  assert(source.includes(needle), `${label} missing: ${needle}`);
}

function assertNotIncludes(source, needle, label) {
  assert(!source.includes(needle), `${label} should not include: ${needle}`);
}

function selectorBlock(source, selector) {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(`${escapedSelector}\\s*\\{`).exec(source);
  const start = match?.index ?? -1;
  assert(start !== -1, `selector missing: ${selector}`);

  const open = source.indexOf("{", start);
  assert(open !== -1, `selector has no body: ${selector}`);

  let depth = 0;
  for (let i = open; i < source.length; i += 1) {
    const char = source[i];
    if (char === "{") depth += 1;
    if (char === "}") depth -= 1;
    if (depth === 0) return source.slice(open + 1, i);
  }

  throw new Error(`selector body never closes: ${selector}`);
}

const files = {
  globals: read("src/styles/globals.css"),
  sidebar: read("src/styles/sidebar.css"),
  titlebar: read("src/styles/titlebar.css"),
  messages: read("src/styles/messages.css"),
  composer: read("src/styles/composer.css"),
  process: read("src/styles/process.css"),
  messagePanel: read("src/styles/message-panel.css"),
  confirm: read("src/styles/confirm.css"),
  delivery: read("src/styles/delivery.css"),
  tauriConfig: read("src-tauri/tauri.conf.json"),
  sessionView: read("src/components/session/SessionView.tsx"),
  sidebarComponent: read("src/components/layout/Sidebar.tsx"),
  conversationLane: read("src/components/chat/ConversationLane.tsx"),
  textBlock: read("src/components/messages/TextBlock.tsx"),
  userMessage: read("src/components/messages/UserMessage.tsx"),
  composerToolbar: read("src/components/session/ComposerToolbar.tsx"),
};

assertIncludes(files.sessionView, 'data-conversation-theme="light"', "SessionView light theme scope");
assertIncludes(files.conversationLane, "data-turn-rail={getConversationTurnRail(turn)}", "conversation turn rail marker");
assertIncludes(files.textBlock, "data-state={block.isComplete ? \"complete\" : \"streaming\"}", "assistant streaming state");
assertIncludes(files.textBlock, "forge-assistant-name", "assistant visible name");
assertIncludes(files.userMessage, 'data-message-role="user"', "user message role marker");
assertIncludes(files.userMessage, 'data-message-length={isLongMessage ? "long" : "short"}', "user message length row marker");
assertIncludes(files.userMessage, 'data-long={isLongMessage ? "true" : "false"}', "user message long marker");
assertIncludes(files.composerToolbar, "forge-composer-tool--file", "composer file tool modifier");
assertIncludes(files.composerToolbar, "forge-composer-tool--command", "composer command tool modifier");
assertIncludes(files.composerToolbar, "forge-composer-tool-label", "composer file label");
assertIncludes(files.sidebarComponent, "forge-sidebar-window-drag-region", "sidebar macOS drag region");
assertIncludes(files.sidebarComponent, 'data-tauri-drag-region="true"', "sidebar macOS drag region");
assertIncludes(files.tauriConfig, '"titleBarStyle": "Overlay"', "macOS overlay titlebar");
assertIncludes(files.tauriConfig, '"hiddenTitle": true', "macOS hidden native title");
assertIncludes(files.tauriConfig, '"trafficLightPosition": { "x": 16, "y": 15 }', "macOS native traffic light placement");

assertIncludes(files.globals, '.forge-app-shell[data-design-version="v3-light-workbench"],', "app shell light theme scope");

const themeBlock = selectorBlock(files.globals, '.forge-session-operating-surface[data-conversation-theme="light"]');
assertIncludes(themeBlock, "--forge-bg-base: #F7F2E9;", "warm light base token");
assertIncludes(themeBlock, "--forge-material-raised: #FFFBF4;", "warm raised token");
assertIncludes(themeBlock, "--forge-composer-surface: #FFFBF4;", "warm composer token");
assertIncludes(themeBlock, "--forge-composer-border: #D8C9B8;", "composer border token");
assertNotIncludes(themeBlock, "#FFFFFF", "light theme token block");
assertNotIncludes(themeBlock, "#FFFDF9", "light theme token block");

const titlebarBlock = selectorBlock(files.titlebar, ".forge-titlebar");
assertIncludes(titlebarBlock, "background: var(--forge-bg-surface);", "titlebar uses light token material");
assertIncludes(titlebarBlock, "backdrop-filter: none;", "titlebar no dark glass blur");
assertNotIncludes(titlebarBlock, "rgba(27, 26, 23", "titlebar should not use hard dark material");

const sidebarBlock = selectorBlock(files.sidebar, ".forge-sidebar");
assertIncludes(sidebarBlock, "background: var(--forge-bg-depth);", "sidebar uses theme token material");
assertIncludes(sidebarBlock, "position: relative;", "sidebar anchors the native window chrome affordance");
assertIncludes(sidebarBlock, "padding: var(--forge-sidebar-chrome-safe-top, 3.625rem) 0.5rem 0.625rem;", "sidebar reserves native traffic light space");
assertNotIncludes(sidebarBlock, "rgba(34, 32, 28", "sidebar should not use hard dark material");

const sidebarDragBlock = selectorBlock(files.sidebar, ".forge-sidebar-window-drag-region");
assertIncludes(sidebarDragBlock, "left: var(--forge-sidebar-traffic-safe-left, 5rem);", "sidebar drag area avoids native traffic lights");
assertIncludes(sidebarDragBlock, "height: var(--forge-sidebar-chrome-safe-top, 3.625rem);", "sidebar drag area matches chrome safe zone");

const workspaceTriggerBlock = selectorBlock(files.sidebar, ".forge-sidebar-workspace-trigger");
assertIncludes(workspaceTriggerBlock, "background: var(--forge-bg-surface);", "workspace trigger uses light token material");
assertNotIncludes(workspaceTriggerBlock, "rgba(34, 32, 28", "workspace trigger should not use hard dark material");

const visibleSurfaceSources = [
  files.globals,
  files.sidebar,
  files.titlebar,
  files.composer,
  files.messages,
  files.process,
  files.messagePanel,
  files.confirm,
  files.delivery,
].join("\n");
assertNotIncludes(visibleSurfaceSources, "rgba(27, 26, 23", "visible light workbench surfaces");
assertNotIncludes(visibleSurfaceSources, "rgba(34, 32, 28", "visible light workbench surfaces");

const composerBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer');
assertIncludes(composerBlock, "border-radius: 14px;", "light composer radius");
assertIncludes(composerBlock, "background: var(--forge-composer-surface);", "light composer material");
assertNotIncludes(composerBlock, "border-radius: 22px;", "light composer radius");

const composerFocusBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer:focus-within');
assertIncludes(composerFocusBlock, "border-color: #D1C7BA;", "composer focused border");
assertIncludes(composerFocusBlock, "background: #FFFBF4;", "composer focused material");
assertIncludes(composerFocusBlock, "box-shadow: var(--forge-composer-shadow-focus);", "composer focused elevation");

const composerRunningBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer[data-streaming="true"],\n  .forge-session-operating-surface[data-conversation-theme="light"] .forge-composer[data-state="running"]');
assertIncludes(composerRunningBlock, "border-color: rgba(184, 138, 86, 0.26);", "composer running border");
assertIncludes(composerRunningBlock, "background: #FFFBF4;", "composer running material");

const composerPausedBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer[data-state="paused"]');
assertIncludes(composerPausedBlock, "background: #FBF7EF;", "composer paused material");

const composerTextareaBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-textarea');
assertIncludes(composerTextareaBlock, "font-size: 18px;", "composer textarea type scale");
assertIncludes(composerTextareaBlock, "line-height: 27px;", "composer textarea line height");
assertNotIncludes(composerTextareaBlock, "font-size: 22px;", "composer textarea type scale");

const composerToolBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-tool');
assertIncludes(composerToolBlock, "height: 2rem;", "composer tool compact height");
assertIncludes(composerToolBlock, "border-radius: 10px;", "composer tool controlled radius");
assertIncludes(composerToolBlock, "padding: 0 0.6875rem;", "composer file tool compact padding");
assertNotIncludes(composerToolBlock, "border-radius: 999px;", "composer tool should not be oversized pill");

const composerCommandToolBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-tool--command');
assertIncludes(composerCommandToolBlock, "width: 2rem;", "composer command compact width");
assertIncludes(composerCommandToolBlock, "border-color: transparent;", "composer command starts as ghost control");
assertIncludes(composerCommandToolBlock, "background: transparent;", "composer command starts as ghost control");
assertNotIncludes(composerCommandToolBlock, "width: 2.25rem;", "composer command should not be oversized circle");

const composerHintBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-hint');
assertIncludes(composerHintBlock, "display: inline-flex;", "composer hint aligns with accessory rail");
assertIncludes(composerHintBlock, "border-left: 1px solid #E4DED5;", "composer hint is separated quietly");
assertIncludes(composerHintBlock, "font-size: 11.5px;", "composer hint subdued type scale");

const composerSendBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-send');
assertIncludes(composerSendBlock, "width: 2.5rem;", "composer send size");
assertIncludes(composerSendBlock, "height: 2.5rem;", "composer send size");
assertIncludes(composerSendBlock, "background: #171612;", "composer send material");

const composerSendDisabledBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-send:disabled');
assertIncludes(composerSendDisabledBlock, "background: #EEE7DC;", "composer disabled send material");
assertIncludes(composerSendDisabledBlock, "box-shadow: none;", "composer disabled send elevation");

const composerSendErrorBlock = selectorBlock(files.composer, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-composer-send.text-destructive');
assertIncludes(composerSendErrorBlock, "background: #FFF0EA;", "composer stop/error send material");
assertIncludes(composerSendErrorBlock, "color: #994731;", "composer stop/error send color");

const userNoteBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .user-command-note');
assertIncludes(userNoteBlock, "border-radius: 10px;", "user note radius");
assertIncludes(userNoteBlock, "background: #EDE6DA;", "user note material");
assertIncludes(userNoteBlock, "max-width: min(500px, 74%);", "user note width");

const longUserRowBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-user-message-row[data-message-length="long"]');
assertIncludes(longUserRowBlock, "justify-content: center;", "long user message centered");

const longUserNoteBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .user-command-note[data-long="true"]');
assertIncludes(longUserNoteBlock, "width: min(720px, 100%);", "long user note width");
assertIncludes(longUserNoteBlock, "background: #F3EADC;", "long user note material");

const lightLaneBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-conversation-lane');
assertIncludes(lightLaneBlock, "--forge-assistant-avatar-size: 2rem;", "assistant rail avatar size");
assertIncludes(lightLaneBlock, "--forge-assistant-avatar-gap: 1.25rem;", "assistant rail avatar gap");
assertIncludes(lightLaneBlock, "--forge-assistant-rail: calc(var(--forge-assistant-avatar-size) + var(--forge-assistant-avatar-gap));", "assistant rail token");

const assistantRailBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-conversation-turn[data-turn-rail="assistant"] .forge-message-block[data-block-role="trace"],\n  .forge-session-operating-surface[data-conversation-theme="light"] .forge-conversation-turn[data-turn-rail="assistant"] .forge-message-block[data-block-role="artifact"]');
assertIncludes(assistantRailBlock, "box-sizing: border-box;", "assistant turn rail sizing");
assertIncludes(assistantRailBlock, "padding-left: var(--forge-assistant-rail);", "trace and artifact align to assistant content rail");

const assistantBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .assistant-paper');
assertIncludes(assistantBlock, "display: grid;", "assistant paper uses explicit rail grid");
assertIncludes(assistantBlock, "grid-template-columns: var(--forge-assistant-avatar-size) minmax(0, 1fr);", "assistant paper rail columns");
assertIncludes(assistantBlock, "column-gap: var(--forge-assistant-avatar-gap);", "assistant paper rail gap");
assertIncludes(assistantBlock, "padding: 0 1rem 0.125rem 0;", "assistant paper rhythm");
assertIncludes(assistantBlock, "line-height: 25px;", "assistant readable line height");

const assistantAvatarBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-assistant-avatar');
assertIncludes(assistantAvatarBlock, "position: static;", "assistant avatar participates in rail grid");
assertIncludes(assistantAvatarBlock, "grid-column: 1;", "assistant avatar rail column");
assertIncludes(assistantAvatarBlock, "grid-row: 2;", "assistant avatar aligns with body row");
assertIncludes(assistantAvatarBlock, "width: var(--forge-assistant-avatar-size);", "assistant avatar size");
assertIncludes(assistantAvatarBlock, "height: var(--forge-assistant-avatar-size);", "assistant avatar size");

const assistantNameBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-assistant-name');
assertIncludes(assistantNameBlock, "position: static;", "assistant name participates in rail grid");
assertIncludes(assistantNameBlock, "grid-column: 2;", "assistant name content column");
assertIncludes(assistantNameBlock, "grid-row: 1;", "assistant name author row");

const copyBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-message-copy-action');
assertIncludes(copyBlock, "backdrop-filter: none;", "light copy action no glass blur");
assertIncludes(copyBlock, "-webkit-backdrop-filter: none;", "light copy action no webkit glass blur");

const scrollButtonBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-scroll-to-bottom');
assertIncludes(scrollButtonBlock, "backdrop-filter: none;", "light scroll button no glass blur");
assertIncludes(scrollButtonBlock, "-webkit-backdrop-filter: none;", "light scroll button no webkit glass blur");

const statusRowBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-status-row,\n  .forge-session-operating-surface[data-conversation-theme="light"] .forge-status-trigger');
assertIncludes(statusRowBlock, "border: 0;", "thinking disclosure has no capsule border");
assertIncludes(statusRowBlock, "background: transparent;", "thinking disclosure has no capsule fill");
assertIncludes(statusRowBlock, "border-radius: 6px;", "thinking disclosure has modest hover radius");
assertIncludes(statusRowBlock, "padding: 0 0.25rem;", "thinking disclosure compact padding");

const messagePanelBlock = selectorBlock(files.messagePanel, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-message-panel');
assertIncludes(messagePanelBlock, "border-radius: 10px;", "message panel radius");
assertIncludes(messagePanelBlock, "background: #FFFBF4 !important;", "message panel material");

const confirmButtonBlock = selectorBlock(files.confirm, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-confirm-button');
assertIncludes(confirmButtonBlock, "border-radius: 8px;", "confirm button radius");

const deliveryItemBlock = selectorBlock(files.delivery, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-delivery-item');
assertIncludes(deliveryItemBlock, "border-radius: 8px;", "delivery item radius");
assertIncludes(deliveryItemBlock, "background: #FBF7EF;", "delivery item material");

const processSummaryBlock = selectorBlock(files.process, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-tool-activity-summary');
assertIncludes(processSummaryBlock, "border: 0;", "process summary has no capsule border");
assertIncludes(processSummaryBlock, "border-radius: 6px;", "process summary has modest hover radius");
assertIncludes(processSummaryBlock, "background: transparent;", "process summary has no capsule fill");
assertIncludes(processSummaryBlock, "padding: 0 0.25rem;", "process summary compact padding");

const routineToolRowBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .tool-machine-plate[data-state="done"][data-tone="default"]');
assertIncludes(routineToolRowBlock, "width: fit-content;", "routine tool row does not span the lane");
assertIncludes(routineToolRowBlock, "min-height: 1.75rem;", "routine tool row compact height");
assertIncludes(routineToolRowBlock, "border-color: transparent;", "routine tool row quiet border");
assertIncludes(routineToolRowBlock, "background: transparent;", "routine tool row quiet material");

const routineToolDurationBlock = selectorBlock(files.messages, '.forge-session-operating-surface[data-conversation-theme="light"] .tool-machine-plate[data-state="done"][data-tone="default"] .tool-machine-duration');
assertIncludes(routineToolDurationBlock, "display: none;", "routine tool duration hidden");

const processRunningBlock = selectorBlock(files.process, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-tool-activity-summary[data-state="running"]');
assertIncludes(processRunningBlock, "border-color: transparent;", "process running border stays quiet");
assertIncludes(processRunningBlock, "background: rgba(184, 138, 86, 0.075);", "process running material");
assertIncludes(processRunningBlock, "color: #8A6127;", "process running text");

const processErrorBlock = selectorBlock(files.process, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-tool-activity-group[data-tone="error"] .forge-tool-activity-summary,\n  .forge-session-operating-surface[data-conversation-theme="light"] .forge-tool-activity-summary[data-state="error"]');
assertIncludes(processErrorBlock, "border-color: transparent;", "process error border stays quiet");
assertIncludes(processErrorBlock, "background: rgba(153, 71, 49, 0.075);", "process error material");
assertIncludes(processErrorBlock, "color: #994731;", "process error text");

const confirmApproveBlock = selectorBlock(files.confirm, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-confirm-button[data-variant="approve"]');
assertIncludes(confirmApproveBlock, "background: #171612;", "confirm approve material");
assertIncludes(confirmApproveBlock, "color: #F7F4EE;", "confirm approve text");

const confirmCancelBlock = selectorBlock(files.confirm, '.forge-session-operating-surface[data-conversation-theme="light"] .forge-confirm-button[data-variant="cancel"]');
assertIncludes(confirmCancelBlock, "background: #FBF7EF;", "confirm cancel material");
assertIncludes(confirmCancelBlock, "color: var(--forge-text-secondary);", "confirm cancel text");

console.log("Conversation light-theme contract passed.");
