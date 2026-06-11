# Worktree Worker Phase 5 Acceptance

Date: 2026-06-11
Branch: `cabbos/internal-a2a-runtime-plan`

## Purpose

Phase 5 verifies that `delegate_task(mode = "worktree_worker")` is no longer just an internal unit-test path. The acceptance target is a repeatable two-layer gate:

1. Deterministic CI coverage for worktree creation, tool execution, diff collection, test-report extraction, and review-gate decisions.
2. A manually triggered real-model smoke command that is safe to run outside CI and has clear pass/fail criteria.

## CI Gate

Run these from the repository root:

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml agent::a2a --lib
npm --prefix apps/desktop run check:backend
npm --prefix apps/desktop run build
npm --prefix apps/desktop run eval:forge:test
npm run eval:report:latest -- --failures
npm run eval:forge:smoke:real -- --dry-run
```

The CI-level gate passes when:

- `agent::a2a` tests pass, including the WorktreeWorker e2e and review-gate unit tests.
- Backend format, clippy, and Rust tests pass.
- Frontend build passes.
- Forge eval tests pass.
- Latest eval report has no quality-gate regression.
- The real smoke dry-run prints the expected `forge_eval_agent` command and budget.

## Real-Model Smoke

Run only when a valid provider key is configured and the operator is ready for a live model call:

```bash
npm run eval:forge:smoke:real
```

The smoke passes when the resulting eval artifact shows:

- A real Forge provider run completed within the configured budget.
- The parent model invoked `delegate_task` with `mode = "worktree_worker"` or the run explicitly records that the model did not choose the mode.
- A WorktreeWorker summary includes `diff_available`, `diff_truncated`, `tests_passed`, `needs_human_review`, `suggested_action`, `reason_codes`, `worktree_path`, and `cleaned_up`.
- `needs_human_review` is true for any WorktreeWorker output that represents a completed or blocked worker attempt.
- `suggested_action` contains `HUMAN REVIEW REQUIRED` and never tells the parent to merge automatically.
- Failed tests, truncated diffs, unparseable test output, sub-agent errors, duplicate active worktrees, and missing diffs produce explicit review reasons.

## Review-Gate Contract

`WorktreeWorker` produces artifacts, not merged code. The parent agent must treat the result as review material:

- Passing tests plus a diff still requires human review.
- Failed tests preserve the worktree for inspection.
- Truncated diffs preserve the worktree for inspection.
- Sub-agent errors preserve the worktree for inspection.
- Duplicate active worktrees return an explicit `already_in_use` reason and a human-review action.
- Missing diffs are surfaced as a review reason so the operator can confirm whether the worker actually changed anything.

## Known Gaps

- Real-model smoke is manual because model choice is nondeterministic and requires credentials.
- Non-git workspaces still fall back to patch/manual guidance instead of a temporary-copy worker.
- Resume normalizes interrupted worker tasks but does not restart them automatically.
- Multi-worker supervisor review and merge recommendation remain Phase 6+ work.
