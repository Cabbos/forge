export interface FirstLoopDraft {
  sessionId: string;
  goal: string;
  scope: string;
  nextStep: string;
  sourceText: string;
  createdAt: number;
}

export const FIRST_LOOP_STANDARD = "可见、可点、可继续";

const FIRST_LOOP_SIGNALS = ["小工具", "番茄钟", "记账", "文案"];
const IDEA_SHAPING_SIGNALS = [
  "不知道怎么说",
  "不知道怎么概括",
  "帮我想想",
  "大概是",
  "类似",
  "我想做个",
  "能不能做一个",
  "最好能",
];
const DIRECT_MAKE_SIGNALS = ["直接做", "直接开始", "先做", "开始做", "做到第一版"];

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
  if (isIdeaShapingRequest(text)) {
    return buildIdeaShapingPrompt(text);
  }
  if (!isFirstLoopRequest(text)) return text;

  return [
    text,
    "",
    "Forge 第一闭环提示：请优先推进到一个可预览的第一版。",
    `第一版标准：${FIRST_LOOP_STANDARD}。`,
    "不需要完整、漂亮、可发布；需要有真实界面、一个核心动作、清楚说明当前范围和下一步。",
  ].join("\n");
}

function buildIdeaShapingPrompt(text: string): string {
  return [
    text,
    "",
    "Forge 需求梳理提示：这个请求还不适合立刻改代码。请先帮用户把想法整理成一个可执行的第一版。",
    "请用中文，保持简短：",
    "1. 用一句话复述你理解的作品或小工具。",
    "2. 提出一个小的第一版，只包含 2-3 个核心动作。",
    "3. 明确列出先不做的内容。",
    "4. 最后只问一个轻确认问题，例如“我先按这个第一版开始，可以吗？”",
    "不要执行命令，不要修改文件，不要要求用户先理解项目、工作区或代码仓库。",
    "如果用户说“可以 / ok / 就这样 / 直接做”，再进入制作。",
  ].join("\n");
}

function isFirstLoopRequest(text: string): boolean {
  const normalized = collapseWhitespace(text);
  if (!normalized) return false;

  if (FIRST_LOOP_SIGNALS.some((signal) => normalized.includes(signal))) return true;
  return normalized.includes("我想做") && normalized.includes("工具");
}

function isIdeaShapingRequest(text: string): boolean {
  const normalized = collapseWhitespace(text);
  if (!normalized) return false;
  if (DIRECT_MAKE_SIGNALS.some((signal) => normalized.includes(signal))) return false;
  if (IDEA_SHAPING_SIGNALS.some((signal) => normalized.includes(signal))) return true;

  const featureCount = ["提醒", "导出", "登录", "统计", "同步", "分享", "表格"].filter((signal) => normalized.includes(signal)).length;
  return featureCount >= 2 && /我想做|做个|做一个|能不能/.test(normalized);
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
