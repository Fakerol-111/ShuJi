# run_test_tier.ps1 — Run a named ShuJi test tier.
#
# Usage:
#   .\scripts\run_test_tier.ps1 fast
#   .\scripts\run_test_tier.ps1 core-integration
#   .\scripts\run_test_tier.ps1 security
#   .\scripts\run_test_tier.ps1 audit
#   .\scripts\run_test_tier.ps1 workflow
#   .\scripts\run_test_tier.ps1 slow
#
# Windows equivalent of run_test_tier.sh.
# Requires PowerShell 7+ for best experience (pwsh).
# Falls back to npm/cargo commands directly — no WSL or Git Bash needed.

param(
    [Parameter(Mandatory=$true)]
    [string]$Tier
)

$RootDir = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$AppDir = Join-Path $RootDir "shuji-app"
$TauriDir = Join-Path $RootDir "shuji-app" "src-tauri"

function Run-Test {
    param([string]$Label, [scriptblock]$Block)
    Write-Host "`n==> $Label" -ForegroundColor Cyan
    & $Block
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $Label" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

switch ($Tier) {
    "fast" {
        Write-Host "=== Fast Tier ===" -ForegroundColor Green
        Run-Test "npm run lint" { Set-Location $AppDir; npm run lint }
        Run-Test "npm test" { Set-Location $AppDir; npm test }
        Run-Test "npm run format:check" { Set-Location $AppDir; npm run format:check }
        Run-Test "cargo fmt --check" { Set-Location $TauriDir; cargo fmt --check }
        Run-Test "cargo test --lib" { Set-Location $TauriDir; cargo test --lib }
        Run-Test "path_security_test" { Set-Location $TauriDir; cargo test --test path_security_test -- --test-threads=1 }
        Run-Test "document_test" { Set-Location $TauriDir; cargo test --test document_test -- --test-threads=1 }
        Run-Test "pipeline_test" { Set-Location $TauriDir; cargo test --test pipeline_test -- --test-threads=1 }
    }
    "core-integration" {
        Write-Host "=== Core Integration Tier ===" -ForegroundColor Green
        Set-Location $TauriDir
        $tests = @("pipeline_test","document_test","dispatch_gate_test","session_control_test","actor_test")
        foreach ($t in $tests) {
            Run-Test "$t" { Set-Location $TauriDir; cargo test --test $t -- --test-threads=1 }
        }
    }
    "security" {
        Write-Host "=== Security Tier ===" -ForegroundColor Green
        Set-Location $TauriDir
        $tests = @("path_security_test","command_security_test","tool_test")
        foreach ($t in $tests) {
            Run-Test "$t" { Set-Location $TauriDir; cargo test --test $t -- --test-threads=1 }
        }
    }
    "audit" {
        Write-Host "=== Audit Tier ===" -ForegroundColor Green
        Set-Location $TauriDir
        Run-Test "audit_test" { Set-Location $TauriDir; cargo test --test audit_test -- --test-threads=1 }
        Run-Test "checkpoint_test" { Set-Location $TauriDir; cargo test --test checkpoint_test -- --test-threads=1 }
    }
    "workflow" {
        Write-Host "=== Workflow Tier ===" -ForegroundColor Green
        Set-Location $TauriDir
        Run-Test "workflow_demo_test" { Set-Location $TauriDir; cargo test --test workflow_demo_test -- --test-threads=1 }
        Run-Test "workflow_mock_test" { Set-Location $TauriDir; cargo test --test workflow_mock_test -- --test-threads=1 }
    }
    "slow" {
        Write-Host "=== Slow Tier (full integration, no real API) ===" -ForegroundColor Green
        Set-Location $TauriDir
        Run-Test "cargo test --tests (skip expand_requirements)" { Set-Location $TauriDir; cargo test --tests -- --skip expand_requirements --test-threads=1 }
    }
    default {
        Write-Host "Unknown tier: $Tier" -ForegroundColor Red
        Write-Host "Usage: $0 {fast|core-integration|security|audit|workflow|slow}"
        exit 1
    }
}

Write-Host "`n=== Tier '$Tier' complete ===" -ForegroundColor Green
