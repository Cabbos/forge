import type {
  ForgeWikiUpdateProposal,
  MemoryCategory,
  MemoryStatus,
} from "@/lib/protocol";

export function proposalStatusLabel(status: ForgeWikiUpdateProposal["status"]) {
  if (status === "accepted") return "已写入项目记录";
  if (status === "discarded") return "已丢弃";
  return "建议写入项目记录";
}

export function proposalStatusMeta(status: ForgeWikiUpdateProposal["status"]) {
  if (status === "accepted") return "已写入";
  if (status === "discarded") return "不再处理";
  return "待确认";
}

export function categoryLabel(category: MemoryCategory) {
  switch (category) {
    case "preference":
      return "偏好";
    case "project_fact":
      return "项目信息";
    case "decision":
      return "决策";
    case "task_state":
      return "任务状态";
  }
}

export function statusLabel(status: MemoryStatus) {
  switch (status) {
    case "candidate":
      return "候选";
    case "accepted":
      return "已确认";
    case "pinned":
      return "已置顶";
    case "forgotten":
      return "已忘记";
    case "archived":
      return "已归档";
  }
}
