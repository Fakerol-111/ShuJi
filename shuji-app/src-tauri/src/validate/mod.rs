//! Validate module: mechanical QC gates for delivery verification.
//!
//! Entry point: `validate_delivery()` - run all enabled checks and produce a report.

pub mod api_extract;
pub mod contract;
pub mod delivery;
pub mod design_schema;
pub mod diff;
pub mod lint;
pub mod report;
pub mod tests_runner;

pub use delivery::validate_delivery;
pub use report::{CheckResult, DeliveryOptions, ValidateConfig, ValidationReport};
