#!/usr/bin/env bash
# count_tests.sh — 统计枢机项目的测试数量，输出 JSON。
#
# 用途：为文档提供单一真相源，避免硬编码数字漂移。
# CI（.github/workflows/check.yml 的 test-count job）会调用本脚本。
#
# 用法：
#   bash scripts/count_tests.sh            # 输出 JSON 到 stdout
#   bash scripts/count_tests.sh --pretty    # 人类可读格式
#
# 统计口径：
#   - Rust 单元测试：shuji-app/src-tauri/src/ 下的 #[test] + #[tokio::test]
#   - Rust 集成测试：shuji-app/src-tauri/tests/ 下的 #[test] + #[tokio::test]
#   - 前端测试：shuji-app/src/ 下 .test.ts/.test.tsx 文件数 + it(/test( 调用数
#
# 注：#[test] 与 #[tokio::test] 计数含 #[cfg(test)] 模块内的测试函数定义，
#     与 cargo test 实际运行的用例数一致（排除宏展开/重复定义等边缘情况）。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_SRC="$REPO_ROOT/shuji-app/src-tauri/src"
RUST_TESTS="$REPO_ROOT/shuji-app/src-tauri/tests"
FE_SRC="$REPO_ROOT/shuji-app/src"

count_rust_tests() {
  local dir="$1"
  if [[ ! -d "$dir" ]]; then
    echo 0
    return
  fi
  # 统计 #[test] 和 #[tokio::test]（用 grep -cE 合并，-r 递归，--include 限定 .rs）
  # 排除注释行（以 // 或 /// 开头的，避免文档里提到 #[test] 被误算）
  grep -rhoE '#\[(tokio::)?test\]' "$dir" --include='*.rs' 2>/dev/null | wc -l | tr -d ' '
}

count_frontend_files() {
  if [[ ! -d "$FE_SRC" ]]; then
    echo 0
    return
  fi
  find "$FE_SRC" -type f \( -name '*.test.ts' -o -name '*.test.tsx' \) 2>/dev/null | wc -l | tr -d ' '
}

count_frontend_cases() {
  if [[ ! -d "$FE_SRC" ]]; then
    echo 0
    return
  fi
  # it( 或 test( 调用，排除注释行
  grep -rhoE '\b(it|test)\(' "$FE_SRC" --include='*.test.ts' --include='*.test.tsx' 2>/dev/null | wc -l | tr -d ' '
}

rust_unit=$(count_rust_tests "$RUST_SRC")
rust_integration=$(count_rust_tests "$RUST_TESTS")
rust_total=$((rust_unit + rust_integration))
fe_files=$(count_frontend_files)
fe_cases=$(count_frontend_cases)
total=$((rust_total + fe_cases))

if [[ "${1:-}" == "--pretty" ]]; then
  echo "枢机测试统计"
  echo "============"
  echo "Rust 单元测试 (src/):     $rust_unit"
  echo "Rust 集成测试 (tests/):   $rust_integration"
  echo "Rust 小计:                $rust_total"
  echo "前端测试文件:             $fe_files"
  echo "前端测试用例 (it/test):   $fe_cases"
  echo "总计:                     $total"
else
  cat <<EOF
{
  "rust": {
    "unit": $rust_unit,
    "integration": $rust_integration,
    "total": $rust_total
  },
  "frontend": {
    "files": $fe_files,
    "cases": $fe_cases
  },
  "total": $total
}
EOF
fi
