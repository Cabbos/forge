import { useEffect, useState } from "react";
import { FileText } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { previewFile, type FilePreview } from "@/lib/tauri";
import { useStore } from "@/store";
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
  const workingDir = useStore((s) => sessionId ? s.sessions.get(sessionId)?.workingDir ?? null : null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!fileRef) return;

    let cancelled = false;
    setLoading(true);
    setError(null);
    setPreview(null);

    previewFile(fileRef.path, fileRef.line, sessionId, workingDir)
      .then((result) => {
        if (!cancelled) setPreview(result);
      })
      .catch((err) => {
        if (!cancelled) setError(String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [fileRef, sessionId, workingDir]);

  const view = deriveFilePreviewView({ fileRef, preview });

  return (
    <Dialog open={Boolean(fileRef)} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent
        className="!h-[min(820px,calc(100vh-48px))] !w-[min(1120px,calc(100vw-48px))] !max-w-[min(1120px,calc(100vw-48px))] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0"
        showCloseButton
      >
        <DialogHeader className="border-b border-border p-4 pr-12">
          <DialogTitle className="flex items-center gap-2 min-w-0">
            <FileText className="size-4 text-primary shrink-0" />
            <span className="truncate font-mono text-sm">{view.title}</span>
          </DialogTitle>
          <DialogDescription>{view.locationLabel}</DialogDescription>
        </DialogHeader>

        <div className="min-h-0 flex-1 overflow-auto bg-background">
          <FilePreviewBody loading={loading} error={error} lines={view.lines} />
        </div>

        <FilePreviewActions
          copyText={view.copyText}
          fileRef={fileRef}
          sessionId={sessionId}
          workingDir={workingDir}
          onError={(message) => setError(message)}
        />
      </DialogContent>
    </Dialog>
  );
}
