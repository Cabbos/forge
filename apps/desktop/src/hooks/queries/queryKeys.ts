export const queryKeys = {
  apiKeyStatus: ["api-key-status"] as const,
  projectRuntimeStatus: (sessionId: string | undefined | null, workingDir: string | null | undefined) =>
    ["project-runtime-status", sessionId ?? "", workingDir ?? ""] as const,
  projectCheckpointStatus: (sessionId: string | undefined | null, workingDir: string | null | undefined) =>
    ["project-checkpoint-status", sessionId ?? "", workingDir ?? ""] as const,
  capabilities: ["capabilities"] as const,
  mcpContextSources: (sessionId: string | undefined | null) =>
    ["mcp-context-sources", sessionId ?? ""] as const,
  continuityExperiences: (
    sessionId: string | undefined | null,
    projectPath: string | null | undefined,
    search?: string,
  ) => ["continuity-experiences", sessionId ?? "", projectPath ?? "", search ?? ""] as const,
  continuityExperiencesAll: ["continuity-experiences"] as const,
};
