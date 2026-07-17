import { AlertTriangle, CheckCircle, XCircle } from "lucide-react";

/** Shared status visuals for diagnostics and gateway runtime surfaces. */
export const STATUS_ICON: Record<string, typeof CheckCircle> = {
  pass: CheckCircle,
  warn: AlertTriangle,
  fail: XCircle,
};

export const STATUS_CLASS: Record<string, string> = {
  pass: "text-green-600",
  warn: "text-amber-500",
  fail: "text-red-500",
};

export const STATUS_LABEL: Record<string, string> = {
  pass: "Pass",
  warn: "Warn",
  fail: "Fail",
};
