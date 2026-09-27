//! Test-only modules shared by `comparative_admission.rs`,
//! `corpus_contract.rs` and `examples/admission_benchmark.rs`.
//!
//! The example reaches this directory with
//! `#[path = "../tests/support/mod.rs"] mod support;` so that the benchmark
//! systems and the corpus parser are literally the same code in both places.
//! Nothing in here may be imported by `src/`.

pub mod admission_case;
