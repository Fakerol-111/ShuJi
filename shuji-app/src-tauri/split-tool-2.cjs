const fs = require('fs');
const INPUT = 'shuji-app/src-tauri/src/tool/mod.rs';
const OUTDIR = 'shuji-app/src-tauri/src/tool/';

const lines = fs.readFileSync(INPUT, 'utf-8').split('\n');

// ===== FILE BOUNDARIES (0-indexed) =====
// Line 0-13: imports (keep in mod.rs)
// 14-57: cache section
// 58-81: ToolContext  
// 82-172: resolve_scoped_path
// 173: "ToolOutput moved" comment
// 175-655: file_ops section (append → modify_file)
// 656-1912: command_ops section (execute_command → before Central tool dispatch)
// 1914: truncate_tool_result_by_name (goes to dispatch)
// 1915-1936: blank/tool_defs (goes to command_ops)
// 1937-2475: dispatch section (execute_named_tool → end)

// ===== CROSS-REFERENCE HANDLING =====
// cache_invalidate: fn → pub(crate) fn (used by dispatch)
// truncate_tool_result_by_name: moves to dispatch.rs (called from dispatch)

// ===== FILE 1: cache.rs (lines 14-57) =====
const cacheLines = lines.slice(14, 58); // includes cache_invalidate
let cacheContent = `use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;
use std::path::{Path, PathBuf};

${cacheLines.join('\n')}`;
// Fix visibility of cache_invalidate
cacheContent = cacheContent.replace('fn cache_invalidate', 'pub(crate) fn cache_invalidate');
// Remove the // ── P2-2: In-memory read cache ───── comment since it's the whole file now
fs.writeFileSync(OUTDIR + 'cache.rs', cacheContent);
console.log('Wrote cache.rs');

// ===== FILE 2: path_security.rs (lines 82-172) =====
const pathLines = lines.slice(82, 173);
const pathContent = `use std::path::{Path, PathBuf};

${pathLines.join('\n')}`;
fs.writeFileSync(OUTDIR + 'path_security.rs', pathContent);
console.log('Wrote path_security.rs');

// ===== FILE 3: file_ops.rs (lines 175-655) =====
const fileOpsLines = lines.slice(175, 656);
const fileOpsContent = `use crate::tool::{ToolOutput, resolve_scoped_path};
use std::path::Path;

${fileOpsLines.join('\n')}`;
fs.writeFileSync(OUTDIR + 'file_ops.rs', fileOpsContent);
console.log('Wrote file_ops.rs');

// ===== FILE 4: command_ops.rs (lines 656-1912, then 1915-1936) =====
// Note: line 1913 is blank, 1914 is truncate_tool (moved to dispatch)
// Lines 1915-1936 are tool_def functions that go with command_ops
const cmdOpsLines = lines.slice(656, 1913);
const cmdTailLines = lines.slice(1915, 1937);
const cmdOpsContent = `use crate::tool::ToolOutput;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use tokio::io::AsyncReadExt;

${cmdOpsLines.join('\n')}
${cmdTailLines.join('\n')}`;
fs.writeFileSync(OUTDIR + 'command_ops.rs', cmdOpsContent);
console.log('Wrote command_ops.rs');

// ===== FILE 5: dispatch.rs (ToolContext + truncate_tool + execute_named_tool + neige_special) =====
// ToolContext lines 58-81 + truncate_tool_result_by_name line 1914 + dispatch lines 1937-2475
const ctxLines = lines.slice(58, 82);
const truncateLine = lines[1914];
const dispLines = lines.slice(1937);
const dispatchContent = `use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;
use crate::actor::FastMessage;
use crate::api::client::AnthropicClient;
use crate::models::role::Role;
use crate::tool::{ToolOutput, cache_invalidate};

${ctxLines.join('\n')}

${truncateLine}

${dispLines.join('\n')}`;
fs.writeFileSync(OUTDIR + 'dispatch.rs', dispatchContent);
console.log('Wrote dispatch.rs');

// ===== NEW mod.rs =====
const modLines = [
  '// ── Module declarations ─────────────────────────────────',
  '',
  'pub mod cache;',
  'pub mod command_ops;',
  'pub mod dispatch;',
  'pub mod documents;',
  'pub mod file_ops;',
  'pub mod output;',
  'pub mod path_security;',
  'pub mod registry;',
  'mod tool_log;',
  '',
  '// ── Re-exports ─────────────────────────────────────────────',
  '',
  'pub use cache::*;',
  'pub use command_ops::*;',
  'pub use dispatch::*;',
  'pub use file_ops::*;',
  'pub use output::*;',
  'pub use path_security::*;',
];
fs.writeFileSync(OUTDIR + 'mod.rs', modLines.join('\n'));
console.log('Wrote mod.rs');
console.log('Done! All files extracted.');
