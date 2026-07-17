import { useDeferredValue, useState } from "react";
import {
  ArrowLeft,
  FileDiff,
  FileText,
  FolderOpen,
  Globe2,
  ListTree,
  LoaderCircle,
  TerminalSquare,
  type LucideIcon,
} from "lucide-react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command";
import { useSearchWorkspaceFilesQuery } from "@/hooks/queries/useSearchWorkspaceFilesQuery";
import { useActiveWorkspace, useStore } from "@/store";
import {
  WORK_PANEL_LAUNCHER_ACTIONS,
  createFileTab,
  createPreviewFileTab,
  createPreviewUrlTab,
  createReviewTab,
  createSubtaskTab,
  createTerminalTab,
} from "./workPanelSelectors";
import type { WorkPanelLauncherAction, WorkPanelTab } from "./workPanelTypes";

const actionIcons: Record<WorkPanelLauncherAction, LucideIcon> = {
  review: FileDiff,
  terminal: TerminalSquare,
  preview: Globe2,
  files: FolderOpen,
  subtasks: ListTree,
};

type PickerAction = Exclude<WorkPanelLauncherAction, "review" | "terminal">;

interface WorkPanelLauncherProps {
  taskKey: string;
  onOpenTab: (tab: WorkPanelTab) => void;
}

export function WorkPanelLauncher({ taskKey, onOpenTab }: WorkPanelLauncherProps) {
  const [selection, setSelection] = useState<PickerAction | null>(null);
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query.trim());
  const activeSessionId = useStore((state) => state.activeSessionId);
  const activeSession = useStore((state) => activeSessionId ? state.sessions.get(activeSessionId) ?? null : null);
  const activeWorkspace = useActiveWorkspace();
  const subtasks = useStore((state) => activeSessionId
    ? state.agentA2ABySession.get(activeSessionId)?.tasks ?? []
    : []);
  const searchesFiles = selection === "files" || selection === "preview";
  const fileSearch = useSearchWorkspaceFilesQuery(
    deferredQuery,
    activeSessionId ?? undefined,
    activeSession?.workingDir ?? activeWorkspace?.path ?? null,
    searchesFiles && deferredQuery.length > 0,
  );

  const returnToRoot = () => {
    setSelection(null);
    setQuery("");
  };

  const handleAction = (action: WorkPanelLauncherAction) => {
    if (action === "review") {
      onOpenTab(createReviewTab(taskKey));
      return;
    }
    if (action === "terminal") {
      onOpenTab(createTerminalTab(taskKey));
      return;
    }
    setSelection(action);
    setQuery("");
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (event.key !== "Escape" || selection === null) return;
    event.preventDefault();
    event.stopPropagation();
    returnToRoot();
  };

  if (selection) {
    const previewUrlTab = selection === "preview" ? createPreviewUrlTab(query) : null;
    return (
      <div className="forge-work-panel-picker" data-testid={`work-panel-${selection}-picker`}>
        <div className="forge-work-panel-picker-heading">
          <ButtonPrimitive type="button" className="forge-work-panel-picker-back" onClick={returnToRoot}>
            <ArrowLeft className="size-4" />
            返回
          </ButtonPrimitive>
          <h3>{pickerTitle(selection)}</h3>
        </div>
        <Command
          className="forge-work-panel-command forge-work-panel-picker-command"
          shouldFilter={selection === "subtasks"}
          onKeyDown={handleKeyDown}
        >
          <CommandInput
            autoFocus
            value={query}
            onValueChange={setQuery}
            placeholder={pickerPlaceholder(selection)}
          />
          <CommandList>
            {selection === "preview" && previewUrlTab ? (
              <CommandGroup heading="网页">
                <CommandItem value={previewUrlTab.id} onSelect={() => onOpenTab(previewUrlTab)}>
                  <Globe2 className="size-4" />
                  <span>{previewUrlTab.target.type === "url" ? previewUrlTab.target.url : previewUrlTab.label}</span>
                </CommandItem>
              </CommandGroup>
            ) : null}

            {(selection === "files" || selection === "preview") && (fileSearch.data?.length ?? 0) > 0 ? (
              <CommandGroup heading={selection === "files" ? "工作区文件" : "可预览文件"}>
                {fileSearch.data?.map((path) => (
                  <CommandItem
                    key={path}
                    value={path}
                    onSelect={() => onOpenTab(selection === "files" ? createFileTab(path) : createPreviewFileTab(path))}
                  >
                    <FileText className="size-4" />
                    <span className="forge-work-panel-result-label">{path}</span>
                  </CommandItem>
                ))}
              </CommandGroup>
            ) : null}

            {selection === "subtasks" && subtasks.length > 0 ? (
              <CommandGroup heading="当前任务">
                {subtasks.map((task) => (
                  <CommandItem
                    key={task.task_id}
                    value={`${task.title} ${task.role} ${task.task_id}`}
                    onSelect={() => onOpenTab(createSubtaskTab(
                      activeSessionId ?? taskKey,
                      task.task_id,
                      task.title || task.role || task.task_id,
                    ))}
                  >
                    <ListTree className="size-4" />
                    <span className="forge-work-panel-result-label">{task.title || task.role || task.task_id}</span>
                    <span className="forge-work-panel-result-meta">{task.status}</span>
                  </CommandItem>
                ))}
              </CommandGroup>
            ) : null}

            {fileSearch.isFetching ? (
              <div className="forge-work-panel-command-status">
                <LoaderCircle className="size-4 animate-spin" />
                正在搜索
              </div>
            ) : null}
            {fileSearch.isError ? (
              <div className="forge-work-panel-command-status" role="alert">无法搜索工作区文件</div>
            ) : null}
            <CommandEmpty>{emptyMessage(selection, query)}</CommandEmpty>
          </CommandList>
        </Command>
      </div>
    );
  }

  return (
    <div className="forge-work-panel-launcher" data-testid="work-panel-launcher">
      <Command className="forge-work-panel-command forge-work-panel-launcher-command" onKeyDown={handleKeyDown}>
        <CommandInput autoFocus value={query} onValueChange={setQuery} placeholder="搜索工作面板" />
        <CommandList className="forge-work-panel-launcher-actions">
          <CommandEmpty>没有匹配的操作</CommandEmpty>
          <CommandGroup>
            {WORK_PANEL_LAUNCHER_ACTIONS.map((action) => {
              const Icon = actionIcons[action.id];
              return (
                <CommandItem
                  key={action.id}
                  role="button"
                  value={action.label}
                  className="forge-work-panel-launcher-action"
                  onSelect={() => handleAction(action.id)}
                >
                  <Icon className="size-5" />
                  <span>{action.label}</span>
                  {action.shortcut ? <CommandShortcut>{action.shortcut}</CommandShortcut> : null}
                </CommandItem>
              );
            })}
          </CommandGroup>
        </CommandList>
      </Command>
    </div>
  );
}

function pickerTitle(action: PickerAction) {
  switch (action) {
    case "preview":
      return "选择预览";
    case "files":
      return "选择文件";
    case "subtasks":
      return "选择子任务";
  }
}

function pickerPlaceholder(action: PickerAction) {
  switch (action) {
    case "preview":
      return "输入本机网址或搜索文件";
    case "files":
      return "搜索工作区文件";
    case "subtasks":
      return "搜索子任务";
  }
}

function emptyMessage(action: PickerAction, query: string) {
  if (action === "subtasks") return "当前没有可打开的子任务";
  if (!query.trim()) return action === "preview" ? "输入本机网址或文件名" : "输入文件名开始搜索";
  return action === "preview" ? "没有匹配的本机网页或文件" : "没有匹配的文件";
}
