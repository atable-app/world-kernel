//! World Kernel experimental contract.

pub mod adapters;
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
