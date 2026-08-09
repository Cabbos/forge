# ForgeBench golden trust self-check correction — 2026-08-10

The eval runner's deterministic golden self-check previously chose the
synthetic `workspace/changes.patch` target whenever a case had no
`context_files`. ForgeBench keeps its controller-owned output contract in
`expected_files_changed`, so that fallback made every real-coding development
case appear to have an out-of-scope change and caused ordinary Forge reports
to be marked `harness_untrusted`.

The fix makes the mock choose the first declared expected output, and treats a
case marked `expected_no_change` as unchanged unless its mock metadata
explicitly requests a change. It does not relax scope matching, trust gates,
hidden verifiers, or any real-provider permission boundary.

Evidence after the fix:

- `run_golden_harness_check` passes against all 10 canonical development cases;
- deterministic ForgeBench development execution is `1.0000` success,
  `0.0000` scope violations, and `trusted: true`;
- the targeted runner/smoke suite passes 88/88;
- the full eval-runner suite passes 645/645.

This corrects trust-evidence pollution. It is not a new paid Kimi or H4
holdout result; a fresh provider run is still required for current quality
metrics.

## Fresh Kimi trust-path smoke

After the correction, a real Kimi K3 run of the
`forgebench-dev-config-env-parser` case completed with:

- `1/1` task success and verification;
- `0` scope violations;
- `trusted: true` with no trust blockers;
- `8` model rounds and `4` confirmation requests;
- known usage/cost at `$0.032733`.

This single-case smoke validates that a real provider report can pass the
corrected golden trust path. It is still development-only evidence and does
not establish a new paired comparison, H4 holdout result, or frontier claim.
