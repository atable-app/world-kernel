//! The shared test-only modules, compiled by three targets.

//!
//! `corpus_contract.rs` includes `admission_case.rs` directly, `comparative_admission.rs`
//! includes this directory as `support`, and `examples/admission_benchmark.rs` includes it
//! by path. No single one of those targets uses every item, so dead-code analysis is
//! disabled here rather than in any individual module.
#![allow(dead_code)]

#[path = "admission_case.rs"]
pub mod admission_case;
#[path = "application_incremental.rs"]
pub mod application_incremental;
#[path = "assurance.rs"]
pub mod assurance;
#[path = "baseline_a.rs"]
pub mod baseline_a;
#[path = "baseline_b.rs"]
pub mod baseline_b;
#[path = "branch_fixture.rs"]
pub mod branch_fixture;
#[path = "harness.rs"]
pub mod harness;
#[path = "impact_fixture.rs"]
pub mod impact_fixture;
#[path = "incumbent_comparison.rs"]
pub mod incumbent_comparison;
#[path = "m3_stories.rs"]
pub mod m3_stories;
#[path = "second_domain.rs"]
pub mod second_domain;
#[path = "system_c.rs"]
pub mod system_c;
#[path = "transfer_fixture.rs"]
pub mod transfer_fixture;
