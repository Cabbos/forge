import { expect, test } from "bun:test";
import { matchesTaskQuery, parseTaskQuery, type Task } from "../../packages/domain/src";

test("quoted text remains one web-filter search value", () => {
  const task: Task = {
    id: "task-1",
    title: "Ship release",
    status: "todo",
    priority: "high",
    revision: 1,
  };
  const query = parseTaskQuery('status:todo "ship release"');
  expect(query).toEqual({ status: "todo", search: "ship release" });
  expect(matchesTaskQuery(task, query)).toBe(true);
});
