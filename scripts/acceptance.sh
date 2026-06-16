#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0

for arg in "$@"; do
  case "$arg" in
    --dry-run)
      DRY_RUN=1
      ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/acceptance.sh [--dry-run]

Runs the Forge Phase 7 acceptance gates:
  1. Desktop production build
  2. Website production build
  3. Eval runner test suite
  4. Desktop acceptance e2e smoke specs

Use --dry-run to print the command plan without executing it.
EOF
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

COMMANDS=(
  "npm run build:desktop"
  "npm run build:website"
  "npm run test:eval"
  "npm --prefix apps/desktop run test:e2e -- e2e/resume.spec.ts e2e/workbench.spec.ts e2e/a2a-confirm-runtime.spec.ts e2e/acceptance.spec.ts"
)

echo "Forge Phase 7 acceptance suite"
echo "Working directory: $ROOT_DIR"
echo

for command in "${COMMANDS[@]}"; do
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "[dry-run] $command"
  else
    echo "==> $command"
    (cd "$ROOT_DIR" && eval "$command")
  fi
done
