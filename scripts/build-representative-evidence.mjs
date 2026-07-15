import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const SHA40 = /^[0-9a-f]{40}$/i;
const TYPES = new Set(["representative_mock", "representative_real_forge"]);

function digest(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

export function buildRepresentativeEvidence({ sourcePath, source, sourceSha256, evidenceType, commit }) {
  if (!TYPES.has(evidenceType)) throw new Error("evidence type must be representative_mock or representative_real_forge");
  if (!SHA40.test(String(commit ?? ""))) throw new Error("producer commit must be a full 40-character SHA");
  const report = source?.report;
  if (!report || typeof report !== "object") throw new Error("backtest artifact report is required");
  if (!Array.isArray(source.traces) || source.traces.length === 0) throw new Error("backtest artifact traces must be non-empty");
  if (!Number.isInteger(report.total_tasks) || report.total_tasks < 1) throw new Error("backtest report total_tasks must be positive");
  if (source.traces.length !== report.total_tasks) throw new Error("backtest trace count does not match total_tasks");
  if (report.success_rate !== 1 || report.verification_pass_rate !== 1 || report.scope_violation_rate !== 0) {
    throw new Error("representative backtest must pass success, verification, and scope thresholds");
  }
  const expectedProvider = evidenceType === "representative_mock" ? "mock" : "forge";
  if (source.traces.some((trace) => trace.provider !== expectedProvider)) {
    throw new Error(`${evidenceType} traces must use provider ${expectedProvider}`);
  }
  const trustResult = report.trust_result;
  if (!trustResult || trustResult.status !== "trusted" || trustResult.trusted !== true) {
    throw new Error("representative backtest trust_result must be trusted");
  }
  if (Array.isArray(trustResult.blockers) && trustResult.blockers.length > 0) {
    throw new Error("trusted representative backtest cannot contain trust blockers");
  }
  return {
    schema_version: 1,
    evidence_type: evidenceType,
    producer_commit: commit,
    run_id: `${evidenceType}-${sourceSha256.slice(0, 16)}`,
    execution_status: "completed",
    condition_status: "passed",
    trust_result: "trusted",
    source_artifact: {
      path: sourcePath,
      sha256: sourceSha256,
    },
    total_tasks: report.total_tasks,
    success_rate: report.success_rate,
    verification_pass_rate: report.verification_pass_rate,
    scope_violation_rate: report.scope_violation_rate,
  };
}

function option(argv, name) {
  const index = argv.indexOf(name);
  return index >= 0 ? argv[index + 1] : null;
}

function main(argv) {
  const inputPath = option(argv, "--input");
  const outputPath = option(argv, "--output");
  try {
    if (!inputPath || !existsSync(inputPath)) throw new Error("--input must point to an existing JSON file");
    if (!outputPath) throw new Error("--output is required");
    const raw = readFileSync(inputPath);
    const source = JSON.parse(raw.toString("utf8"));
    const evidence = buildRepresentativeEvidence({
      sourcePath: inputPath,
      source,
      sourceSha256: digest(raw),
      evidenceType: option(argv, "--evidence-type"),
      commit: option(argv, "--commit"),
    });
    mkdirSync(dirname(outputPath), { recursive: true });
    const tempPath = `${outputPath}.tmp-${process.pid}`;
    try {
      writeFileSync(tempPath, `${JSON.stringify(evidence, null, 2)}\n`, { flag: "wx" });
      renameSync(tempPath, outputPath);
    } catch (error) {
      if (existsSync(tempPath)) unlinkSync(tempPath);
      throw error;
    }
    console.log(JSON.stringify(evidence, null, 2));
    return 0;
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    return 1;
  }
}

if (process.argv[1] === new URL(import.meta.url).pathname) process.exitCode = main(process.argv.slice(2));
