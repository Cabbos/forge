# Public Beta Merge-Train Evidence

This ledger consumes owner-produced evidence without reimplementing Desktop Safety or Eval Trustworthiness behavior. It does not claim that the feature branch is integrated into `main`, and it does not advance the release beyond R2.

## R1 Desktop Safety handoff

- Producer commit: `0a3d758d64c50b485b357c16d8eb221ffe193a31`
- Release profile: `public-beta-r3-v1`
- Machine-readable handoff: `release/evidence/merge-train/desktop-safety.json`
- Raw result digest: `d603e9cbe6d6ddfb02f2dfabf564d2ce9bbc795a381df53303a439aeefe6e8bb`
- Result: five selected gates, five executed gates, zero execution failures, zero failed conditions, and zero unknown conditions.

The deterministic-signal gate owns the unified-memory ID assertion, the Tailwind 4 warning-free production build check, and the console-clean continuity fixture. These signals are not duplicated under another owner slice.

## R2 Eval Trustworthiness handoff

- Producer commit: `0a3d758d64c50b485b357c16d8eb221ffe193a31`
- Release profile: `public-beta-r3-v1`
- Machine-readable handoff: `release/evidence/merge-train/eval-trustworthiness.json`
- Trust-gate result digest: `312e5aea7a37e4ed8ad500b4c5bec81c667ed3ca3cf32c33fb36e563e37d9202`
- Full-quality result digest: `528452fab22f97866f21d151b4c298ec1a2cc2b9c02370aed030974f4fad7191`
- Result: four selected trust gates and the full Eval quality gate completed with zero failed or unknown conditions; the full suite included 247 tests plus Ruff check, Ruff format check, and mypy.

The R2 handoff binds strict provider identity, independent workspace observation, trusted orchestration, authenticated/fenced worker behavior, and the full quality suite to the producing commit.

## Contract enforcement

`scripts/validate-release-gate-profile.mjs` rejects a handoff when a required owner label is missing, a selected/executed count differs, a condition is failed or unknown, a result digest is malformed, an evidence artifact is unreferenced, or the producer commit differs from the expected commit.

The current branch remains pre-R3. Candidate generation is allowed only after the integrated commit is reachable from `main`, all R3 profile gates pass, representative mock and trusted real-Forge evidence exists, and the GitNexus record is attached.
