import { ComposerMenuLayer } from "./ComposerMenuLayer";
import { ComposerPermissionModeButton } from "./ComposerPermissionModeButton";
import { ComposerResumeError } from "./ComposerResumeError";
import { ComposerSurface } from "./ComposerSurface";
import { useComposerController } from "./useComposerController";

interface InputBarProps { sessionId: string }

export function InputBar({ sessionId }: InputBarProps) {
  const composer = useComposerController(sessionId);

  return (
    <div data-testid="composer-frame" data-surface="composer" className="forge-composer-frame relative flex-shrink-0">
      <div ref={composer.composerRootRef} data-testid="composer-lane" className="forge-conversation-lane relative">
        <ComposerResumeError message={composer.resumeErrorMessage} />
        <ComposerMenuLayer {...composer.menuLayerProps} />
        <ComposerSurface
          ref={composer.textareaRef}
          {...composer.surfaceProps}
          permissionControl={<ComposerPermissionModeButton sessionId={sessionId} />}
        />
      </div>
    </div>
  );
}
