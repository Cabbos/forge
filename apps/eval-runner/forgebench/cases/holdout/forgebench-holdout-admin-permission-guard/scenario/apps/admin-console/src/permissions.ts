export type AdminRole = "viewer" | "operator" | "admin";

export function canBulkEdit(_role: AdminRole): boolean {
  return true;
}
