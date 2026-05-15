import type { BlockState } from "@/lib/protocol";
import type { FirstLoopDraft } from "@/lib/first-loop";

export type FirstLoopPhaseId = "understand" | "prepare" | "make" | "preview" | "review";

export interface FirstLoopPhase {
  id: FirstLoopPhaseId;
  label: string;
  state: "done" | "active" | "upcoming";
}

const PHASES: Array<{ id: FirstLoopPhaseId; label: string }> = [
  { id: "understand", label: "理解目标" },
  { id: "prepare", label: "准备修改" },
  { id: "make", label: "正在制作" },
  { id: "preview", label: "可以预览" },
  { id: "review", label: "等你验收" },
];

export function deriveFirstLoopProgress(blocks: BlockState[], draft: FirstLoopDraft | null): FirstLoopPhase[] {
  const currentId = currentPhase(blocks, draft);
  const currentIndex = PHASES.findIndex((phase) => phase.id === currentId);

  return PHASES.map((phase, index) => ({
    ...phase,
    state: index < currentIndex ? "done" : index === currentIndex ? "active" : "upcoming",
  }));
}

function currentPhase(blocks: BlockState[], draft: FirstLoopDraft | null): FirstLoopPhaseId {
  if (blocks.some((block) => block.event_type === "delivery_summary")) return "review";

  const latestConfirm = [...blocks].reverse().find((block) => block.event_type === "confirm_ask");
  if (latestConfirm && latestConfirm.metadata.confirmed !== true) return "prepare";

  if (blocks.some((block) => block.event_type === "shell" || block.event_type === "tool_call" || block.event_type === "diff_view")) {
    return "make";
  }

  if (draft || blocks.some((block) => block.event_type === "user_message")) return "make";
  return "understand";
}
