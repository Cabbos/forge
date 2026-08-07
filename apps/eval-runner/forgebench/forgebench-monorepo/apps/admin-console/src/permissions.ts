export type AdminRole = "viewer" | "operator" | "admin";

export function canBulkEdit(role: AdminRole): boolean {
  return role === "operator" || role === "admin";
}
