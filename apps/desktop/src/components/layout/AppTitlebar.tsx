import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { FolderOpen, PanelRightOpen, Search } from "lucide-react";
import type { SessionState } from "@/lib/protocol";
import { getProjectDisplay, getSessionStatus, getSessionTitle } from "@/lib/session-display";

interface AppTitlebarProps {
  session: SessionState | null;
  project: ReturnType<typeof getProjectDisplay>;
  onOpenWorkPanel: () => void;
  onOpenSearch: () => void;
}

export function AppTitlebar({
  session,
  project,
  onOpenWorkPanel,
  onOpenSearch,
}: AppTitlebarProps) {
  const hasPendingOutput = session?.blocks.some((block) => block.event_type === "pending") ?? false;
  const status = getSessionStatus(session);
  const titlebarStatus = hasPendingOutput ? { label: "响应中", color: "var(--forge-accent)" } : status;
  const showSessionStatus = hasPendingOutput || session?.streaming || session?.status === "error";
  const titlebarStatusState = session?.status === "error"
    ? "error"
    : hasPendingOutput || session?.streaming
      ? "running"
      : "idle";

  return (
    <div
      data-testid="app-titlebar"
      data-tauri-drag-region="true"
      className="forge-titlebar forge-app-titlebar"
    >
      <div data-testid="titlebar-context" className="forge-titlebar-context">
        <div className="forge-titlebar-title-row">
          <span data-testid="titlebar-title" className="forge-titlebar-title">
            {getSessionTitle(session)}
          </span>
          {showSessionStatus && (
            <span
              data-testid="titlebar-status-pill"
              data-state={titlebarStatusState}
              className="forge-titlebar-status-pill"
              style={{
                color: titlebarStatus.color,
                borderColor: `color-mix(in srgb, ${titlebarStatus.color} 28%, transparent)`,
                backgroundColor: `color-mix(in srgb, ${titlebarStatus.color} 10%, transparent)`,
              }}
            >
              <span className="forge-titlebar-status-dot" style={{ background: titlebarStatus.color }} />
              {titlebarStatus.label}
            </span>
          )}
        </div>
        <div
          data-testid="titlebar-project-boundary"
          aria-label="当前项目边界"
          className="forge-titlebar-project"
          title={project.path}
        >
          <FolderOpen className="forge-titlebar-project-icon" />
          <span className="forge-titlebar-project-label">当前项目</span>
          <span className="forge-titlebar-project-name">{project.name}</span>
        </div>
      </div>

      <div data-testid="titlebar-actions" className="forge-titlebar-actions">
        <ButtonPrimitive
          type="button"
          onClick={onOpenSearch}
          aria-label="搜索"
          title="搜索"
          className="forge-titlebar-button"
        >
          <Search className="size-3.5" />
        </ButtonPrimitive>
        <ButtonPrimitive
          type="button"
          onClick={onOpenWorkPanel}
          aria-label="打开工作面板"
          title="打开工作面板"
          className="forge-titlebar-button"
        >
          <PanelRightOpen className="size-4" />
        </ButtonPrimitive>
      </div>
    </div>
  );
}
