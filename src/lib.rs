//! World Kernel experimental contract.
//!
//! The Kernel is admission, history, portable continuity and the impact engine, plus the adapters that
//! reach UNI and Kollio through their public surfaces. The impact engine was reduced away in
//! `docs/ADR-003-measure-before-building.md` and restored by the reversal case that ADR itself
//! required, recorded in `docs/ADR-004-the-facet-advantage-scales.md`. The rules it enforces are also
//! written down for a consumer in `docs/IMPACT-CONTRACT.md`, and the two are bound by a test.

pub mod adapters;
pub mod branch;
pub mod continuation;
pub mod impact;
mod kernel;
mod model;

pub use kernel::{AuthoritySource, Kernel, StaticAuthority};
pub use model::{
    AcceptedAssessment, Candidate, Coverage, GroundedChange, Intent, KernelError, ObjectRevision,
    ObjectState, Patch, Receipt, Rejection, RejectionCode, SubmissionOutcome, WorldBootstrap,
    WorldSnapshot, digest_of_bytes, digest_of_str,
};
