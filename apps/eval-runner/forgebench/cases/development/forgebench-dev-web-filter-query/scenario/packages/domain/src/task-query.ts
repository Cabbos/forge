import type { Task, TaskStatus } from "./task";

export interface TaskQuery {
  search?: string;
  status?: TaskStatus;
}

export function parseTaskQuery(input: string): TaskQuery {
  const query: TaskQuery = {};
  const search: string[] = [];
  for (const token of input.trim().split(/\s+/u)) {
    if (token.startsWith("status:")) {
      query.status = token.slice("status:".length) as TaskStatus;
    } else if (token) {
      search.push(token);
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
  return !query.search || task.title.toLowerCase().includes(query.search.toLowerCase());
}
