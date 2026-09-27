//! The shared test-only modules, compiled by three targets.
//!
//! `corpus_contract.rs` includes `admission_case.rs` directly, `comparative_admission.rs`
//! includes this directory as `support`, and `examples/admission_benchmark.rs` includes it
//! by path. No single one of those targets uses every item, so dead-code analysis is
//! disabled here rather than in any individual module.
#![allow(dead_code)]

#[path = "admission_case.rs"]
pub mod admission_case;
#[path = "assurance.rs"]
pub mod assurance;
#[path = "baseline_a.rs"]
pub mod baseline_a;
#[path = "baseline_b.rs"]
pub mod baseline_b;
#[path = "harness.rs"]
pub mod harness;
#[path = "system_c.rs"]
pub mod system_c;
