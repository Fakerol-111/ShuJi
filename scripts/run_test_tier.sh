#!/usr/bin/env bash
# run_test_tier.sh — Run a named ShuJi test tier.
#
# Usage:
#   bash scripts/run_test_tier.sh fast
#   bash scripts/run_test_tier.sh core-integration
#   bash scripts/run_test_tier.sh security
#   bash scripts/run_test_tier.sh audit
#   bash scripts/run_test_tier.sh workflow
#   bash scripts/run_test_tier.sh slow
#
# Each command is printed before execution so maintainers can
# identify which suite is currently running.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TAURI_DIR="$REPO_ROOT/shuji-app/src-tauri"
APP_DIR="$REPO_ROOT/shuji-app"

TIER="${1:-}"
if [[ -z "$TIER" ]]; then
  echo "Usage: $0 {fast|core-integration|security|audit|workflow|slow}"
  exit 1
fi

run() {
  echo ""
  echo "==> $*"
  "$@"
}

case "$TIER" in
  fast)
    echo "=== Fast Tier ==="
    cd "$APP_DIR"
    run npm run lint
    run npm test
    run npm run format:check
    cd "$TAURI_DIR"
    run cargo fmt --check
    run cargo test --lib
    run cargo test --test path_security_test -- --test-threads=1
    run cargo test --test document_test -- --test-threads=1
    run cargo test --test pipeline_test -- --test-threads=1
    ;;
  core-integration)
    echo "=== Core Integration Tier ==="
    cd "$TAURI_DIR"
    run cargo test --test pipeline_test -- --test-threads=1
    run cargo test --test document_test -- --test-threads=1
    run cargo test --test dispatch_gate_test -- --test-threads=1
    run cargo test --test session_control_test -- --test-threads=1
    run cargo test --test actor_test -- --test-threads=1
    ;;
  security)
    echo "=== Security Tier ==="
    cd "$TAURI_DIR"
    run cargo test --test path_security_test -- --test-threads=1
    run cargo test --test command_security_test -- --test-threads=1
    run cargo test --test tool_test -- --test-threads=1
    ;;
  audit)
    echo "=== Audit Tier ==="
    cd "$TAURI_DIR"
    run cargo test --test audit_test -- --test-threads=1
    run cargo test --test checkpoint_test -- --test-threads=1
    ;;
  workflow)
    echo "=== Workflow Tier ==="
    cd "$TAURI_DIR"
    run cargo test --test workflow_demo_test -- --test-threads=1
    run cargo test --test workflow_mock_test -- --test-threads=1
    ;;
  slow)
    echo "=== Slow Tier (full integration, no real API) ==="
    cd "$TAURI_DIR"
    run cargo test --tests -- --skip expand_requirements --test-threads=1
    ;;
  *)
    echo "Unknown tier: $TIER"
    echo "Usage: $0 {fast|core-integration|security|audit|workflow|slow}"
    exit 1
    ;;
esac

echo ""
echo "=== Tier '$TIER' complete ==="
