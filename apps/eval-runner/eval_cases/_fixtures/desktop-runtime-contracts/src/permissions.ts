export type PermissionRule = {
  pattern: string;
  action: "allow" | "deny";
};

export function decidePermission(path: string, rules: PermissionRule[]): "allow" | "deny" {
  const match = rules.find((rule) => path.startsWith(rule.pattern));
  return match?.action ?? "deny";
}
