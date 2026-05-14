import type {
  WorkflowGate,
  WorkflowOverrideAction,
  WorkflowPhase,
  WorkflowRoute,
  WorkflowState,
} from "@/lib/protocol";

export type TaskModeId =
  | "ready"
  | "clarify"
  | "spec"
  | "plan"
  | "build"
  | "debug"
  | "verify"
  | "wrap";

export interface TaskModeView {
  id: TaskModeId;
  label: string;
  title: string;
  description: string;
  tone: "neutral" | "accent" | "warning" | "danger";
}

const MODE_COPY: Record<TaskModeId, TaskModeView> = {
  ready: {
    id: "ready",
    label: "准备判断",
    title: "正在判断工作方式",
    description: "描述一个小工具、修改或问题，Forge 会判断下一步。",
    tone: "neutral",
  },
  clarify: {
    id: "clarify",
    label: "梳理想法",
    title: "正在把想法整理清楚",
    description: "适合把一句话的小工具想法收拢成第一版。",
    tone: "accent",
  },
  spec: {
    id: "spec",
    label: "确认方案",
    title: "先确认方案再继续",
    description: "这个任务可能影响多个部分，建议先看方案。",
    tone: "warning",
  },
  plan: {
    id: "plan",
    label: "拆成步骤",
    title: "正在拆成可执行步骤",
    description: "Forge 会把方案变成小步任务，便于执行和验证。",
    tone: "accent",
  },
  build: {
    id: "build",
    label: "开始制作",
    title: "正在处理项目",
    description: "Forge 会把目标推进到可见、可点、可继续的第一版。",
    tone: "accent",
  },
  debug: {
    id: "debug",
    label: "排查问题",
    title: "正在定位问题",
    description: "Forge 会先收集症状，再做有依据的修复。",
    tone: "danger",
  },
  verify: {
    id: "verify",
    label: "检查结果",
    title: "正在检查结果",
    description: "Forge 会跑构建、测试或查看关键状态。",
    tone: "neutral",
  },
  wrap: {
    id: "wrap",
    label: "整理结果",
    title: "正在整理完成情况",
    description: "Forge 会说明改了什么、验证了什么、还剩什么。",
    tone: "neutral",
  },
};

export function deriveTaskModeView(workflow: WorkflowState | null): TaskModeView {
  if (!workflow) return MODE_COPY.ready;
  return MODE_COPY[deriveTaskModeId(workflow.route, workflow.phase, workflow.gate)];
}

function deriveTaskModeId(route: WorkflowRoute, phase: WorkflowPhase, gate: WorkflowGate): TaskModeId {
  if (gate === "approval_required") return "spec";
  if (route === "recovery" || phase === "debugging" || phase === "blocked") return "debug";
  if (route === "verification" || phase === "verifying") return "verify";
  if (phase === "done") return "wrap";
  if (phase === "planning") return "plan";
  if (phase === "spec" || phase === "designing") return "spec";
  if (phase === "clarifying" || route === "workflow" || route === "strict_workflow") return "clarify";
  if (phase === "executing" || route === "light") return "build";
  return "ready";
}

export function taskGateLabel(gate: WorkflowGate): string {
  if (gate === "approval_required") return "需确认";
  if (gate === "soft") return "建议";
  return "直接";
}

export function taskGateCopy(gate: WorkflowGate): string | null {
  if (gate === "approval_required") return "这个请求风险较高，建议先确认方案和步骤。";
  if (gate === "soft") return "这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。";
  return null;
}

export function workflowOverrideLabel(action: WorkflowOverrideAction): string {
  switch (action) {
    case "direct":
      return "直接回答";
    case "plan_first":
      return "先拆方案";
    case "debug":
      return "排查问题";
    case "verify":
      return "检查结果";
  }
}

export function modeAwarePlaceholder(workflow: WorkflowState | null, isRunning: boolean): string {
  if (!isRunning) return "这个会话已停止，可以继续后再发送";
  const mode = deriveTaskModeView(workflow).id;
  switch (mode) {
    case "clarify":
      return "描述小工具目标、核心动作、输入和输出。";
    case "spec":
      return "看完方案后，可以说“开始做”或指出要改哪里。";
    case "plan":
      return "可以补充约束，或说“按这个计划执行”。";
    case "build":
      return "继续描述修改，Forge 会推进第一版。";
    case "debug":
      return "粘贴报错、失败现象或复现步骤。";
    case "verify":
      return "说要检查什么，或让 Forge 跑构建/测试。";
    case "wrap":
      return "可以继续追问结果，或指定下一步。";
    case "ready":
      return "描述你想做的小工具、要修改的地方，或遇到的问题。";
  }
}
