export interface EmptyStateView {
  title: string;
  detail: string;
}

export function emptyState(title: string, detail: string): EmptyStateView {
  return {
    title,
    detail,
  };
}

export function escapeHtml(value: string): string {
  return value.replace(
    /[&<>"']/gu,
    (character) =>
      ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;",
      })[character] ?? character,
  );
}
