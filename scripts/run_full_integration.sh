#!/usr/bin/env bash
# run_full_integration.sh — Run the full Rust integration test suite (except real-API tests)
# with visibility into which test file is currently running.
#
# Each test binary is printed before execution so maintainers can identify
# which suite is currently executing. This prevents the "waiting with no output"
# problem when a test suite takes longer than expected.
#
# Usage:
#   bash scripts/run_full_integration.sh
#
# Note: This script does NOT set --test-threads=1 for individual test files
# because they are already run serially (one cargo invocation per file).
# Some individual test suites (pipeline_test, document_test, etc.) require
# --test-threads=1 to avoid shared-state contention.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_DIR="$REPO_ROOT/shuji-app/src-tauri"

cd "$TAURI_DIR"

echo "=============================================="
echo "  Full Integration Suite (no real API)"
echo "  Started: $(date)"
echo "=============================================="
echo ""

# ── Test files that MUST run with --test-threads=1 ──
# (shared state: _counter, filesystem, or global config)
SERIAL_TESTS=(
  "pipeline_test"
  "document_test"
  "dispatch_gate_test"
  "path_security_test"
  "command_security_test"
  "session_control_test"
  "session_test"
  "send_message_routing_test"
  "workflow_demo_test"
  "workflow_mock_test"
  "editor_test"
  "config_test"
  "pattern_guard_test"
  "tool_test"
  "validate_test"
)

# ── Test files safe for parallel execution ──
PARALLEL_TESTS=(
  "actor_test"
  "audit_test"
  "checkpoint_test"
  "learning_test"
  "scenario_replay_test"
  "watchdog_behavior_test"
)

echo "--- Serial tests (--test-threads=1) ---"
for test_name in "${SERIAL_TESTS[@]}"; do
  echo ""
  echo ">>> $(date)  Running: ${test_name} (serial)"
  cargo test --test "${test_name}" -- --test-threads=1 2>&1
  echo "<<< $(date)  Completed: ${test_name}"
  echo ""
done

echo "--- Parallel-safe tests ---"
for test_name in "${PARALLEL_TESTS[@]}"; do
  echo ""
  echo ">>> $(date)  Running: ${test_name} (parallel)"
  cargo test --test "${test_name}" 2>&1
  echo "<<< $(date)  Completed: ${test_name}"
  echo ""
done

echo "=============================================="
echo "  Full Integration Suite Complete"
echo "  Finished: $(date)"
echo "=============================================="
