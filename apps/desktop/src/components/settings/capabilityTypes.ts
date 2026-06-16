export type CapabilityTab = "skills" | "providers" | "mcp" | "hooks";

export const CAPABILITY_TABS: CapabilityTab[] = ["skills", "providers", "mcp", "hooks"];

export function tabLabel(tab: CapabilityTab) {
  switch (tab) {
    case "skills":
      return "插件";
    case "providers":
      return "模型";
    case "mcp":
      return "连接";
    case "hooks":
      return "自动化";
  }
}
