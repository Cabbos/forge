import { expect, test } from "bun:test";
import {
  deserializeSavedFilters,
  serializeSavedFilters,
} from "../../apps/web-console/src/saved-filters";

test("saved filters serialize in deterministic name order", () => {
  const serialized = serializeSavedFilters([
    { name: " Zed ", query: { status: "done" } },
    { name: "Alpha", query: { search: "release" } },
  ]);
  expect(serialized).toBe(
    '[{"name":"Alpha","query":{"search":"release"}},{"name":"Zed","query":{"status":"done"}}]',
  );
  expect(deserializeSavedFilters(serialized)[1].name).toBe("Zed");
});
