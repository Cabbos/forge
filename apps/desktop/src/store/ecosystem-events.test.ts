import { describe, it } from "node:test";
import assert from "node:assert";
import { invalidateEcosystemQueries } from "./ecosystem-events.ts";

describe("invalidateEcosystemQueries", () => {
  it("invalidates all ecosystem query surfaces", () => {
    const invalidated: unknown[][] = [];
    const queryClient = {
      invalidateQueries(options: { queryKey: readonly unknown[] }) {
        invalidated.push(Array.from(options.queryKey));
      },
    };

    invalidateEcosystemQueries(queryClient);

    assert.deepStrictEqual(invalidated, [
      ["capabilities"],
      ["ecosystem-items"],
      ["tool-inventory"],
    ]);
  });
});
