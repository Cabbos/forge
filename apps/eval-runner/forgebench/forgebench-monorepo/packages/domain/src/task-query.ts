import type { Task, TaskStatus } from "./task";

export interface TaskQuery {
  search?: string;
  status?: TaskStatus;
}

const TOKEN_PATTERN = /(?:[^\s"]+|"[^"]*")+/gu;

export function parseTaskQuery(input: string): TaskQuery {
  const query: TaskQuery = {};
  const search: string[] = [];
  for (const token of input.match(TOKEN_PATTERN) ?? []) {
    const normalized = token.replace(/^"|"$/gu, "");
    if (normalized.startsWith("status:")) {
      query.status = normalized.slice("status:".length) as TaskStatus;
    } else if (normalized) {
      search.push(normalized);
    }
  }
  if (search.length > 0) {
    query.search = search.join(" ");
  }
  return query;
}

export function matchesTaskQuery(task: Task, query: TaskQuery): boolean {
  if (query.status && task.status !== query.status) {
    return false;
  }
  return !query.search || task.title.toLocaleLowerCase().includes(query.search.toLocaleLowerCase());
}
