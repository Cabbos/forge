import type { ReactNode } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { AlertCircle, Blocks, Clock3, Search, Settings, SquarePen } from "lucide-react";
import type { Workspace } from "@/lib/workspaces";
import { cn } from "@/lib/utils";

export type SidebarPanel = "plugins" | "automation";
export type SidebarNotice = { message: string; action?: "settings" };

interface SidebarPrimaryNavProps {
  activeWorkspace: Workspace | null;
  onNewSession: () => void;
  onOpenSearch: () => void;
}

interface SidebarUtilityNavProps {
  activePanel: SidebarPanel | null;
  onOpenPanel: (panel: SidebarPanel) => void;
  onOpenSettings: () => void;
}

export function SidebarPrimaryNav({
  activeWorkspace,
  onNewSession,
  onOpenSearch,
}: SidebarPrimaryNavProps) {
  return (
    <nav data-testid="sidebar-primary-nav" data-forge-motion="sidebar-entry" className="forge-sidebar-primary-nav">
      <SidebarAction icon={<SquarePen className="size-4" />} label="新对话" disabled={!activeWorkspace} onClick={onNewSession} />
      <SidebarAction icon={<Search className="size-4" />} label="搜索" onClick={onOpenSearch} />
    </nav>
  );
}

export function SidebarNoticeBanner({ notice }: { notice: SidebarNotice }) {
  return (
    <div
      role="status"
      className="mb-3 flex items-start gap-2 rounded-md border border-primary/20 bg-primary/5 px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground"
    >
      <AlertCircle className="mt-0.5 size-3.5 shrink-0 text-primary" />
      <span className="min-w-0 flex-1">{notice.message}</span>
      {notice.action === "settings" && (
        <ButtonPrimitive
          type="button"
          onClick={() => window.dispatchEvent(new Event("forge:open-settings"))}
          className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium text-primary transition-colors hover:bg-primary/10"
        >
          打开设置
        </ButtonPrimitive>
      )}
    </div>
  );
}

export function SidebarUtilityNav({
  activePanel,
  onOpenPanel,
  onOpenSettings,
}: SidebarUtilityNavProps) {
  return (
    <nav
      data-testid="sidebar-utility-nav"
      data-forge-motion="sidebar-entry"
      className="forge-sidebar-utility-nav"
    >
      <SidebarIconAction
        icon={<Blocks className="size-4" />}
        label="插件"
        active={activePanel === "plugins"}
        onClick={() => onOpenPanel("plugins")}
      />
      <SidebarIconAction
        icon={<Clock3 className="size-4" />}
        label="自动化"
        active={activePanel === "automation"}
        onClick={() => onOpenPanel("automation")}
      />
      <SidebarIconAction
        icon={<Settings className="size-4" />}
        label="设置"
        onClick={onOpenSettings}
      />
    </nav>
  );
}

export function createSessionNotice(error: unknown): SidebarNotice {
  const message = error instanceof Error ? error.message : String(error);
  if (/api key|密钥/i.test(message)) {
    return {
      message: "模型服务还没有可用密钥。添加密钥后就可以开始新对话。",
      action: "settings",
    };
  }
  if (message.includes("请选择具体项目文件夹")) {
    return { message: "请选择具体项目文件夹，不要直接使用用户主目录。" };
  }
  if (message.includes("无法打开项目文件夹") || message.includes("不是项目文件夹")) {
    return { message: "这个项目文件夹打不开。请重新选择一个具体项目文件夹。" };
  }
  return { message: "新对话没有创建成功。请检查设置后重试。" };
}

function SidebarAction({
  icon,
  label,
  active,
  disabled,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <ButtonPrimitive
      type="button"
      data-testid="sidebar-primary-action"
      onClick={onClick}
      disabled={disabled}
      data-active={active ? "true" : "false"}
      className="forge-sidebar-action"
    >
      <span className="forge-sidebar-action-icon">
        {icon}
      </span>
      <span className="truncate">{label}</span>
    </ButtonPrimitive>
  );
}

function SidebarIconAction({
  icon,
  label,
  active,
  onClick,
}: {
  icon: ReactNode;
  label: string;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <ButtonPrimitive
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn("forge-sidebar-utility-button", active && "forge-sidebar-utility-button-active")}
    >
      {icon}
    </ButtonPrimitive>
  );
}
