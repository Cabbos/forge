import { useEffect } from "react";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";
import { Bug, CheckCircle2, Compass, MessageSquarePlus, Moon, Sun, Zap } from "lucide-react";
import { useStore } from "@/store";
import { useSession } from "@/hooks/useSession";
import { getDefaultWorkingDir, getRememberedWorkingDir, overrideWorkflowRoute } from "@/lib/tauri";
import type { WorkflowOverrideAction } from "@/lib/protocol";
import { getSessionMeta, getSessionStatus, getSessionTitle } from "@/lib/session-display";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useStore((s) => s.sessions);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const setWorkflowState = useStore((s) => s.setWorkflowState);
  const { create } = useSession();

  // Keyboard shortcut
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        onOpenChange(true);
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, [onOpenChange]);

  const handleCreate = async () => {
    onOpenChange(false);
    try {
      const remembered = getRememberedWorkingDir();
      const fallback = await getDefaultWorkingDir();
      const workingDir = remembered && !isBroadLocalPath(remembered) ? remembered : fallback;
      if (isBroadLocalPath(workingDir)) {
        alert("请选择一个具体的项目文件夹，不要直接使用用户主目录。");
        return;
      }
      await create(workingDir, selectedProvider, selectedModel);
    } catch (e) {
      console.error("Failed to create session:", e);
    }
  };

  const handleWorkflowOverride = async (action: WorkflowOverrideAction) => {
    if (!activeSessionId) return;
    onOpenChange(false);
    try {
      const workflow = await overrideWorkflowRoute(activeSessionId, action);
      setWorkflowState(activeSessionId, workflow);
    } catch (error) {
      console.error("Failed to override workflow:", error);
    }
  };

  const sessionList = Array.from(sessions.values());

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <Command>
        <CommandInput placeholder="搜索任务、命令或设置..." />
        <CommandList>
          <CommandEmpty>没有匹配结果</CommandEmpty>

          <CommandGroup heading="操作">
            <CommandItem onSelect={handleCreate}>
              <MessageSquarePlus className="size-4" />
              新建任务
            </CommandItem>
          </CommandGroup>

          {activeSessionId && (
            <>
              <CommandSeparator />
              <CommandGroup heading="工作方式">
                <CommandItem onSelect={() => handleWorkflowOverride("plan_first")}>
                  <Compass className="size-4" />
                  先梳理方案
                </CommandItem>
                <CommandItem onSelect={() => handleWorkflowOverride("direct")}>
                  <Zap className="size-4" />
                  直接处理
                </CommandItem>
                <CommandItem onSelect={() => handleWorkflowOverride("debug")}>
                  <Bug className="size-4" />
                  排查问题
                </CommandItem>
                <CommandItem onSelect={() => handleWorkflowOverride("verify")}>
                  <CheckCircle2 className="size-4" />
                  检查结果
                </CommandItem>
              </CommandGroup>
            </>
          )}

          {sessionList.length > 0 && (
            <>
              <CommandSeparator />
              <CommandGroup heading="切换任务">
                {sessionList.map((s) => {
                  const status = getSessionStatus(s);
                  return (
                    <CommandItem
                      key={s.id}
                      onSelect={() => {
                        setActiveSession(s.id);
                        onOpenChange(false);
                      }}
                    >
                      <span className="size-2 flex-shrink-0 rounded-full" style={{ background: status.color }} />
                      <span className="min-w-0 flex-1 truncate">{getSessionTitle(s)}</span>
                      <span className="shrink-0 text-xs text-muted-foreground">{getSessionMeta(s)}</span>
                    </CommandItem>
                  );
                })}
              </CommandGroup>
            </>
          )}

          <CommandSeparator />
          <CommandGroup heading="偏好设置">
            <CommandItem
              onSelect={() => setTheme(theme === "dark" ? "light" : "dark")}
            >
              {theme === "dark" ? (
                <Sun className="size-4" />
              ) : (
                <Moon className="size-4" />
              )}
              切换主题（{theme === "dark" ? "浅色" : "深色"}）
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  );
}

function isBroadLocalPath(path: string): boolean {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return true;
  return /^\/Users\/[^/]+$/.test(normalized) || /^\/home\/[^/]+$/.test(normalized);
}
