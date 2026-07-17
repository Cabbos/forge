import {
  ForgeCommand,
  ForgeCommandEmpty,
  ForgeCommandGroup,
  ForgeCommandInput,
  ForgeCommandItem,
  ForgeCommandList,
  ForgeCommandSeparator,
} from "@/components/primitives/command";
import { Bug, CheckCircle2, Compass, FolderOpen, MessageSquarePlus, Moon, PanelRightOpen, Settings, Sun, Zap } from "lucide-react";
import { ForgeIcon } from "@/components/primitives/icon";
import type { SessionState, WorkflowOverrideAction } from "@/lib/protocol";
import { getSessionTitle } from "@/lib/session-display";
import type { Workspace } from "@/lib/workspaces";

interface CommandPaletteContentProps {
  activeWorkspace: Workspace | null;
  notice: string;
  sessions: SessionState[];
  activeSessionId: string | null;
  theme: "light" | "dark";
  onCreate: () => void;
  onOpenWorkPanel: () => void;
  onOpenSettings: () => void;
  onWorkflowOverride: (action: WorkflowOverrideAction) => void;
  onSelectSession: (sessionId: string) => void;
  onToggleTheme: () => void;
}

export function CommandPaletteContent({
  activeWorkspace,
  notice,
  sessions,
  activeSessionId,
  theme,
  onCreate,
  onOpenWorkPanel,
  onOpenSettings,
  onWorkflowOverride,
  onSelectSession,
  onToggleTheme,
}: CommandPaletteContentProps) {
  return (
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
          <ForgeCommandItem onSelect={onCreate} disabled={!activeWorkspace}>
            <ForgeIcon icon={MessageSquarePlus} tone="action" />
            <span className="min-w-0 flex-1 truncate">{activeWorkspace ? "新建对话" : "先选择项目"}</span>
            {activeWorkspace && <ShortcutHint keys="⌘N" />}
          </ForgeCommandItem>
          <ForgeCommandItem onSelect={onOpenWorkPanel}>
            <ForgeIcon icon={PanelRightOpen} tone="context" />
            <span className="min-w-0 flex-1 truncate">打开工作面板</span>
            <ShortcutHint keys="⌘I" />
          </ForgeCommandItem>
          <ForgeCommandItem onSelect={onOpenSettings}>
            <ForgeIcon icon={Settings} tone="neutral" />
            <span className="min-w-0 flex-1 truncate">设置</span>
            <ShortcutHint keys="⌘," />
          </ForgeCommandItem>
        </ForgeCommandGroup>

        {activeSessionId && (
          <>
            <ForgeCommandSeparator />
            <ForgeCommandGroup data-forge-motion="command-entry" heading="当前任务">
              <ForgeCommandItem onSelect={() => onWorkflowOverride("plan_first")}>
                <ForgeIcon icon={Compass} tone="reasoning" />
                先梳理方案
              </ForgeCommandItem>
              <ForgeCommandItem onSelect={() => onWorkflowOverride("direct")}>
                <ForgeIcon icon={Zap} tone="action" />
                直接处理
              </ForgeCommandItem>
              <ForgeCommandItem onSelect={() => onWorkflowOverride("debug")}>
                <ForgeIcon icon={Bug} tone="safety" />
                排查问题
              </ForgeCommandItem>
              <ForgeCommandItem onSelect={() => onWorkflowOverride("verify")}>
                <ForgeIcon icon={CheckCircle2} tone="safety" />
                检查结果
              </ForgeCommandItem>
            </ForgeCommandGroup>
          </>
        )}

        {sessions.length > 0 && (
          <>
            <ForgeCommandSeparator />
            <ForgeCommandGroup data-forge-motion="command-entry" heading="最近对话">
              {sessions.map((session) => (
                <ForgeCommandItem
                  key={session.id}
                  onSelect={() => onSelectSession(session.id)}
                >
                  <span className="min-w-0 flex-1 truncate">{getSessionTitle(session)}</span>
                </ForgeCommandItem>
              ))}
            </ForgeCommandGroup>
          </>
        )}

        <ForgeCommandSeparator />
        <ForgeCommandGroup data-forge-motion="command-entry" heading="外观">
          <ForgeCommandItem onSelect={onToggleTheme}>
            <ForgeIcon icon={theme === "dark" ? Sun : Moon} tone="neutral" />
            切换主题（{theme === "dark" ? "浅色" : "深色"}）
          </ForgeCommandItem>
        </ForgeCommandGroup>
      </ForgeCommandList>
    </ForgeCommand>
  );
}

function ShortcutHint({ keys }: { keys: string }) {
  return (
    <span data-testid="command-shortcut" className="forge-command-shortcut">
      {keys}
    </span>
  );
}
