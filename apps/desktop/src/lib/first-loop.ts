export interface FirstLoopDraft {
  sessionId: string;
  goal: string;
  scope: string;
  nextStep: string;
  sourceText: string;
  createdAt: number;
}

export const FIRST_LOOP_STANDARD = "可见、可点、可继续";

export const FIRST_LOOP_QUICK_PROMPTS = [
  "我想做一个番茄钟小工具，可以开始、暂停、重置。",
  "我想做一个记账小工具，先能记录一笔收入或支出。",
  "我想做一个文案小工具，输入主题后生成一版短文案。",
];

const FIRST_LOOP_SIGNALS = ["小工具", "番茄钟", "记账", "文案"];

export function deriveFirstLoopDraft(sessionId: string, text: string): FirstLoopDraft | null {
  const sourceText = collapseWhitespace(text);
  if (!sourceText || !isFirstLoopRequest(sourceText)) return null;

  const goal = deriveGoal(sourceText);
  const actionScope = deriveActionScope(sourceText);

  return {
    sessionId,
    goal,
    scope: actionScope
      ? `先做一个真实界面，并让 ${actionScope} 成为第一版核心动作。`
      : "先做一个真实界面，并让一个核心动作可以完成。",
    nextStep: "生成可预览第一版，预览后继续调整样式、数据或更多流程。",
    sourceText,
    createdAt: Date.now(),
  };
}

export function buildFirstLoopAgentPrompt(text: string): string {
  if (!isFirstLoopRequest(text)) return text;

  return [
    text,
    "",
    "Forge 第一闭环提示：请优先推进到一个可预览的第一版。",
    `第一版标准：${FIRST_LOOP_STANDARD}。`,
    "不需要完整、漂亮、可发布；需要有真实界面、一个核心动作、清楚说明当前范围和下一步。",
  ].join("\n");
}

function isFirstLoopRequest(text: string): boolean {
  const normalized = collapseWhitespace(text);
  if (!normalized) return false;

  if (FIRST_LOOP_SIGNALS.some((signal) => normalized.includes(signal))) return true;
  return normalized.includes("我想做") && normalized.includes("工具");
}

function deriveGoal(text: string): string {
  const toolMatch = text.match(/(?:我想做|做一个|做个|制作一个|制作个)([^。！？\n]+?工具)/);
  if (toolMatch?.[1]) return toolMatch[1].trim();

  const sentence = text.split(/[。！？\n]/)[0]?.trim();
  return sentence || "小工具第一版";
}

function deriveActionScope(text: string): string {
  const canMatch = text.match(/可以([^。！？\n]+)/);
  if (canMatch?.[1]) return stripLeadingPunctuation(canMatch[1]);

  const firstMatch = text.match(/先能([^。！？\n]+)/);
  if (firstMatch?.[1]) return stripLeadingPunctuation(firstMatch[1]);

  const inputMatch = text.match(/输入([^。！？\n]+)/);
  if (inputMatch?.[0]) return stripLeadingPunctuation(inputMatch[0]);

  return "";
}

function stripLeadingPunctuation(text: string): string {
  return text.replace(/^[，,：:\s]+/, "").trim();
}

function collapseWhitespace(text: string): string {
  return text.split(/\s+/).filter(Boolean).join(" ").trim();
}

