//! The impact engine that was measured, kept only so the measurement can be repeated.
//!
//! This is not part of the Kernel. It was `src/impact` until the reduction recorded in
//! `docs/ADR-003-measure-before-building.md`, which found that a competent application can produce the
//! same observable results without it. It lives here now so that the comparison, and the reversal case
//! stated with it, stay re-runnable. Nothing in `src/` depends on it.
//!
//! The engine answers four questions about a changed scope: what changed, what can still be reused and
//! on what basis, what is now necessary, and why each object is concerned or was kept. It keeps human
//! judgement human and never launches an external effect.
//!
//! The rules it enforced are not lost with it. They are written down as a consumer-side contract in
//! `docs/IMPACT-CONTRACT.md`, and `the_contract_document_and_the_measured_codes_agree` fails if the two
//! part company.

// Every test target that compiles this module uses a different subset of the re-exports below, and
// nothing in a test crate is publicly reachable, so the facade is read as unused per target. The
// re-exports exist for the experiment as a whole, not for any one target.
#![allow(unused_imports)]

pub mod engine;
pub mod model;

pub use engine::{
    Evaluated, Evaluator, Limits, Query, ReadJournal, ReadOutcome, Revised, TrustConfiguration,
    assert_publishable, derivable, full_recompute, is_recomputable, revise,
};
pub use model::{
    Authority, CapturedRead, Coverage, Currency, Disposition, Error, EvaluationRecord, Explanation,
    FACET_COMPARATOR_VERSION, Facet, Finding, HistoryState, NodeId, NodeNature, NodeVersion,
    Obligation, Outcome, Profile, QueryRead, ReadBinding, Snapshot, WorkType,
};
