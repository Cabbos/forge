export const queryKeys = {
  apiKeyStatus: ["api-key-status"] as const,
  providerCatalog: ["provider-catalog"] as const,
  projectRuntimeStatus: (sessionId: string | undefined | null, workingDir: string | null | undefined) =>
    ["project-runtime-status", sessionId ?? "", workingDir ?? ""] as const,
  projectCheckpointStatus: (sessionId: string | undefined | null, workingDir: string | null | undefined) =>
    ["project-checkpoint-status", sessionId ?? "", workingDir ?? ""] as const,
  capabilities: ["capabilities"] as const,
  ecosystemItems: ["ecosystem-items"] as const,
  toolInventory: ["tool-inventory"] as const,
  mcpContextSources: (sessionId: string | undefined | null) =>
    ["mcp-context-sources", sessionId ?? ""] as const,
  continuityExperiences: (
    sessionId: string | undefined | null,
    projectPath: string | null | undefined,
    search?: string,
  ) => ["continuity-experiences", sessionId ?? "", projectPath ?? "", search ?? ""] as const,
  continuityExperiencesAll: ["continuity-experiences"] as const,
  searchWorkspaceFiles: (query: string, sessionId?: string, workingDir?: string | null) =>
    ["search-workspace-files", query, sessionId ?? "", workingDir ?? ""] as const,
  previewFile: (path: string, line?: number, sessionId?: string, workingDir?: string | null) =>
    ["preview-file", path, line ?? 0, sessionId ?? "", workingDir ?? ""] as const,
  forgeWikiState: (projectPath: string, sessionId?: string | null) =>
    ["forge-wiki-state", projectPath, sessionId ?? ""] as const,
  appMetadata: ["app-metadata"] as const,
  sessions: ["sessions"] as const,
  diagnosticsReport: ["diagnostics-report"] as const,
  gatewayRuntimeStatus: ["gateway-runtime-status"] as const,
  gatewayTriggers: ["gateway-triggers"] as const,
  gatewaySessions: ["gateway-sessions"] as const,
  memoryFactsAll: ["memory-facts"] as const,
  memoryFacts: (query?: string, profileId?: string | null) =>
    ["memory-facts", query ?? "", profileId ?? ""] as const,
  profilesAll: ["profiles"] as const,
  schedulerAll: ["scheduler"] as const,
};
