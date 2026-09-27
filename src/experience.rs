//! M5 tranche 1: a past attempt packaged as an inspectable, reusable experience.
//!
//! The type shapes here are not decoration. Three of them exist to make a specific mistake impossible to
//! write down:
//!
//! - [`Fact`] refuses to collapse "we looked and it is not there" into "we do not know". A retrieval
//!   system that cannot tell those apart will treat a missing observation as a fact.
//! - [`CapabilityState`] keeps "unsupported" apart from "not observed yet", because a missing observation
//!   is a thing to go and get, not a thing to refuse on.
//! - [`ApplicabilityCoverage`] is a claim about completeness that the capsule carries and cannot upgrade
//!   for itself. A partial list of conditions is not a complete list of conditions.
//!
//! The capsule is immutable once published. A correction is a new revision that links to the old one, so
//! that a decision taken against the old revision keeps pointing at what it actually read.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A declared fact about a work context.
///
/// The four cases are kept apart on purpose. Collapsing `Absent` and `Unknown` is the error that turns a
/// high-confidence retrieval into a false direct transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fact {
    /// Observed and known.
    Known(serde_json::Value),
    /// Observed, and established that it is not there.
    Absent,
    /// Not observed. Nobody has looked. This is not the same as `Absent`.
    Unknown,
}

/// A declared capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Available,
    Unavailable,
    /// Observed and known not to exist in this environment.
    Unsupported,
    /// Nobody has checked. An obligation to observe, not a refusal.
    NotObserved,
}

impl CapabilityState {
    pub fn is_established(&self) -> bool {
        matches!(
            self,
            Self::Available | Self::Unavailable | Self::Unsupported
        )
    }
}

/// The declared set of facts and capabilities relevant to an experience.
///
/// This is not a description of reality. It is the set the author declared to matter, which is why it
/// carries a coverage claim of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ContextSnapshot {
    pub context_id: String,
    pub revision: u64,
    #[serde(default)]
    pub facts: BTreeMap<String, Fact>,
    #[serde(default)]
    pub capabilities: BTreeMap<String, CapabilityState>,
    #[serde(default)]
    pub constraints: BTreeMap<String, serde_json::Value>,
    /// Whether the author claims the declared set is complete.
    #[serde(default)]
    pub coverage: ApplicabilityCoverage,
}

impl ContextSnapshot {
    pub fn new(context_id: &str, revision: u64) -> Self {
        Self {
            context_id: context_id.to_owned(),
            revision,
            facts: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            constraints: BTreeMap::new(),
            coverage: ApplicabilityCoverage::Opaque,
        }
    }

    pub fn with_fact(mut self, key: &str, fact: Fact) -> Self {
        self.facts.insert(key.to_owned(), fact);
        self
    }

    pub fn with_capability(mut self, key: &str, state: CapabilityState) -> Self {
        self.capabilities.insert(key.to_owned(), state);
        self
    }
}

/// What the author claims about the completeness of the declared conditions.
///
/// A capsule grants this to itself for its own conditions, which is exactly the move the impact engine
/// refuses: a producer never awards itself a stronger trust profile than the consumer grants. So a
/// capsule's coverage is read as a claim, and the planner honours `ClosedDeclared` only when the
/// target's own context declares the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApplicabilityCoverage {
    /// The domain guarantees all relevant inputs are captured.
    ClosedDeclared,
    /// Known conditions are captured. Completeness is not guaranteed.
    PartialDeclared,
    /// Discovery only. Direct reuse cannot be established from this capsule.
    #[default]
    Opaque,
}

/// One condition the author believes matters for reuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ApplicabilityCondition {
    pub key: String,
    pub kind: ConditionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<serde_json::Value>,
    /// Whether the intervention carries a parameter that can be re-derived for this key.
    ///
    /// This is the difference between "the target reserves 30 and the source reserved 10", which is
    /// adaptable by setting the parameter, and "the target has no local storage", which is not. The
    /// distinction is declared by the author and defaults to false, because guessing that a difference
    /// is bridgeable is how a false direct transfer happens with extra steps.
    #[serde(default)]
    pub adaptable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    Equals,
    NotEquals,
    Present,
    Absent,
    Gte,
    Lte,
    MemberOf,
    CapabilityAvailable,
    CapabilityUnavailable,
    PredicateRef,
}

impl ApplicabilityCondition {
    pub fn equals(key: &str, expected: serde_json::Value) -> Self {
        Self {
            key: key.to_owned(),
            kind: ConditionKind::Equals,
            expected: Some(expected),
            adaptable: false,
        }
    }

    /// A value the intervention sets, so a different value in the target is a re-parameterisation rather
    /// than a refutation.
    pub fn adaptable_equals(key: &str, expected: serde_json::Value) -> Self {
        Self {
            key: key.to_owned(),
            kind: ConditionKind::Equals,
            expected: Some(expected),
            adaptable: true,
        }
    }

    pub fn gte(key: &str, expected: i64) -> Self {
        Self {
            key: key.to_owned(),
            kind: ConditionKind::Gte,
            expected: Some(serde_json::json!(expected)),
            adaptable: false,
        }
    }

    pub fn capability_available(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            kind: ConditionKind::CapabilityAvailable,
            expected: None,
            adaptable: false,
        }
    }
}

/// How an attempt ended, and what it did not establish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ExperienceOutcome {
    pub verdict: String,
    pub detail: String,
    /// What this outcome does not prove. Usually: that it holds anywhere else.
    #[serde(default)]
    pub limits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct FailureObservation {
    pub id: String,
    /// The condition that was present when it failed.
    pub condition_key: String,
    pub condition_value: serde_json::Value,
    /// How it failed, in terms someone can recognise next time.
    pub detail: String,
}

/// Who produced it, from what, and under which revision. Never a reconstructed narrative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProvenanceRecord {
    pub author: String,
    pub activity: String,
    pub source_revision: u64,
    /// References into history. Not a claim of causation.
    #[serde(default)]
    pub used_refs: Vec<String>,
}

/// What was done, and to what. Kept together so a capsule cannot be built with a problem and an
/// intervention that answer to different work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Intervention {
    pub problem: String,
    pub description: String,
}

impl Intervention {
    pub fn new(problem: &str, description: &str) -> Self {
        Self {
            problem: problem.to_owned(),
            description: description.to_owned(),
        }
    }
}

/// An immutable, packaged past attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ExperienceCapsule {
    pub schema_version: String,
    pub id: String,
    pub revision: u64,
    pub source_context: ContextSnapshot,
    pub problem: String,
    pub intervention: String,
    /// The conditions known to matter. A list, which is not the same as a complete set.
    pub applicability_conditions: Vec<ApplicabilityCondition>,
    /// What the intervention consumed. Used to compute what has to be observed again.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Assumptions the outcome rested on. They are not promoted by a matching fact.
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub outcome: ExperienceOutcome,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    /// Assurances obtained in the **source**. Never inherited by a target.
    #[serde(default)]
    pub assurance_refs: Vec<String>,
    #[serde(default)]
    pub known_failures: Vec<FailureObservation>,
    pub provenance: ProvenanceRecord,
}

impl ExperienceCapsule {
    /// Build a capsule. The revision and schema are set here rather than by the caller so a capsule
    /// cannot claim to be a later version of itself than it is.
    pub fn new(
        id: &str,
        revision: u64,
        source_context: ContextSnapshot,
        intervention: Intervention,
        applicability_conditions: Vec<ApplicabilityCondition>,
        outcome: ExperienceOutcome,
        provenance: ProvenanceRecord,
    ) -> Self {
        Self {
            schema_version: "world-experience-capsule/v0".to_owned(),
            id: id.to_owned(),
            revision,
            source_context,
            problem: intervention.problem,
            intervention: intervention.description,
            applicability_conditions,
            dependencies: Vec::new(),
            assumptions: Vec::new(),
            outcome,
            evidence_refs: Vec::new(),
            assurance_refs: Vec::new(),
            known_failures: Vec::new(),
            provenance,
        }
    }

    pub fn with_dependency(mut self, key: &str) -> Self {
        self.dependencies.push(key.to_owned());
        self
    }

    pub fn with_assurance(mut self, reference: &str) -> Self {
        self.assurance_refs.push(reference.to_owned());
        self
    }

    pub fn with_failure(mut self, failure: FailureObservation) -> Self {
        self.known_failures.push(failure);
        self
    }

    /// The next revision of this capsule. Corrections never mutate; they link and supersede.
    pub fn revise(&self, id: &str) -> Self {
        let mut next = self.clone();
        next.revision += 1;
        next.id = format!("{id}@{}", next.revision);
        next
    }
}
