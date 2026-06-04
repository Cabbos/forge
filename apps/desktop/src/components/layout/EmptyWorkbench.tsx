import type { KeyboardEventHandler, RefObject } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ArrowUp, FolderOpen, SquarePen } from "lucide-react";
import { StartReadinessCard } from "@/components/session/StartReadinessCard";
import forgeMark from "@/assets/forge-mark.svg";

const EMPTY_START_HINTS = [
  "我想做一个记录喝水次数的小工具",
  "我想做一个客户跟进小工具",
  "检查这个项目能不能运行",
];

export type EmptyStartMode = "new-tool" | "existing-project";

export interface EmptyWorkbenchProps {
  emptyComposerContext: string;
  emptyComposerPlaceholder: string;
  emptyPrompt: string;
  emptyPromptRef: RefObject<HTMLTextAreaElement>;
  emptyPromptStarting: boolean;
  emptyStartMode: EmptyStartMode | null;
  emptyWorkspaceNotice: string | null;
  hasActiveWorkspace: boolean;
  onEmptyPromptChange: (value: string) => void;
  onEmptyPromptKeyDown: KeyboardEventHandler<HTMLTextAreaElement>;
  onSelectExistingProjectEntry: () => void;
  onSelectNewToolEntry: () => void;
  onStartConversation: () => void;
  onStartConversationWithPrompt: () => void;
  onUseEmptyHint: (hint: string) => void;
  project: {
    name: string;
    path: string;
  };
}

export function EmptyWorkbench({
  emptyComposerContext,
  emptyComposerPlaceholder,
  emptyPrompt,
  emptyPromptRef,
  emptyPromptStarting,
  emptyStartMode,
  emptyWorkspaceNotice,
  hasActiveWorkspace,
  onEmptyPromptChange,
  onEmptyPromptKeyDown,
  onSelectExistingProjectEntry,
  onSelectNewToolEntry,
  onStartConversation,
  onStartConversationWithPrompt,
  onUseEmptyHint,
  project,
}: EmptyWorkbenchProps) {
  const emptyHeading = hasActiveWorkspace ? "从当前项目开始" : "选择项目开始";
  const emptySubheading = hasActiveWorkspace
    ? "选择一个入口，或直接在下方输入下一步任务。"
    : "先绑定一个本地文件夹，Forge 才会开始行动。";

  return (
    <div
      className={hasActiveWorkspace ? "forge-empty-shell forge-empty-shell-codex" : "forge-empty-shell forge-empty-shell-centered"}
    >
      <div data-testid="empty-workbench" className="forge-empty-workbench">
        <div data-testid="empty-middle-hints" className="forge-empty-hints">
          <div className="forge-empty-hints-inner">
            <div className="forge-empty-identity" data-forge-motion="empty-entry">
              <img src={forgeMark} alt="" className="forge-empty-identity-mark" />
              <div className="forge-empty-identity-copy">
                <span className="forge-empty-kicker">Forge Workbench</span>
                <h1 className="forge-empty-heading">{emptyHeading}</h1>
                <p className="forge-empty-subheading">{emptySubheading}</p>
              </div>
            </div>
            <div className="forge-empty-entry-grid" aria-label="开始方式">
              <ButtonPrimitive
                type="button"
                data-testid="empty-entry-new-tool"
                data-active={emptyStartMode === "new-tool"}
                data-forge-motion="empty-entry"
                onClick={onSelectNewToolEntry}
                className="forge-empty-entry-card"
              >
                <span className="forge-empty-entry-icon">
                  <SquarePen className="size-4" />
                </span>
                <span className="forge-empty-entry-copy">
                  <span className="forge-empty-entry-title">做个新工具</span>
                  <span className="forge-empty-entry-desc">
                    从一句想法开始，先做可预览的本地网页第一版。
                  </span>
                </span>
              </ButtonPrimitive>
              <ButtonPrimitive
                type="button"
                data-testid="empty-entry-existing-project"
                data-active={emptyStartMode === "existing-project"}
                data-forge-motion="empty-entry"
                onClick={onSelectExistingProjectEntry}
                className="forge-empty-entry-card"
              >
                <span className="forge-empty-entry-icon">
                  <FolderOpen className="size-4" />
                </span>
                <span className="forge-empty-entry-copy">
                  <span className="forge-empty-entry-title">打开已有项目</span>
                  <span className="forge-empty-entry-desc">
                    继续修改、检查、预览，所有动作绑定当前文件夹。
                  </span>
                </span>
              </ButtonPrimitive>
            </div>
            {hasActiveWorkspace ? (
              <>
                <p className="forge-empty-hints-title">可以这样开始</p>
                <div className="forge-empty-hint-list">
                  {EMPTY_START_HINTS.map((hint) => (
                    <ButtonPrimitive
                      key={hint}
                      type="button"
                      data-forge-motion="empty-hint"
                      onClick={() => onUseEmptyHint(hint)}
                      className="forge-empty-hint"
                    >
                      {hint}
                    </ButtonPrimitive>
                  ))}
                </div>
              </>
            ) : (
              <p data-testid="empty-workspace-notice" className="forge-empty-workspace-notice">
                {emptyWorkspaceNotice ?? "选择保存位置或已有项目后，就可以开始对话。"}
              </p>
            )}
          </div>
        </div>
      </div>
      {hasActiveWorkspace && (
        <div className="forge-empty-composer-frame">
          <div className="forge-conversation-lane">
            <div data-forge-motion="empty-context" className="forge-empty-context-row">
              <div data-testid="empty-workbench-project" className="forge-empty-project">
                <FolderOpen className="forge-empty-project-icon" />
                <span className="forge-empty-project-name">{project.name}</span>
              </div>
              <ButtonPrimitive
                type="button"
                data-testid="empty-workbench-action"
                onClick={onStartConversation}
                className="forge-empty-action"
              >
                <SquarePen className="size-3.5" />
                开始新对话
              </ButtonPrimitive>
            </div>
            <div data-testid="empty-start-composer" data-forge-motion="empty-composer" className="forge-empty-composer">
              <textarea
                ref={emptyPromptRef}
                value={emptyPrompt}
                onChange={(event) => onEmptyPromptChange(event.target.value)}
                onKeyDown={onEmptyPromptKeyDown}
                placeholder={emptyComposerPlaceholder}
                rows={3}
                className="forge-empty-composer-input"
              />
              <div className="forge-empty-composer-footer">
                <span className="forge-empty-composer-context">{emptyComposerContext}</span>
                <ButtonPrimitive
                  type="button"
                  data-testid="empty-start-send"
                  aria-label="发送并开始"
                  onClick={onStartConversationWithPrompt}
                  disabled={!emptyPrompt.trim() || emptyPromptStarting}
                  data-ready={emptyPrompt.trim() ? "true" : "false"}
                  className="forge-empty-composer-send"
                >
                  <ArrowUp className="size-4" />
                </ButtonPrimitive>
              </div>
            </div>
            <div data-forge-motion="empty-readiness">
              <StartReadinessCard variant="setup-strip" />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
