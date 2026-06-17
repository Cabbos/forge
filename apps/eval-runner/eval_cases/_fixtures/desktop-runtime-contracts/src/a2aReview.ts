export type ReviewFinding = {
  severity: "info" | "warning" | "error";
  resolved: boolean;
};

export type ReviewSummary = {
  total: number;
  openErrors: number;
  openWarnings: number;
  resolved: number;
};

export function summarizeReview(findings: ReviewFinding[]): ReviewSummary {
  return {
    total: findings.length,
    openErrors: 0,
    openWarnings: 0,
    resolved: 0
  };
}
