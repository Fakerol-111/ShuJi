//! Diff between ContractSpec and extracted API.
//!
//! Produces a structured diff showing:
//! - Functions in contract but missing from code
//! - Functions in code but not in contract
//! - Signature mismatches (parameter or return type differences)

use crate::validate::contract::{ContractSpec, FunctionSig};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ApiDiff {
    pub missing_in_code: Vec<FunctionSig>,
    pub extra_in_code: Vec<FunctionSig>,
    pub signature_mismatch: Vec<SignatureMismatch>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignatureMismatch {
    pub name: String,
    pub expected: String, // from contract
    pub actual: String,   // from code
}

/// Compare contract spec against extracted API functions.
pub fn diff_contract_vs_code(contract: &ContractSpec, code_fns: &[FunctionSig]) -> ApiDiff {
    let mut missing_in_code = Vec::new();
    let mut extra_in_code = Vec::new();
    let mut signature_mismatch = Vec::new();

    for contract_fn in &contract.functions {
        match code_fns.iter().find(|cf| cf.name == contract_fn.name) {
            None => {
                missing_in_code.push(contract_fn.clone());
            }
            Some(code_fn) => {
                // Check signature match
                let param_match = contract_fn.params.len() == code_fn.params.len()
                    && contract_fn
                        .params
                        .iter()
                        .zip(&code_fn.params)
                        .all(|(a, b)| a.0 == b.0 && a.1 == b.1);
                let return_match = contract_fn.return_type == code_fn.return_type;

                if !param_match || !return_match {
                    let expected = format!(
                        "fn {}({}) -> {}",
                        contract_fn.name,
                        contract_fn
                            .params
                            .iter()
                            .map(|(n, t)| format!("{}: {}", n, t))
                            .collect::<Vec<_>>()
                            .join(", "),
                        contract_fn.return_type
                    );
                    let actual = format!(
                        "fn {}({}) -> {}",
                        code_fn.name,
                        code_fn
                            .params
                            .iter()
                            .map(|(n, t)| format!("{}: {}", n, t))
                            .collect::<Vec<_>>()
                            .join(", "),
                        code_fn.return_type
                    );
                    signature_mismatch.push(SignatureMismatch {
                        name: contract_fn.name.clone(),
                        expected,
                        actual,
                    });
                }
            }
        }
    }

    for code_fn in code_fns {
        if !contract.functions.iter().any(|cf| cf.name == code_fn.name) {
            extra_in_code.push(code_fn.clone());
        }
    }

    ApiDiff {
        missing_in_code,
        extra_in_code,
        signature_mismatch,
    }
}

impl ApiDiff {
    pub fn is_clean(&self) -> bool {
        self.missing_in_code.is_empty()
            && self.extra_in_code.is_empty()
            && self.signature_mismatch.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::contract::{ContractSpec, FunctionSig};

    fn make_fn(name: &str, params: Vec<(&str, &str)>, ret: &str) -> FunctionSig {
        FunctionSig {
            name: name.to_string(),
            params: params
                .into_iter()
                .map(|(n, t)| (n.to_string(), t.to_string()))
                .collect(),
            return_type: ret.to_string(),
        }
    }

    #[test]
    fn test_contract_matches_code() {
        let contract = ContractSpec {
            functions: vec![
                make_fn("hello", vec![], "String"),
                make_fn("add", vec![("a", "i32"), ("b", "i32")], "i32"),
            ],
            classes: vec![],
        };

        let code = vec![
            make_fn("hello", vec![], "String"),
            make_fn("add", vec![("a", "i32"), ("b", "i32")], "i32"),
        ];

        let diff = diff_contract_vs_code(&contract, &code);
        assert!(diff.is_clean(), "matching code should produce no diff");
    }

    #[test]
    fn test_missing_function_in_code() {
        let contract = ContractSpec {
            functions: vec![make_fn("missing_fn", vec![], "()")],
            classes: vec![],
        };
        let code = vec![];

        let diff = diff_contract_vs_code(&contract, &code);
        assert_eq!(diff.missing_in_code.len(), 1);
        assert_eq!(diff.missing_in_code[0].name, "missing_fn");
        assert!(!diff.is_clean());
    }

    #[test]
    fn test_extra_function_in_code() {
        let contract = ContractSpec {
            functions: vec![],
            classes: vec![],
        };
        let code = vec![make_fn("extra_fn", vec![], "()")];

        let diff = diff_contract_vs_code(&contract, &code);
        assert_eq!(diff.extra_in_code.len(), 1);
        assert_eq!(diff.extra_in_code[0].name, "extra_fn");
    }

    #[test]
    fn test_signature_mismatch() {
        let contract = ContractSpec {
            functions: vec![make_fn("add", vec![("a", "i32"), ("b", "i32")], "i32")],
            classes: vec![],
        };
        let code = vec![make_fn("add", vec![("a", "i64"), ("b", "i64")], "i64")];

        let diff = diff_contract_vs_code(&contract, &code);
        assert_eq!(diff.signature_mismatch.len(), 1);
        assert_eq!(diff.signature_mismatch[0].name, "add");
    }
}
