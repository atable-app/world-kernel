//! The impact engine: what changed, what can be reused, what is now necessary, and why.
//!
//! This module was cut by the reduction recorded in `docs/ADR-003-measure-before-building.md`, which
//! found that a competent application could produce the same observable results without it, and
//! restored on 2026-09-27 by `docs/ADR-004-the-facet-advantage-scales.md` because the reversal case
//! that document itself required fires: a consumed facet is a narrower cache key than a whole value,
//! and the advantage grows with the number of consumers. It is part of the Kernel again, and
//! `src/branch.rs` depends on it. The rules it enforces are also written down as a consumer-side
//! contract in `docs/IMPACT-CONTRACT.md`, where `the_contract_document_and_the_measured_codes_agree`
//! fails if the two part company.
//!
//! The engine answers four questions about a changed scope: what changed, what can still be reused and
//! on what basis, what is now necessary, and why each object is concerned or was kept. It keeps human
//! judgement human and never launches an external effect.

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
