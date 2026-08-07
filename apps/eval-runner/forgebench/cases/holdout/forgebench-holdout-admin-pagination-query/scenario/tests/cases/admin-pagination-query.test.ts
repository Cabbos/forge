import { expect, test } from "bun:test";
import { pageForQuery } from "../../apps/admin-console/src/bulk-actions";

test("same effective query keeps current page", () => {
  expect(pageForQuery("status:todo", "status:todo", 7)).toBe(7);
  expect(pageForQuery("status:todo", "status:done", 7)).toBe(1);
});
