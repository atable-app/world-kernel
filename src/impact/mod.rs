//! M3: incremental, explainable revision of recorded work.
//!
//! The engine answers four questions about a changed scope: what changed, what
//! can still be reused and on what basis, what is now necessary, and why each
//! object is concerned or was kept. It keeps human judgement human and never
//! launches an external effect.

pub mod engine;
pub mod model;

pub use engine::{
    Evaluated, Evaluator, Limits, Query, ReadJournal, ReadOutcome, Revised, TrustConfiguration,
    assert_publishable, derivable, full_recompute, is_recomputable, revise,
};
pub use model::{
    Authority, CapturedRead, Coverage, Currency, Disposition, Error, EvaluationRecord, Explanation,
    Facet, Finding, HistoryState, NodeId, NodeNature, NodeVersion, Obligation, Outcome, Profile,
    QueryRead, ReadBinding, Snapshot, WorkType,
};
