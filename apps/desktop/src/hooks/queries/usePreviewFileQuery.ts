import { useQuery } from "@tanstack/react-query";
import { previewFile, type FilePreview } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function usePreviewFileQuery(
  path: string | undefined,
  line?: number,
  sessionId?: string,
  workingDir?: string | null,
  enabled = true,
) {
  return useQuery<FilePreview>({
    queryKey: queryKeys.previewFile(path ?? "", line, sessionId, workingDir),
    queryFn: async () => {
      return await previewFile(path!, line, sessionId, workingDir);
    },
    enabled: enabled && !!path,
  });
}
