import { useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { FolderOpen } from "lucide-react";
import { useActiveWorkspace, useStore, useWorkspaceList } from "@/store";
import { hasTauriRuntime, pickWorkspaceFolder } from "@/lib/tauri";
import { isBroadWorkspacePath, workspaceFromPath } from "@/lib/workspaces";
import { SidebarWorkspaceMenuContent } from "./SidebarWorkspaceMenuContent";

interface SidebarWorkspaceMenuProps {
  onWorkspaceActivated: () => void;
}

export function SidebarWorkspaceMenu({ onWorkspaceActivated }: SidebarWorkspaceMenuProps) {
  const [workspaceMenuOpen, setWorkspaceMenuOpen] = useState(false);
  const [manualWorkspaceEntry, setManualWorkspaceEntry] = useState(false);
  const [workspacePathDraft, setWorkspacePathDraft] = useState("");
  const [workspacePathError, setWorkspacePathError] = useState<string | null>(null);
  const [choosingWorkspace, setChoosingWorkspace] = useState(false);
  const activeWorkspace = useActiveWorkspace();
  const workspaces = useWorkspaceList();
  const setActiveWorkspace = useStore((s) => s.setActiveWorkspace);
  const upsertWorkspace = useStore((s) => s.upsertWorkspace);
  const removeWorkspace = useStore((s) => s.removeWorkspace);

  const toggleWorkspaceMenu = () => {
    setWorkspaceMenuOpen((open) => {
      if (open) {
        setManualWorkspaceEntry(false);
        setWorkspacePathError(null);
      }
      return !open;
    });
  };

  const activateWorkspacePath = (path: string): boolean => {
    if (!path) {
      setWorkspacePathError("请输入一个项目文件夹路径。");
      return false;
    }
    if (isBroadWorkspacePath(path)) {
      setWorkspacePathError("请选择具体项目文件夹，不要直接使用用户主目录。");
      return false;
    }
    const workspace = workspaceFromPath(path);
    if (!workspace) {
      setWorkspacePathError("这个路径暂时不能作为项目文件夹。");
      return false;
    }
    upsertWorkspace(workspace);
    onWorkspaceActivated();
    setWorkspacePathDraft("");
    setWorkspacePathError(null);
    setManualWorkspaceEntry(false);
    setWorkspaceMenuOpen(false);
    return true;
  };

  const addWorkspaceFromDraft = () => {
    activateWorkspacePath(workspacePathDraft.trim());
  };

  const chooseWorkspaceFolder = async () => {
    setWorkspacePathError(null);
    setChoosingWorkspace(true);
    try {
      const selectedPath = await pickWorkspaceFolder();
      if (!selectedPath) {
        if (!hasTauriRuntime()) setManualWorkspaceEntry(true);
        return;
      }
      activateWorkspacePath(selectedPath);
    } catch (error) {
      console.error("Failed to choose workspace folder:", error);
      setWorkspacePathError("没有打开文件夹选择器，请使用手动输入。");
      setManualWorkspaceEntry(true);
    } finally {
      setChoosingWorkspace(false);
    }
  };

  return (
    <div data-forge-motion="sidebar-entry" className="forge-sidebar-workspace-shell relative">
      <ButtonPrimitive
        type="button"
        data-testid="workspace-trigger"
        onClick={toggleWorkspaceMenu}
        title={activeWorkspace?.path}
        className="forge-sidebar-workspace-trigger"
        aria-controls={workspaceMenuOpen ? "workspace-menu" : undefined}
        aria-expanded={workspaceMenuOpen}
        aria-haspopup="menu"
      >
        <FolderOpen className="size-3.5 shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate text-[12px] font-medium">
          {activeWorkspace?.name ?? "选择项目"}
        </span>
      </ButtonPrimitive>
      {workspaceMenuOpen && (
        <SidebarWorkspaceMenuContent
          activeWorkspace={activeWorkspace}
          choosingWorkspace={choosingWorkspace}
          manualWorkspaceEntry={manualWorkspaceEntry}
          workspacePathDraft={workspacePathDraft}
          workspacePathError={workspacePathError}
          workspaces={workspaces}
          onAddWorkspaceFromDraft={addWorkspaceFromDraft}
          onCancelManualWorkspaceEntry={() => {
            setManualWorkspaceEntry(false);
            setWorkspacePathError(null);
          }}
          onChooseWorkspaceFolder={chooseWorkspaceFolder}
          onCloseMenu={() => setWorkspaceMenuOpen(false)}
          onManualWorkspaceEntry={() => {
            setManualWorkspaceEntry(true);
            setWorkspacePathError(null);
          }}
          onRemoveActiveWorkspace={() => {
            if (!activeWorkspace) return;
            removeWorkspace(activeWorkspace.id);
            setWorkspaceMenuOpen(false);
          }}
          onSelectWorkspace={(workspaceId) => {
            setActiveWorkspace(workspaceId);
            setWorkspaceMenuOpen(false);
          }}
          onWorkspacePathDraftChange={(value) => {
            setWorkspacePathDraft(value);
            setWorkspacePathError(null);
          }}
        />
      )}
    </div>
  );
}
