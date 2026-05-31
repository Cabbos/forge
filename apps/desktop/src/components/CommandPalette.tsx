import { useEffect, useRef, useState } from "react";
import {
  ForgeCommand,
  ForgeCommandDialog,
  ForgeCommandEmpty,
  ForgeCommandGroup,
  ForgeCommandInput,
  ForgeCommandItem,
  ForgeCommandList,
  ForgeCommandSeparator,
} from "@/components/primitives/command";
import { Bug, CheckCircle2, Compass, FolderOpen, MessageSquarePlus, Moon, PanelRightOpen, Settings, Sun, Zap } from "lucide-react";
import { ForgeIcon } from "@/components/primitives/icon";
import { useActiveWorkspace, useSessionList, useStore } from "@/store";
import { useSession } from "@/hooks/useSession";
import { overrideWorkflowRoute } from "@/lib/tauri";
import type { WorkflowOverrideAction } from "@/lib/protocol";
import { getSessionTitle } from "@/lib/session-display";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const paletteRef = useRef<HTMLDivElement>(null);
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useSessionList();
  const activeWorkspace = useActiveWorkspace();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const setWorkflowState = useStore((s) => s.setWorkflowState);
  const { create } = useSession();
  const [notice, setNotice] = useState("");

  useGSAP(() => {
    if (!open || prefersReducedMotion()) return;
    const palette = paletteRef.current;
    if (!palette) return;

    const entries = gsap.utils.toArray<HTMLElement>(
      "[data-forge-motion='command-entry']",
      palette,
    );
    const timeline = gsap.timeline();
    timeline.fromTo(
      palette,
      { autoAlpha: 0, y: 8, scale: 0.99 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
    if (entries.length > 0) {
      timeline.fromTo(
        entries,
        { autoAlpha: 0, y: 4 },
        {
          autoAlpha: 1,
          y: 0,
          duration: forgeMotion.evidence.duration,
          ease: forgeMotion.evidence.ease,
          stagger: 0.02,
          clearProps: "transform,opacity,visibility",
        },
        "-=0.1",
      );
    }
  }, { scope: paletteRef, dependencies: [open, sessions.length, activeWorkspace?.id] });

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

  useEffect(() => {
    if (!open) setNotice("");
  }, [open]);

  const handleCreate = async () => {
    if (!activeWorkspace) {
      setNotice("先选择一个具体项目，再开始新对话。");
      return;
    }
    try {
      await create(activeWorkspace.path, selectedProvider, selectedModel);
      onOpenChange(false);
    } catch (e) {
      console.error("Failed to create session:", e);
      setNotice(createSessionNotice(e));
    }
  };

  const handleOpenProjectArchive = () => {
    onOpenChange(false);
    window.dispatchEvent(new Event("open-hub"));
  };

  const handleOpenSettings = () => {
    onOpenChange(false);
    window.dispatchEvent(new Event("forge:open-settings"));
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

  const sessionList = sessions;

  return (
    <ForgeCommandDialog open={open} onOpenChange={onOpenChange} className="forge-command-dialog sm:max-w-[580px]">
      <div ref={paletteRef} className="forge-command-motion-root">
        <ForgeCommand data-testid="command-palette-surface" className="forge-command-surface">
          <ForgeCommandInput placeholder="搜索或输入命令..." className="forge-command-input" />
          <ForgeCommandList className="forge-command-list">
            <ForgeCommandEmpty>没有匹配结果</ForgeCommandEmpty>

            {activeWorkspace && (
              <div data-forge-motion="command-entry" className="forge-command-context-strip">
                <ForgeIcon icon={FolderOpen} tone="context" contained={false} />
                <span className="min-w-0 truncate">当前项目 · {activeWorkspace.name}</span>
              </div>
            )}

            {notice && (
              <div role="status" data-forge-motion="command-entry" className="forge-command-notice">
                {notice}
              </div>
            )}

            <ForgeCommandGroup data-forge-motion="command-entry" heading="常用">
              <ForgeCommandItem onSelect={handleCreate} disabled={!activeWorkspace}>
                <ForgeIcon icon={MessageSquarePlus} tone="action" />
                <span className="min-w-0 flex-1 truncate">{activeWorkspace ? "新建对话" : "先选择项目"}</span>
                {activeWorkspace && <ShortcutHint keys="⌘N" />}
              </ForgeCommandItem>
              <ForgeCommandItem onSelect={handleOpenProjectArchive}>
                <ForgeIcon icon={PanelRightOpen} tone="context" />
                <span className="min-w-0 flex-1 truncate">打开项目档案</span>
                <ShortcutHint keys="⌘I" />
              </ForgeCommandItem>
              <ForgeCommandItem onSelect={handleOpenSettings}>
                <ForgeIcon icon={Settings} tone="neutral" />
                <span className="min-w-0 flex-1 truncate">设置</span>
                <ShortcutHint keys="⌘," />
              </ForgeCommandItem>
            </ForgeCommandGroup>

            {activeSessionId && (
              <>
                <ForgeCommandSeparator />
                <ForgeCommandGroup data-forge-motion="command-entry" heading="当前任务">
                  <ForgeCommandItem onSelect={() => handleWorkflowOverride("plan_first")}>
                    <ForgeIcon icon={Compass} tone="reasoning" />
                    先梳理方案
                  </ForgeCommandItem>
                  <ForgeCommandItem onSelect={() => handleWorkflowOverride("direct")}>
                    <ForgeIcon icon={Zap} tone="action" />
                    直接处理
                  </ForgeCommandItem>
                  <ForgeCommandItem onSelect={() => handleWorkflowOverride("debug")}>
                    <ForgeIcon icon={Bug} tone="safety" />
                    排查问题
                  </ForgeCommandItem>
                  <ForgeCommandItem onSelect={() => handleWorkflowOverride("verify")}>
                    <ForgeIcon icon={CheckCircle2} tone="safety" />
                    检查结果
                  </ForgeCommandItem>
                </ForgeCommandGroup>
              </>
            )}

            {sessionList.length > 0 && (
              <>
                <ForgeCommandSeparator />
                <ForgeCommandGroup data-forge-motion="command-entry" heading="最近对话">
                  {sessionList.map((s) => (
                    <ForgeCommandItem
                      key={s.id}
                      onSelect={() => {
                        setActiveSession(s.id);
                        onOpenChange(false);
                      }}
                    >
                      <span className="min-w-0 flex-1 truncate">{getSessionTitle(s)}</span>
                    </ForgeCommandItem>
                  ))}
                </ForgeCommandGroup>
              </>
            )}

            <ForgeCommandSeparator />
            <ForgeCommandGroup data-forge-motion="command-entry" heading="外观">
              <ForgeCommandItem
                onSelect={() => setTheme(theme === "dark" ? "light" : "dark")}
              >
                <ForgeIcon icon={theme === "dark" ? Sun : Moon} tone="neutral" />
                切换主题（{theme === "dark" ? "浅色" : "深色"}）
              </ForgeCommandItem>
            </ForgeCommandGroup>
          </ForgeCommandList>
        </ForgeCommand>
      </div>
    </ForgeCommandDialog>
  );
}

function ShortcutHint({ keys }: { keys: string }) {
  return (
    <span data-testid="command-shortcut" className="forge-command-shortcut">
      {keys}
    </span>
  );
}

function createSessionNotice(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (/api key|密钥/i.test(message)) {
    return "模型服务还没有可用密钥。选择下方「设置」添加后再试。";
  }
  return "新对话没有创建成功。请检查设置后重试。";
}
