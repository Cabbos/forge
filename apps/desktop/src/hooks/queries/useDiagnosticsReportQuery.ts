import { useQuery } from "@tanstack/react-query";
import { getDiagnosticsReport, type DiagnosticsReport } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useDiagnosticsReportQuery(enabled = true) {
  return useQuery<DiagnosticsReport>({
    queryKey: queryKeys.diagnosticsReport,
    queryFn: async () => {
      return await getDiagnosticsReport();
    },
    enabled,
    staleTime: 30_000, // 30s before refetch is allowed
  });
}
