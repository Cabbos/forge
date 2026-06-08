import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { FolderOpen, Trash2 } from "lucide-react";
import type { Workspace } from "@/lib/workspaces";

interface SidebarWorkspaceMenuContentProps {
  activeWorkspace: Workspace | null;
  choosingWorkspace: boolean;
  manualWorkspaceEntry: boolean;
  workspacePathDraft: string;
  workspacePathError: string | null;
  workspaces: Workspace[];
  onAddWorkspaceFromDraft: () => void;
  onCancelManualWorkspaceEntry: () => void;
  onChooseWorkspaceFolder: () => void;
  onCloseMenu: () => void;
  onManualWorkspaceEntry: () => void;
  onRemoveActiveWorkspace: () => void;
  onSelectWorkspace: (workspaceId: string) => void;
  onWorkspacePathDraftChange: (value: string) => void;
}

export function SidebarWorkspaceMenuContent({
  activeWorkspace,
  choosingWorkspace,
  manualWorkspaceEntry,
  workspacePathDraft,
  workspacePathError,
  workspaces,
  onAddWorkspaceFromDraft,
  onCancelManualWorkspaceEntry,
  onChooseWorkspaceFolder,
  onCloseMenu,
  onManualWorkspaceEntry,
  onRemoveActiveWorkspace,
  onSelectWorkspace,
  onWorkspacePathDraftChange,
}: SidebarWorkspaceMenuContentProps) {
  return (
    <div
      id="workspace-menu"
      role="menu"
      aria-label="项目文件夹"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          onCloseMenu();
        }
      }}
      className="forge-floating-menu forge-sidebar-menu"
    >
      {workspaces.length > 0 && (
        <div className="max-h-52 overflow-y-auto py-1">
          {workspaces.map((workspace) => (
            <ButtonPrimitive
              key={workspace.id}
              type="button"
              role="menuitemradio"
              aria-checked={workspace.id === activeWorkspace?.id}
              title={workspace.path}
              onClick={() => onSelectWorkspace(workspace.id)}
              className="forge-menu-option"
            >
              <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate text-foreground">{workspace.name}</span>
            </ButtonPrimitive>
          ))}
        </div>
      )}
      <ButtonPrimitive
        type="button"
        role="menuitem"
        onClick={onChooseWorkspaceFolder}
        disabled={choosingWorkspace}
        className="forge-menu-option border-t border-border text-foreground disabled:cursor-default disabled:opacity-60"
      >
        <FolderOpen className="size-3.5" />
        {choosingWorkspace ? "正在打开..." : "选择文件夹"}
      </ButtonPrimitive>
      <ButtonPrimitive
        type="button"
        role="menuitem"
        onClick={onManualWorkspaceEntry}
        className="forge-menu-option border-t border-border text-muted-foreground hover:text-foreground"
      >
        <FolderOpen className="size-3.5" />
        手动输入路径
      </ButtonPrimitive>
      {activeWorkspace && workspaces.length > 1 && (
        <ButtonPrimitive
          type="button"
          role="menuitem"
          onClick={onRemoveActiveWorkspace}
          className="forge-menu-option border-t border-border text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
        >
          <Trash2 className="size-3.5" />
          从列表移除当前项目
        </ButtonPrimitive>
      )}
      {manualWorkspaceEntry && (
        <WorkspacePathForm
          workspacePathDraft={workspacePathDraft}
          workspacePathError={workspacePathError}
          onAddWorkspaceFromDraft={onAddWorkspaceFromDraft}
          onCancelManualWorkspaceEntry={onCancelManualWorkspaceEntry}
          onWorkspacePathDraftChange={onWorkspacePathDraftChange}
        />
      )}
    </div>
  );
}

function WorkspacePathForm({
  workspacePathDraft,
  workspacePathError,
  onAddWorkspaceFromDraft,
  onCancelManualWorkspaceEntry,
  onWorkspacePathDraftChange,
}: {
  workspacePathDraft: string;
  workspacePathError: string | null;
  onAddWorkspaceFromDraft: () => void;
  onCancelManualWorkspaceEntry: () => void;
  onWorkspacePathDraftChange: (value: string) => void;
}) {
  return (
    <form
      className="border-t border-border px-3 py-3"
      onSubmit={(event) => {
        event.preventDefault();
        onAddWorkspaceFromDraft();
      }}
    >
      <label htmlFor="workspace-path-input" className="block text-[10px] font-medium text-muted-foreground">
        项目文件夹路径
      </label>
      <input
        id="workspace-path-input"
        autoFocus
        value={workspacePathDraft}
        onChange={(event) => onWorkspacePathDraftChange(event.target.value)}
        placeholder="/Users/you/project/app"
        className="mt-1 h-8 w-full rounded-md border border-border bg-background px-2 text-xs text-foreground outline-none placeholder:text-muted-foreground/70 focus:border-primary"
      />
      {workspacePathError && (
        <p className="mt-1 text-[10px] leading-snug text-destructive">{workspacePathError}</p>
      )}
      <div className="mt-2 flex items-center justify-end gap-2">
        <ButtonPrimitive
          type="button"
          onClick={onCancelManualWorkspaceEntry}
          className="h-7 rounded-md px-2 text-[11px] text-muted-foreground hover:bg-secondary hover:text-foreground"
        >
          取消
        </ButtonPrimitive>
        <ButtonPrimitive
          type="submit"
          className="h-7 rounded-md bg-primary px-2.5 text-[11px] font-medium text-primary-foreground hover:opacity-90"
        >
          添加
        </ButtonPrimitive>
      </div>
    </form>
  );
}
