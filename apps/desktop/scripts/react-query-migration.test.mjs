import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

test("Query keys centralised and new query hooks exist", () => {
  assert.equal(existsSync(join(root, "src/hooks/queries/queryKeys.ts")), true);

  assert.equal(existsSync(join(root, "src/hooks/queries/useProjectRuntimeStatusQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useProjectCheckpointStatusQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useCapabilitiesQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useMcpContextSourcesQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useContinuityExperiencesQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useSearchWorkspaceFilesQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/usePreviewFileQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useForgeWikiStateQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useAppMetadataQuery.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useSessionsQuery.ts")), true);

  const qk = read("src/hooks/queries/queryKeys.ts");
  assert.match(qk, /apiKeyStatus/);
  assert.match(qk, /projectRuntimeStatus/);
  assert.match(qk, /projectCheckpointStatus/);
  assert.match(qk, /capabilities/);
  assert.match(qk, /mcpContextSources/);
  assert.match(qk, /continuityExperiences/);
  assert.match(qk, /searchWorkspaceFiles/);
  assert.match(qk, /previewFile/);
  assert.match(qk, /forgeWikiState/);
  assert.match(qk, /appMetadata/);
  assert.match(qk, /sessions/);
});

test("Project status queries use Query and query keys", () => {
  const runtimeHook = read("src/hooks/queries/useProjectRuntimeStatusQuery.ts");
  assert.match(runtimeHook, /useQuery\s*[<\(]/);
  assert.match(runtimeHook, /queryKeys\.projectRuntimeStatus/);
  assert.match(runtimeHook, /getProjectRuntimeStatus/);

  const checkpointHook = read("src/hooks/queries/useProjectCheckpointStatusQuery.ts");
  assert.match(checkpointHook, /useQuery\s*[<\(]/);
  assert.match(checkpointHook, /queryKeys\.projectCheckpointStatus/);
  assert.match(checkpointHook, /getProjectCheckpointStatus/);
});

test("Capabilities query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useCapabilitiesQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.capabilities/);
  assert.match(hook, /listCapabilities/);
});

test("MCP context sources query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useMcpContextSourcesQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.mcpContextSources/);
  assert.match(hook, /listMcpContextSources/);
});

test("Continuity experiences query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useContinuityExperiencesQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.continuityExperiences/);
  assert.match(hook, /listContinuityExperiences/);
});

test("Search workspace files query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useSearchWorkspaceFilesQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.searchWorkspaceFiles/);
  assert.match(hook, /searchWorkspaceFiles/);
});

test("Preview file query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/usePreviewFileQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.previewFile/);
  assert.match(hook, /previewFile/);
});

test("Forge wiki state query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useForgeWikiStateQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.forgeWikiState/);
  assert.match(hook, /getForgeWikiState/);
});

test("App metadata query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useAppMetadataQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.appMetadata/);
  assert.match(hook, /loadAppMetadata/);
});

test("Sessions query uses Query and query keys", () => {
  const hook = read("src/hooks/queries/useSessionsQuery.ts");
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /queryKeys\.sessions/);
  assert.match(hook, /listSessions/);
});

test("Composer file suggestions keep empty @ query enabled", () => {
  const suggestions = read("src/components/session/useComposerSuggestions.ts");
  assert.doesNotMatch(suggestions, /showSuggestions === ["']@["'] && searchTerm\.length > 0/);
  assert.match(suggestions, /showSuggestions === ["']@["']/);

  const hook = read("src/hooks/queries/useSearchWorkspaceFilesQuery.ts");
  assert.doesNotMatch(hook, /enabled:\s*enabled\s*&&\s*query\.trim\(\)\.length > 0/);
  assert.match(hook, /enabled,|enabled:\s*enabled/);
});

test("File preview clears action errors when switching targets", () => {
  const sheet = read("src/components/messages/FilePreviewSheet.tsx");
  assert.match(sheet, /import\s+\{[^}]*useEffect[^}]*\}\s+from\s+["']react["']/);
  assert.match(sheet, /setActionError\(null\)/);
  assert.match(sheet, /\[fileRef\?\.path,\s*fileRef\?\.line\]/);
});

test("Migrated components no longer directly call IPC read functions", () => {
  const startReadiness = read("src/components/session/StartReadinessCard.tsx");
  assert.doesNotMatch(startReadiness, /getProjectRuntimeStatus\(/);
  assert.doesNotMatch(startReadiness, /getProjectCheckpointStatus\(/);

  const projectStatus = read("src/components/layout/ProjectStatusCard.tsx");
  assert.doesNotMatch(projectStatus, /getProjectRuntimeStatus\(/);
  assert.doesNotMatch(projectStatus, /getProjectCheckpointStatus\(/);

  const capabilityManager = read("src/components/settings/CapabilityManager.tsx");
  assert.doesNotMatch(capabilityManager, /listCapabilities\(/);

  const hubPanel = read("src/components/layout/useHubPanelData.ts");
  assert.doesNotMatch(hubPanel, /listMcpContextSources\(/);
  assert.doesNotMatch(hubPanel, /getProjectRuntimeStatus\(/);

  const continuity = read("src/components/context/ContinuityExperiencesSection.tsx");
  assert.doesNotMatch(continuity, /listContinuityExperiences\(/);
  assert.doesNotMatch(continuity, /searchContinuityExperiences\(/);

  const composerSuggestions = read("src/components/session/useComposerSuggestions.ts");
  assert.doesNotMatch(composerSuggestions, /searchWorkspaceFiles\(/);

  const filePreview = read("src/components/messages/FilePreviewSheet.tsx");
  assert.doesNotMatch(filePreview, /previewFile\(/);

  const wikiSections = read("src/components/context/WikiSections.tsx");
  assert.doesNotMatch(wikiSections, /getForgeWikiState\(/);

  const hydration = read("src/store/hydration.ts");
  assert.doesNotMatch(hydration, /await\s+loadAppMetadata\(/);
  assert.doesNotMatch(hydration, /loadAppMetadata\(\)\.catch/);
  assert.doesNotMatch(hydration, /await\s+listSessions\(/);
  assert.doesNotMatch(hydration, /listSessions\(\)\.catch/);
});

test("Components use useQueryClient instead of importing global queryClient", () => {
  const migratedComponents = [
    "src/components/session/StartReadinessCard.tsx",
    "src/components/layout/ProjectStatusCard.tsx",
    "src/components/settings/CapabilityManager.tsx",
    "src/components/context/ContinuityExperiencesSection.tsx",
    "src/components/settings/useSettingsDialogController.ts",
  ];
  for (const path of migratedComponents) {
    const content = read(path);
    assert.doesNotMatch(content, /import\s+.*queryClient\s+from\s*["']@\/lib\/query-client["']/);
    assert.match(content, /import\s+.*useQueryClient.*from\s*["']@tanstack\/react-query["']/);
  }
});

test("No hardcoded queryKey arrays in invalidateQueries", () => {
  const filesWithInvalidation = [
    "src/components/session/StartReadinessCard.tsx",
    "src/components/layout/ProjectStatusCard.tsx",
    "src/components/settings/CapabilityManager.tsx",
    "src/components/context/ContinuityExperiencesSection.tsx",
    "src/components/settings/useSettingsDialogController.ts",
  ];
  for (const path of filesWithInvalidation) {
    const content = read(path);
    assert.doesNotMatch(content, /invalidateQueries\(\{\s*queryKey:\s*\[/);
  }
});

test("Query hooks do not swallow errors into empty arrays or null", () => {
  const hooks = [
    "src/hooks/queries/useProjectRuntimeStatusQuery.ts",
    "src/hooks/queries/useProjectCheckpointStatusQuery.ts",
    "src/hooks/queries/useCapabilitiesQuery.ts",
    "src/hooks/queries/useMcpContextSourcesQuery.ts",
    "src/hooks/queries/useContinuityExperiencesQuery.ts",
    "src/hooks/queries/useSearchWorkspaceFilesQuery.ts",
    "src/hooks/queries/usePreviewFileQuery.ts",
    "src/hooks/queries/useForgeWikiStateQuery.ts",
    "src/hooks/queries/useAppMetadataQuery.ts",
    "src/hooks/queries/useSessionsQuery.ts",
  ];
  for (const path of hooks) {
    const content = read(path);
    assert.doesNotMatch(content, /catch\s*\{[^}]*return\s*(\[\]|null)/s);
  }
});

test("Migrated query consumers surface query errors", () => {
  const consumers = [
    "src/components/session/StartReadinessCard.tsx",
    "src/components/layout/ProjectStatusCard.tsx",
    "src/components/settings/CapabilityManager.tsx",
    "src/components/context/ContinuityExperiencesSection.tsx",
    "src/components/settings/useSettingsDialogController.ts",
    "src/components/messages/FilePreviewSheet.tsx",
    "src/components/context/WikiSections.tsx",
  ];
  for (const path of consumers) {
    const content = read(path);
    assert.match(content, /isError/);
    assert.match(content, /error:/);
    assert.match(content, /getQueryErrorMessage/);
  }
});

test("Settings API key query pilot still intact", () => {
  const controller = read("src/components/settings/useSettingsDialogController.ts");
  assert.doesNotMatch(controller, /getApiKeyStatus\(/);
  assert.match(controller, /useApiKeyStatusQuery/);
});

test("Session mutations invalidate queryKeys.sessions", () => {
  const useSession = read("src/hooks/useSession.ts");
  const sessionInvalidations = useSession.match(/queryClient\.invalidateQueries\(\{\s*queryKey:\s*queryKeys\.sessions\s*\}\)/g) ?? [];
  assert.equal(sessionInvalidations.length, 3);
  assert.match(useSession, /const resume = useCallback\([\s\S]*?resumeSession\(sessionId\)[\s\S]*?queryClient\.invalidateQueries\(\{\s*queryKey:\s*queryKeys\.sessions\s*\}\)/);
  assert.doesNotMatch(useSession, /invalidateQueries\(\{\s*queryKey:\s*\[/);

  const settingsController = read("src/components/settings/useSettingsDialogController.ts");
  assert.match(settingsController, /queryClient\.invalidateQueries\(\{\s*queryKey:\s*queryKeys\.sessions\s*\}\)/);
});
