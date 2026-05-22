import { ArrowUp, ChevronDown, RotateCcw, X } from "lucide-react";
import { composerToolbarIcons } from "@/lib/capability-icons";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { cn } from "@/lib/utils";
import type { ComposerMenuMode } from "./composerTypes";

interface ComposerToolbarProps {
  canSend: boolean;
  isResuming: boolean;
  isRunning: boolean;
  isStreaming: boolean;
  modelMenuId: string;
  selectedContextWindow: string;
  selectedModelLabel: string;
  selectedProviderLabel: string;
  showModelMenu: boolean;
  showSuggestions: ComposerMenuMode;
  suggestionListId: string;
  onResume: () => void;
  onSend: () => void;
  onStop: () => void;
  onToggleModelMenu: () => void;
  onToggleSuggestion: (mode: Exclude<ComposerMenuMode, null>) => void;
}

export function ComposerToolbar({
  canSend,
  isResuming,
  isRunning,
  isStreaming,
  modelMenuId,
  selectedContextWindow,
  selectedModelLabel,
  selectedProviderLabel,
  showModelMenu,
  showSuggestions,
  suggestionListId,
  onResume,
  onSend,
  onStop,
  onToggleModelMenu,
  onToggleSuggestion,
}: ComposerToolbarProps) {
  return (
    <div data-testid="composer-toolbar" className="forge-composer-toolbar">
      <div data-testid="composer-tool-cluster" className="forge-composer-tool-cluster">
        <div className="forge-composer-tool-buttons">
          <button
            type="button"
            data-testid="composer-tool-button"
            aria-label="引用文件"
            aria-controls={showSuggestions === "@" ? suggestionListId : undefined}
            aria-expanded={showSuggestions === "@"}
            aria-haspopup="listbox"
            title="引用文件"
            onClick={() => onToggleSuggestion("@")}
            data-active={showSuggestions === "@" ? "true" : "false"}
            className="forge-composer-tool"
          >
            <ForgeIcon icon={composerToolbarIcons.file.icon} tone={composerToolbarIcons.file.tone} contained={false} />
          </button>
          <button
            type="button"
            data-testid="composer-tool-button"
            aria-label="常用请求"
            aria-controls={showSuggestions === "/" ? suggestionListId : undefined}
            aria-expanded={showSuggestions === "/"}
            aria-haspopup="listbox"
            title="常用请求"
            onClick={() => onToggleSuggestion("/")}
            data-active={showSuggestions === "/" ? "true" : "false"}
            className="forge-composer-tool"
          >
            <ForgeIcon icon={composerToolbarIcons.command.icon} tone={composerToolbarIcons.command.tone} contained={false} />
          </button>
        </div>
        <span className="forge-composer-hint hidden truncate sm:inline">
          Enter 发送 · Shift↵ 换行
        </span>
      </div>

      <div data-testid="composer-control-cluster" className="forge-composer-control-cluster">
        <div className="relative">
          <button
            type="button"
            data-testid="composer-model-chip"
            id={`${modelMenuId}-button`}
            onClick={onToggleModelMenu}
            aria-label={`模型：${selectedModelLabel}`}
            aria-controls={showModelMenu ? modelMenuId : undefined}
            aria-expanded={showModelMenu}
            aria-haspopup="menu"
            data-active={showModelMenu ? "true" : "false"}
            className="forge-composer-model"
            title={selectedContextWindow ? `${selectedProviderLabel} · ${selectedModelLabel} · 上下文 ${selectedContextWindow}` : `${selectedProviderLabel} · ${selectedModelLabel}`}
          >
            <span data-testid="composer-model-indicator" className="forge-composer-model-indicator" aria-hidden="true" />
            <span className="truncate">{selectedModelLabel}</span>
            <ChevronDown className="size-3" style={{ color: "var(--muted-foreground)" }} />
          </button>
        </div>

        {!isRunning ? (
          <button
            type="button"
            aria-label="继续会话"
            onClick={onResume}
            disabled={isResuming}
            className="forge-composer-resume disabled:cursor-default disabled:opacity-60"
          >
            <RotateCcw className={isResuming ? "size-3 animate-spin" : "size-3"} />
            {isResuming ? "恢复中" : "继续会话"}
          </button>
        ) : isStreaming ? (
          <button
            type="button"
            data-testid="composer-stop"
            aria-label="停止生成"
            onClick={onStop}
            className="forge-composer-send text-destructive hover:border-destructive/35 hover:bg-destructive/10"
          >
            <X className="size-4" />
          </button>
        ) : (
          <button
            type="button"
            data-testid="composer-send"
            aria-label="发送"
            onClick={onSend}
            disabled={!canSend}
            data-ready={canSend}
            className={cn("forge-composer-send", canSend && "text-primary")}
          >
            <ArrowUp className="size-4" />
          </button>
        )}
      </div>
    </div>
  );
}
