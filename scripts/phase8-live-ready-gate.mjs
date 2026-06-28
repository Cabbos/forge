export const PHASE8_LIVE_READY_GATE_COMMAND = "node scripts/phase8-disposable-loop-status.mjs --json --require-live-ready";

export function evaluatePhase8LiveReadyGate(result, { command = PHASE8_LIVE_READY_GATE_COMMAND } = {}) {
  const uiEvidenceStatus = result.uiEvidencePreflight?.status ?? "unknown";
  const checkedUiEvidencePreflight = result.uiEvidencePreflight?.canCollectLiveUiEvidence !== null
    && result.uiEvidencePreflight?.canCollectLiveUiEvidence !== undefined;
  const readyForLiveRun = Boolean(result.readyForLiveRun);
  const base = {
    pass: false,
    reason: "project_not_ready",
    readyForLiveRun,
    requiresCheckedUiPreflight: true,
    checkedUiEvidencePreflight,
    uiEvidenceStatus,
    command,
  };

  if (result.status === "complete") {
    return { ...base, pass: true, reason: "complete" };
  }
  if (result.status === "project_not_ready") {
    return { ...base, reason: "project_not_ready" };
  }
  if (!checkedUiEvidencePreflight) {
    return { ...base, reason: "ui_evidence_not_checked" };
  }
  if (result.uiEvidencePreflight?.canCollectLiveUiEvidence !== true) {
    return { ...base, reason: "ui_evidence_not_ready" };
  }
  if (!readyForLiveRun) {
    return { ...base, reason: "project_not_ready" };
  }
  return { ...base, pass: true, reason: "ready" };
}
