//! World Kernel experimental contract.
//!
//! The Kernel is admission, history and portable continuity, plus the adapters that reach UNI and
//! Kollio through their public surfaces. It is not the impact engine: that was measured against a
//! competent application and left the shipped surface, as decided in
//! `docs/ADR-003-measure-before-building.md`. The engine is still in the repository, in
//! `tests/support/impact_core/`, so the measurement can be repeated. What survives it is written down as
//! a rule a consumer can be held to in `docs/IMPACT-CONTRACT.md`, not as a struct in here.

pub mod adapters;
pub mod continuation;
mod kernel;
mod model;

pub use kernel::{AuthoritySource, Kernel, StaticAuthority};
pub use model::{
    AcceptedAssessment, Candidate, Coverage, GroundedChange, Intent, KernelError, ObjectRevision,
    ObjectState, Patch, Receipt, Rejection, RejectionCode, SubmissionOutcome, WorldBootstrap,
    WorldSnapshot, digest_of_bytes, digest_of_str,
};
