import { useCallback, useEffect, useState } from "react";
import { FileText } from "lucide-react";
import {
  ForgeDialog,
  ForgeDialogContent,
  ForgeDialogDescription,
  ForgeDialogHeader,
  ForgeDialogTitle,
} from "@/components/primitives/dialog";
// FilePreview type used via usePreviewFileQuery return type inference
import { useStore } from "@/store";
import { usePreviewFileQuery } from "@/hooks/queries/usePreviewFileQuery";
import { getQueryErrorMessage } from "@/hooks/queries/queryErrors";
import { FilePreviewActions } from "@/components/messages/FilePreviewActions";
import { FilePreviewBody } from "@/components/messages/FilePreviewBody";
import { deriveFilePreviewView } from "@/components/messages/filePreviewPresentation";
import type { FileRef } from "@/components/messages/filePreviewTypes";

interface FilePreviewSheetProps {
  fileRef: FileRef | null;
  onClose: () => void;
  sessionId?: string;
}

export function FilePreviewSheet({ fileRef, onClose, sessionId }: FilePreviewSheetProps) {
  const selectWorkingDir = useCallback(
    (s: ReturnType<typeof useStore.getState>) => sessionId ? s.sessions.get(sessionId)?.workingDir ?? null : null,
    [sessionId],
  );
  const workingDir = useStore(selectWorkingDir);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    setActionError(null);
  }, [fileRef?.path, fileRef?.line]);

  const {
    data: preview,
    isLoading: loading,
    isError,
    error: queryError,
  } = usePreviewFileQuery(
    fileRef?.path,
    fileRef?.line,
    sessionId,
    workingDir,
    !!fileRef,
  );

  const queryErrorDisplay = isError ? getQueryErrorMessage(queryError) : null;
  const error = queryErrorDisplay || actionError;
  const view = deriveFilePreviewView({ fileRef, preview: preview ?? null });

  return (
    <ForgeDialog open={Boolean(fileRef)} onOpenChange={(open) => { if (!open) onClose(); }}>
      <ForgeDialogContent
        className="!h-[min(820px,calc(100vh-48px))] !w-[min(1120px,calc(100vw-48px))] !max-w-[min(1120px,calc(100vw-48px))] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0"
        showCloseButton
      >
        <ForgeDialogHeader className="border-b border-border p-4 pr-12">
          <ForgeDialogTitle className="flex items-center gap-2 min-w-0">
            <FileText className="size-4 text-primary shrink-0" />
            <span className="truncate font-mono text-sm">{view.title}</span>
          </ForgeDialogTitle>
          <ForgeDialogDescription>{view.locationLabel}</ForgeDialogDescription>
        </ForgeDialogHeader>

        <div className="min-h-0 flex-1 overflow-auto bg-background">
          <FilePreviewBody loading={loading} error={error} lines={view.lines} />
        </div>

        <FilePreviewActions
          copyText={view.copyText}
          fileRef={fileRef}
          sessionId={sessionId}
          workingDir={workingDir}
          onError={setActionError}
        />
      </ForgeDialogContent>
    </ForgeDialog>
  );
}
