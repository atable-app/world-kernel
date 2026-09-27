//! World Kernel experimental contract.
//!
//! The Kernel is admission, history, portable continuity, the impact engine, branch convergence and
//! transferable experience, plus the adapters that reach UNI and Kollio through their public surfaces.
//!
//! Two of those were nearly absent. The impact engine was reduced away in
//! `docs/ADR-003-measure-before-building.md` and restored by the reversal case that ADR itself required,
//! in `docs/ADR-004-the-facet-advantage-scales.md`. Branch convergence is authorised to tranche 1 only, in
//! `docs/M4-PROTOCOL.md`, and transferable experience to a closed benchmark only, in
//! `docs/M5-PROTOCOL.md`.
//!
//! The impact rules are also written down from a consumer's side in `docs/IMPACT-CONTRACT.md`, and a test
//! binds that document to the engine so the two cannot drift.

pub mod adapters;
pub mod branch;
pub mod continuation;
pub mod experience;
pub mod impact;
mod kernel;
mod model;
pub mod transfer;

pub use kernel::{AuthoritySource, Kernel, StaticAuthority};
pub use model::{
    AcceptedAssessment, Candidate, Coverage, GroundedChange, Intent, KernelError, ObjectRevision,
    ObjectState, Patch, Receipt, Rejection, RejectionCode, SubmissionOutcome, WorldBootstrap,
    WorldSnapshot, digest_of_bytes, digest_of_str,
};
