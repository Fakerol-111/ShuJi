//! Contract parsing: extract ContractSpec from ctrt markdown documents.
//!
//! Phase 1: regex-based extraction supporting Rust/Python/TypeScript signatures.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractSpec {
    pub functions: Vec<FunctionSig>,
    pub classes: Vec<ClassDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSig {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub return_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDef {
    pub name: String,
    pub methods: Vec<FunctionSig>,
}

/// Parse a ctrt document body (markdown) into a ContractSpec.
///
/// Strategy:
/// - Search for code blocks containing function signatures
/// - Match patterns: `fn name(params) -> Ret`, `def name(params) -> Ret`, `function name(params)`
/// - Parse parameter lists as `name: type` pairs
/// - On parse failure, return partial spec with available entries
pub fn parse_contract(body: &str) -> ContractSpec {
    let mut functions = Vec::new();
    let classes = Vec::new();

    // Extract code blocks
    let code_blocks: Vec<&str> = body
        .split("```")
        .enumerate()
        .filter(|(i, _)| i % 2 == 1) // odd-indexed blocks are code
        .map(|(_, block)| block)
        .collect();

    for block in &code_blocks {
        // Strip language tag from first line
        let block_body = if let Some(pos) = block.find('\n') {
            &block[pos + 1..]
        } else {
            block
        };

        // Parse each line for function signatures
        for line in block_body.lines() {
            let trimmed = line.trim();
            if let Some(sig) = parse_function_sig(trimmed) {
                functions.push(sig);
            }
        }
    }

    // Also scan non-code-block text for inline signatures
    let non_code: Vec<&str> = body
        .split("```")
        .enumerate()
        .filter(|(i, _)| i % 2 == 0)
        .map(|(_, t)| t)
        .collect();

    for text in &non_code {
        for line in text.lines() {
            let trimmed = line.trim();
            // Only match inline fn signatures that don't appear inside code blocks
            if let Some(sig) = parse_function_sig(trimmed) {
                if !functions.iter().any(|f| f.name == sig.name) {
                    functions.push(sig);
                }
            }
        }
    }

    ContractSpec { functions, classes }
}

/// Try to parse a single line as a function signature.
/// Supports: `fn name(p1: T1, p2: T2) -> Ret`, `def name(p1, p2)`, `function name(p1, p2)`
fn parse_function_sig(line: &str) -> Option<FunctionSig> {
    let line = line.trim();

    // Match `fn name(...) -> Ret`, `def name(...) -> Ret`, or `function name(...): Ret`
    let fn_re = regex_lite::Regex::new(
        r"^(?:fn|def|function)\s+([a-zA-Z_][a-zA-Z0-9_]*)\(([^)]*)\)\s*(?:(?::\s*)?->\s*(.+))?",
    )
    .ok()?;

    if let Some(caps) = fn_re.captures(line) {
        let name = caps.get(1)?.as_str().to_string();
        let params_str = caps.get(2).map_or("", |m| m.as_str());
        let return_type = caps
            .get(3)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| "()".to_string());

        let params = parse_params(params_str);

        // Handle `name: type` → (name, type) pairs
        Some(FunctionSig {
            name,
            params,
            return_type,
        })
    } else {
        None
    }
}

/// Parse parameter list like `a: i32, b: String` or just `x, y` into (name, type) pairs.
fn parse_params(params_str: &str) -> Vec<(String, String)> {
    if params_str.trim().is_empty() {
        return vec![];
    }

    params_str
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if let Some(pos) = p.find(':') {
                let name = p[..pos].trim().to_string();
                let typ = p[pos + 1..].trim().to_string();
                (name, typ)
            } else {
                // No type annotation — use "unknown"
                (p.to_string(), "unknown".to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_fn() {
        let sig = parse_function_sig("fn create_user(name: str, email: str) -> User").unwrap();
        assert_eq!(sig.name, "create_user");
        assert_eq!(sig.params.len(), 2);
        assert_eq!(sig.params[0], ("name".into(), "str".into()));
        assert_eq!(sig.params[1], ("email".into(), "str".into()));
        assert_eq!(sig.return_type, "User");
    }

    #[test]
    fn test_parse_python_fn() {
        let sig = parse_function_sig("def get_user(user_id: int) -> dict").unwrap();
        assert_eq!(sig.name, "get_user");
        assert_eq!(sig.params[0], ("user_id".into(), "int".into()));
        assert_eq!(sig.return_type, "dict");
    }

    #[test]
    fn test_parse_ts_fn() {
        let sig =
            parse_function_sig("function formatDate(date: Date, format: string): string").unwrap();
        assert_eq!(sig.name, "formatDate");
        assert_eq!(sig.params.len(), 2);
    }

    #[test]
    fn test_parse_fn_no_return() {
        let sig = parse_function_sig("fn do_thing(x: i32)").unwrap();
        assert_eq!(sig.name, "do_thing");
        assert_eq!(sig.return_type, "()");
    }

    #[test]
    fn test_parse_fn_no_params() {
        let sig = parse_function_sig("fn ping() -> bool").unwrap();
        assert_eq!(sig.name, "ping");
        assert!(sig.params.is_empty());
        assert_eq!(sig.return_type, "bool");
    }

    #[test]
    fn test_parse_contract_from_markdown() {
        let md = r#"# API Contract

## Functions

```rust
fn create_user(name: str, email: str) -> User
fn delete_user(id: u64) -> bool
```

## Description

Some text here.
"#;

        let spec = parse_contract(md);
        assert_eq!(spec.functions.len(), 2);
        assert_eq!(spec.functions[0].name, "create_user");
        assert_eq!(spec.functions[1].name, "delete_user");
    }

    #[test]
    fn test_parse_params_with_types() {
        let params = parse_params("a: i32, b: String, c: bool");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0], ("a".into(), "i32".into()));
        assert_eq!(params[1], ("b".into(), "String".into()));
        assert_eq!(params[2], ("c".into(), "bool".into()));
    }

    #[test]
    fn test_parse_params_no_types() {
        let params = parse_params("x, y, z");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0], ("x".into(), "unknown".into()));
    }

    #[test]
    fn test_parse_params_empty() {
        let params = parse_params("");
        assert!(params.is_empty());
    }

    #[test]
    fn test_not_a_function() {
        assert!(parse_function_sig("let x = 42;").is_none());
        assert!(parse_function_sig("class Foo {").is_none());
        assert!(parse_function_sig("// fn comment").is_none());
    }
}
