export type CapabilityTab = "skills" | "mcp" | "hooks";

export const CAPABILITY_TABS: CapabilityTab[] = ["skills", "mcp", "hooks"];

export function tabLabel(tab: CapabilityTab) {
  switch (tab) {
    case "skills":
      return "插件";
    case "mcp":
      return "连接";
    case "hooks":
      return "自动化";
  }
}
