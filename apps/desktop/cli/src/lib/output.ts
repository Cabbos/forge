export function renderJson(payload: unknown): string {
  return `${JSON.stringify(payload, null, 2)}\n`;
}

export function renderRunSummary(payload: Record<string, unknown>): string {
  return [
    `Provider: ${stringValue(payload.provider) ?? "unknown"}`,
    `Model: ${stringValue(payload.model) ?? "unknown"}`,
    `Changed files: ${changedFileCount(payload) ?? "unknown"}`,
    `Validation: ${validationStatus(payload) ?? "unknown"}`,
    `Final answer: ${stringValue(payload.final_answer) ?? stringValue(payload.finalAnswer) ?? ""}`,
  ].join("\n") + "\n";
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function changedFileCount(payload: Record<string, unknown>): number | undefined {
  if (typeof payload.changed_file_count === "number") {
    return payload.changed_file_count;
  }
  if (Array.isArray(payload.changed_files)) {
    return payload.changed_files.length;
  }
  return undefined;
}

function validationStatus(payload: Record<string, unknown>): string | undefined {
  if (typeof payload.validation === "string") {
    return payload.validation;
  }
  if (typeof payload.validation_passed === "boolean") {
    return payload.validation_passed ? "passed" : "failed";
  }
  if (
    isRecord(payload.verification_result) &&
    typeof payload.verification_result.passed === "boolean"
  ) {
    return payload.verification_result.passed ? "passed" : "failed";
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
