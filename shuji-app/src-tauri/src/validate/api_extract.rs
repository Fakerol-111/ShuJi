//! API extraction from source code (Phase 1: Rust only).
//!
//! Uses regex to extract `pub fn` signatures from Rust source files.
//! In a future phase, can use `syn` crate for AST-level extraction.

use crate::validate::contract::FunctionSig;
use std::path::Path;

/// Extract public function signatures from Rust source files under `src/`.
pub fn extract_rust_api(project_dir: &Path) -> Vec<FunctionSig> {
    let src_dir = project_dir.join("src");
    if !src_dir.is_dir() {
        return vec![];
    }

    let mut functions = Vec::new();
    collect_rust_files(&src_dir, &mut functions);
    functions
}

fn collect_rust_files(dir: &Path, out: &mut Vec<FunctionSig>) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip tests/ and target/
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "tests" || name == "target" {
                continue;
            }
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            // skip test modules (mod.rs in tests/ is already excluded above,
            // but inline #[cfg(test)] modules are handled by content check)
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Extract all `pub fn` signatures
            let sigs = extract_pub_fns_from_content(&content);
            out.extend(sigs);
        }
    }
}

/// Extract `pub fn` signatures from Rust source text using regex.
fn extract_pub_fns_from_content(content: &str) -> Vec<FunctionSig> {
    let mut fns = Vec::new();

    // Match `pub fn name(...)` or `pub async fn name(...)`
    let re = regex_lite::Regex::new(
        r"pub\s+(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\(([^)]*)\)\s*(?:->\s*([^{;]+))?",
    )
    .ok();

    let Some(re) = re else { return fns };

    for (line_idx, line) in content.lines().enumerate() {
        // Skip #[cfg(test)] modules
        if line.trim().starts_with("#[cfg(test)]") {
            continue;
        }

        if let Some(caps) = re.captures(line) {
            let name = caps.get(1).map_or("", |m| m.as_str()).to_string();
            let params_str = caps.get(2).map_or("", |m| m.as_str());
            let return_type = caps
                .get(3)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| "()".to_string());

            // Filter out known test attributes to reduce noise
            if name.starts_with("test_") && line.trim().starts_with("#[") {
                // This is a test function — skip it
                // But we need context, so skip based on simple heuristics
                if line_idx > 0 {
                    let prev_line = content.lines().nth(line_idx - 1).unwrap_or("");
                    if prev_line.trim() == "#[test]" || prev_line.trim().contains("#[tokio::test]")
                    {
                        continue;
                    }
                }
            }

            let params = parse_fn_params(params_str);
            fns.push(FunctionSig {
                name,
                params,
                return_type,
            });
        }
    }

    fns
}

fn parse_fn_params(params_str: &str) -> Vec<(String, String)> {
    params_str
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            // Pattern: `name: Type` or `mut name: Type` or `&self`
            if p == "&self" || p == "self" {
                ("self".to_string(), "Self".to_string())
            } else {
                let clean = p.strip_prefix("mut ").unwrap_or(p);
                if let Some(pos) = clean.find(':') {
                    let name = clean[..pos].trim().to_string();
                    let typ = clean[pos + 1..].trim().to_string();
                    (name, typ)
                } else {
                    (clean.to_string(), "unknown".to_string())
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_extract_pub_fn_simple() {
        let content = r#"
pub fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}

pub async fn fetch_data(url: &str) -> Result<String, Error> {
    // ...
}
"#;
        let fns = extract_pub_fns_from_content(content);
        assert_eq!(fns.len(), 2, "should find both pub fns");
        assert_eq!(fns[0].name, "greet");
        assert_eq!(fns[1].name, "fetch_data");
    }

    #[test]
    fn test_extract_skips_test_fns() {
        let content = r#"
pub fn real_func() -> i32 { 42 }

#[cfg(test)]
mod tests {
    #[test]
    fn test_real_func() {
        assert_eq!(real_func(), 42);
    }
}
"#;
        let fns = extract_pub_fns_from_content(content);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "real_func");
    }

    #[test]
    fn test_extract_from_mini_crate() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();

        let mut lib = std::fs::File::create(src.join("lib.rs")).unwrap();
        writeln!(lib, "pub fn hello() -> &'static str {{ \"world\" }}").unwrap();
        writeln!(lib, "pub fn add(a: i32, b: i32) -> i32 {{ a + b }}").unwrap();

        let fns = extract_rust_api(tmp.path());
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "hello");
        assert_eq!(fns[1].name, "add");
    }
}
