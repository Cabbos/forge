import { useEffect, useRef, useState } from "react";
import { ForgeCommandDialog } from "@/components/primitives/command";
import { CommandPaletteContent } from "@/components/CommandPaletteContent";
import { useActiveWorkspace, useSessionList, useStore } from "@/store";
import { useSession } from "@/hooks/useSession";
import { overrideWorkflowRoute } from "@/lib/tauri";
import type { WorkflowOverrideAction } from "@/lib/protocol";
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

  const handleOpenWorkPanel = () => {
    onOpenChange(false);
    window.dispatchEvent(new Event("open-work-panel"));
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

  const handleSelectSession = (sessionId: string) => {
    setActiveSession(sessionId);
    onOpenChange(false);
  };

  const handleToggleTheme = () => {
    setTheme(theme === "dark" ? "light" : "dark");
  };

  return (
    <ForgeCommandDialog open={open} onOpenChange={onOpenChange} className="forge-command-dialog sm:max-w-[580px]">
      <div ref={paletteRef} className="forge-command-motion-root">
        <CommandPaletteContent
          activeWorkspace={activeWorkspace}
          notice={notice}
          sessions={sessions}
          activeSessionId={activeSessionId}
          theme={theme}
          onCreate={handleCreate}
        onOpenWorkPanel={handleOpenWorkPanel}
          onOpenSettings={handleOpenSettings}
          onWorkflowOverride={handleWorkflowOverride}
          onSelectSession={handleSelectSession}
          onToggleTheme={handleToggleTheme}
        />
      </div>
    </ForgeCommandDialog>
  );
}

function createSessionNotice(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (/api key|密钥/i.test(message)) {
    return "模型服务还没有可用密钥。选择下方「设置」添加后再试。";
  }
  return "新对话没有创建成功。请检查设置后重试。";
}
