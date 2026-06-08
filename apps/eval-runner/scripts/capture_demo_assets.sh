#!/usr/bin/env bash
# capture_demo_assets.sh — 重新生成 examples/ 目录下的示例 JSON 文件
#
# 用法：
#   cd apps/eval-runner
#   bash scripts/capture_demo_assets.sh
#
# 前置条件：
#   服务已在 localhost:8000 运行（uv run uvicorn app.main:app --port 8000）

set -euo pipefail

BASE_URL="${FORGE_EVAL_BASE_URL:-http://localhost:8000}"
EXAMPLES_DIR="$(cd "$(dirname "$0")/.." && pwd)/examples"

echo "=== forge-eval-runner: capture demo assets ==="
echo "Base URL: $BASE_URL"
echo "Output:   $EXAMPLES_DIR"
echo ""

# 1. Health check
echo "[1/6] Health check..."
HEALTH=$(curl -sf "$BASE_URL/health")
echo "  $HEALTH"

# 2. 保存任务列表
echo "[2/6] Fetching tasks..."
curl -sf "$BASE_URL/tasks" | python3 -m json.tool > "$EXAMPLES_DIR/sample-tasks.json"
echo "  -> sample-tasks.json"

# 3. 创建 run（2 个任务）
echo "[3/6] Creating run (2 tasks)..."
RUN_RESPONSE_TMP="$(mktemp)"
curl -sf -X POST "$BASE_URL/runs" \
  -H "Content-Type: application/json" \
  -d '{
    "task_ids": ["python-cli-dry-run", "parser-regression-failure"],
    "provider": "mock",
    "model": "deterministic-agent-v1"
  }' > "$RUN_RESPONSE_TMP"
python3 -m json.tool "$RUN_RESPONSE_TMP" > "$EXAMPLES_DIR/sample-run-response.json"
rm -f "$RUN_RESPONSE_TMP"

# 提取 run_id
RUN_ID=$(python3 -c "import json,sys; print(json.load(open('$EXAMPLES_DIR/sample-run-response.json'))['run_id'])")
echo "  -> sample-run-response.json (run_id: $RUN_ID)"

# 4. 保存请求体
echo "[4/6] Writing request example..."
cat > "$EXAMPLES_DIR/sample-run-request.json" << 'EOF'
{
  "task_ids": ["python-cli-dry-run", "parser-regression-failure"],
  "provider": "mock",
  "model": "deterministic-agent-v1"
}
EOF
echo "  -> sample-run-request.json"

# 5. 获取 trace
echo "[5/6] Fetching trace..."
curl -sf "$BASE_URL/runs/$RUN_ID/trace" \
  | python3 -m json.tool > "$EXAMPLES_DIR/sample-trace-response.json"
echo "  -> sample-trace-response.json"

# 6. 获取 metrics
echo "[6/6] Fetching metrics..."
curl -sf "$BASE_URL/runs/$RUN_ID/metrics" \
  | python3 -m json.tool > "$EXAMPLES_DIR/sample-metrics-response.json"
echo "  -> sample-metrics-response.json"

echo ""
echo "=== Done ==="
echo ""
echo "Generated files:"
ls -lh "$EXAMPLES_DIR"/*.json
echo ""
echo "Next steps:"
echo "  1. 浏览器打开 $BASE_URL/docs 截图 OpenAPI 文档"
echo "  2. 终端截图 curl $BASE_URL/tasks 输出"
echo "  3. 终端截图 curl $BASE_URL/runs/$RUN_ID/metrics 输出"
echo "  4. 格式化 trace: curl $BASE_URL/runs/$RUN_ID/trace | python3 -m json.tool"
