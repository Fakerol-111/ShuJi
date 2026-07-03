# run_full_integration.ps1 — Run the full Rust integration test suite (except real-API tests)
#
# Windows equivalent of run_full_integration.sh.
# Requires PowerShell 7+ for best experience (pwsh).

$RootDir = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$TauriDir = Join-Path $RootDir "shuji-app" "src-tauri"

# ── Test files that MUST run with --test-threads=1 ──
$SerialTests = @(
    "pipeline_test",
    "document_test",
    "dispatch_gate_test",
    "path_security_test",
    "command_security_test",
    "session_control_test",
    "session_test",
    "send_message_routing_test",
    "workflow_demo_test",
    "workflow_mock_test",
    "editor_test",
    "config_test",
    "pattern_guard_test",
    "tool_test",
    "validate_test"
)

# ── Test files safe for parallel execution ──
$ParallelTests = @(
    "actor_test",
    "audit_test",
    "checkpoint_test",
    "learning_test",
    "scenario_replay_test",
    "watchdog_behavior_test"
)

Set-Location $TauriDir

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "  Full Integration Suite (no real API)" -ForegroundColor Cyan
Write-Host "  Started: $(Get-Date)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan

Write-Host "`n--- Serial tests (--test-threads=1) ---" -ForegroundColor Yellow
foreach ($t in $SerialTests) {
    Write-Host "`n>>> $(Get-Date)  Running: $t (serial)" -ForegroundColor Magenta
    cargo test --test $t -- --test-threads=1 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $t" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "<<< $(Get-Date)  Completed: $t" -ForegroundColor Green
}

Write-Host "`n--- Parallel-safe tests ---" -ForegroundColor Yellow
foreach ($t in $ParallelTests) {
    Write-Host "`n>>> $(Get-Date)  Running: $t (parallel)" -ForegroundColor Magenta
    cargo test --test $t 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $t" -ForegroundColor Red
        exit $LASTEXITCODE
    }
    Write-Host "<<< $(Get-Date)  Completed: $t" -ForegroundColor Green
}

Write-Host "`n==============================================" -ForegroundColor Cyan
Write-Host "  Full Integration Suite Complete" -ForegroundColor Cyan
Write-Host "  Finished: $(Get-Date)" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan
