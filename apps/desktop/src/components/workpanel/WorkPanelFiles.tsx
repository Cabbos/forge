import { useEffect, useState } from "react";
import { Eye, FileText, RefreshCw } from "lucide-react";
import { FilePreviewActions } from "@/components/messages/FilePreviewActions";
import { FilePreviewBody } from "@/components/messages/FilePreviewBody";
import { deriveFilePreviewView } from "@/components/messages/filePreviewPresentation";
import { ForgeButton } from "@/components/primitives/button";
import { usePreviewFileQuery } from "@/hooks/queries/usePreviewFileQuery";
import { useActiveWorkspace, useStore } from "@/store";
import { createPreviewFileTab } from "./workPanelSelectors";
import type { WorkPanelTab } from "./workPanelTypes";

type FileTab = Extract<WorkPanelTab, { kind: "file" }>;

export function WorkPanelFiles({
  tab,
  onOpenTab,
}: {
  tab: FileTab;
  onOpenTab: (tab: WorkPanelTab) => void;
}) {
  return (
    <WorkPanelFileDocument
      path={tab.path}
      onOpenPreview={() => onOpenTab(createPreviewFileTab(tab.path))}
    />
  );
}

export function WorkPanelFileDocument({
  path,
  onOpenPreview,
}: {
  path: string;
  onOpenPreview?: () => void;
}) {
  const [actionError, setActionError] = useState<string | null>(null);
  const activeSessionId = useStore((state) => state.activeSessionId);
  const activeSession = useStore((state) => activeSessionId ? state.sessions.get(activeSessionId) ?? null : null);
  const activeWorkspace = useActiveWorkspace();
  const workingDir = activeSession?.workingDir ?? activeWorkspace?.path ?? null;
  const previewQuery = usePreviewFileQuery(path, undefined, activeSessionId ?? undefined, workingDir);
  const view = deriveFilePreviewView({ fileRef: { path }, preview: previewQuery.data ?? null });
  const queryError = previewQuery.error ? String(previewQuery.error) : null;

  useEffect(() => {
    setActionError(null);
  }, [path]);

  return (
    <section className="forge-work-panel-file-view" data-testid="work-panel-file-view" aria-label={`文件 ${view.title}`}>
      <header className="forge-work-panel-content-toolbar">
        <div className="forge-work-panel-content-title">
          <FileText className="size-4" />
          <span title={view.title}>{view.title}</span>
          <small className="forge-work-panel-file-path">{view.locationLabel}</small>
        </div>
        {onOpenPreview ? (
          <ForgeButton variant="ghost" size="sm" onClick={onOpenPreview}>
            <Eye className="size-3.5" />
            在预览中打开
          </ForgeButton>
        ) : null}
      </header>
      {actionError ? <div className="forge-work-panel-inline-error" role="alert">{actionError}</div> : null}
      <div className="forge-work-panel-file-body">
        {queryError ? (
          <div className="forge-work-panel-unavailable" role="alert">
            <div>
              <strong>无法预览这个文件</strong>
              <span>{queryError}</span>
            </div>
            <ForgeButton variant="ghost" size="sm" onClick={() => void previewQuery.refetch()} aria-label="重试读取文件">
              <RefreshCw className="size-3.5" />
              重试
            </ForgeButton>
          </div>
        ) : (
          <FilePreviewBody
            loading={previewQuery.isPending}
            error={null}
            lines={view.lines}
          />
        )}
      </div>
      <FilePreviewActions
        copyText={view.copyText}
        fileRef={{ path }}
        sessionId={activeSessionId ?? undefined}
        workingDir={workingDir}
        onError={setActionError}
      />
    </section>
  );
}
