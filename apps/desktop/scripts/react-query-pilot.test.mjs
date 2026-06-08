import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const root = new URL("..", import.meta.url).pathname;

function read(path) {
  return readFileSync(join(root, path), "utf8");
}

test("TanStack Query pilot wiring exists for API key status", () => {
  assert.equal(existsSync(join(root, "src/lib/query-client.ts")), true);
  assert.equal(existsSync(join(root, "src/hooks/queries/useApiKeyStatusQuery.ts")), true);

  const queryClient = read("src/lib/query-client.ts");
  assert.match(queryClient, /export const queryClient/);
  assert.match(queryClient, /new QueryClient\(/);

  const hook = read("src/hooks/queries/useApiKeyStatusQuery.ts");
  assert.match(hook, /export function useApiKeyStatusQuery/);
  assert.match(hook, /useQuery\s*[<\(]/);
  assert.match(hook, /getApiKeyStatus/);
  assert.match(hook, /queryKeys\.apiKeyStatus/);

  const main = read("src/main.tsx");
  assert.match(main, /import\s+.*QueryClientProvider.*from\s*["']@tanstack\/react-query["']/);
  assert.match(main, /<QueryClientProvider\s+client=\{queryClient\}/);

  const controller = read("src/components/settings/useSettingsDialogController.ts");
  assert.match(controller, /import\s+.*useApiKeyStatusQuery.*from\s*["']@\/hooks\/queries\/useApiKeyStatusQuery["']/);
  assert.doesNotMatch(controller, /getApiKeyStatus\(\)/);
});
