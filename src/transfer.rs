//! M5 tranche 1: what of a past experience transfers, and what must be seen again.
//!
//! The operation is [`plan_transfer`], and it is read-only. It does not change target state, it does not
//! call anything, and it does not create assurance. It compares the capsule's **declared** applicability
//! conditions and dependencies against the target and reports four kinds of difference plus the target's
//! own extra constraints.
//!
//! The five statuses are the whole product, and none of them is a boolean:
//!
//! - [`TransferStatus::DirectlyReusable`] means the **experience structure** is reusable. It never means
//!   the source's evidence or assurance became the target's.
//! - [`TransferStatus::AdaptationRequired`] means a known difference needs an explicit transformation
//!   that a declared adaptation covers.
//! - [`TransferStatus::AdditionalEvidenceRequired`] means a relevant condition is not observed yet. This
//!   is the status a retrieval system must reach for instead of guessing, and it is the one most often
//!   lost.
//! - [`TransferStatus::Incompatible`] means a known target fact violates a required condition and no
//!   declared adaptation covers it.
//! - [`TransferStatus::InsufficientInformation`] means the capsule itself does not declare enough to
//!   decide, which is a statement about the source and not about the target.
//!
//! A plan is bound to both the capsule revision and the target context revision. If either moves, the
//! plan is stale and must be replanned; [`plan_is_current`] is how that is checked, and
//! [`instantiate_transfer`] refuses a stale plan.

use crate::experience::{
    ApplicabilityCondition, ApplicabilityCoverage, CapabilityState, ConditionKind,
    ExperienceCapsule, Fact, FailureObservation, ProvenanceRecord,
};

/// How a declared condition resolved in the target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionMatch {
    pub key: String,
    pub kind: ConditionKind,
    pub expected: Option<serde_json::Value>,
    pub actual: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionDifference {
    pub key: String,
    pub kind: ConditionKind,
    pub expected: Option<serde_json::Value>,
    pub actual: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionUnknown {
    pub key: String,
    pub kind: ConditionKind,
    pub expected: Option<serde_json::Value>,
    /// Why it is unknown: not observed yet, or the capability was never checked.
    pub reason: UnknownReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    FactUnknown,
    CapabilityNotObserved,
    KeyAbsentFromDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionUnavailable {
    pub key: String,
    pub reason: String,
}

/// A constraint the target has that the source never declared. It cannot be silently ignored, because the
/// source had no reason to know about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetConstraint {
    pub key: String,
    pub actual: serde_json::Value,
}

/// The comparison, which is not a textual diff.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContextDelta {
    pub same: Vec<ConditionMatch>,
    pub different: Vec<ConditionDifference>,
    pub unknown: Vec<ConditionUnknown>,
    pub unavailable: Vec<ConditionUnavailable>,
    pub extra_target_constraints: Vec<TargetConstraint>,
}

impl ContextDelta {
    /// The four parts, kept separate on purpose. There is no `is_equivalent` method, because collapsing
    /// these into one answer is the mistake the milestone exists to prevent.
    pub fn is_clean(&self) -> bool {
        self.different.is_empty() && self.unknown.is_empty() && self.unavailable.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    DirectlyReusable,
    AdaptationRequired,
    AdditionalEvidenceRequired,
    Incompatible,
    InsufficientInformation,
}

impl TransferStatus {
    /// Whether this status would let a target candidate be created from the experience as it stands.
    /// A plan with obligations still yields a candidate; a refusal does not.
    pub fn permits_candidate(&self) -> bool {
        matches!(
            self,
            Self::DirectlyReusable | Self::AdaptationRequired | Self::AdditionalEvidenceRequired
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdaptationObligation {
    pub key: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationObligation {
    pub key: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssuranceObligation {
    /// A reference to something that must be verified in the target, never a source reference reused.
    pub detail: String,
    /// Always the target, never the source.
    pub bound_to: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredExplanation {
    pub why_relevant: String,
    pub why_not_directly_reusable: String,
    pub what_differs: Vec<String>,
    pub what_is_unknown: Vec<String>,
    pub what_can_be_reused: Vec<String>,
    pub what_must_be_redone: Vec<String>,
    pub what_previous_failure_matters: Vec<String>,
    pub what_target_assurance_is_still_required: Vec<String>,
}

/// A known transformation a domain declares covers a specific difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredAdaptation {
    pub key: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPlan {
    pub experience_id: String,
    pub source_revision: u64,
    pub target_context_id: String,
    pub target_context_revision: u64,
    pub status: TransferStatus,
    pub delta: ContextDelta,
    pub reusable_components: Vec<String>,
    pub invalidated_components: Vec<String>,
    pub required_adaptations: Vec<AdaptationObligation>,
    pub required_observations: Vec<ObservationObligation>,
    pub required_assurances: Vec<AssuranceObligation>,
    pub relevant_prior_failures: Vec<FailureObservation>,
    pub explanation: StructuredExplanation,
}

/// Plan the reuse of `capsule` in `target`. Read-only.
///
/// `adaptations` is what the target domain declares it can do about a known difference. An adaptation the
/// domain has not declared is not available, however obvious it looks.
pub fn plan_transfer(
    capsule: &ExperienceCapsule,
    target: &crate::experience::ContextSnapshot,
    adaptations: &[DeclaredAdaptation],
) -> TransferPlan {
    let mut delta = ContextDelta::default();

    for condition in &capsule.applicability_conditions {
        match evaluate(condition, target) {
            Outcome::Holds(actual) => delta.same.push(ConditionMatch {
                key: condition.key.clone(),
                kind: condition.kind,
                expected: condition.expected.clone(),
                actual,
            }),
            Outcome::Differs(actual) => delta.different.push(ConditionDifference {
                key: condition.key.clone(),
                kind: condition.kind,
                expected: condition.expected.clone(),
                actual,
            }),
            Outcome::Unknown(reason) => delta.unknown.push(ConditionUnknown {
                key: condition.key.clone(),
                kind: condition.kind,
                expected: condition.expected.clone(),
                reason,
            }),
            Outcome::Unavailable(detail) => delta.unavailable.push(ConditionUnavailable {
                key: condition.key.clone(),
                reason: detail,
            }),
        }
    }

    // A target constraint the source never declared cannot be evaluated, only recorded. It is not a
    // failure and it is not silently dropped.
    for (key, value) in &target.constraints {
        let declared = capsule
            .applicability_conditions
            .iter()
            .any(|condition| &condition.key == key);
        if !declared {
            delta.extra_target_constraints.push(TargetConstraint {
                key: key.clone(),
                actual: value.clone(),
            });
        }
    }

    let adaptation_for = |key: &str| -> Option<&DeclaredAdaptation> {
        adaptations.iter().find(|adaptation| adaptation.key == key)
    };
    // Adaptability is either declared by the author of the condition or supplied by the target domain.
    // It is never inferred from the value.
    let bridgeable = |key: &str| -> bool {
        capsule
            .applicability_conditions
            .iter()
            .any(|condition| condition.key == key && condition.adaptable)
            || adaptation_for(key).is_some()
    };

    // Conditions the outcome depended on, and what a target candidate would have to observe again.
    let reobserve: Vec<ObservationObligation> = capsule
        .dependencies
        .iter()
        .map(|key| ObservationObligation {
            key: key.clone(),
            detail: format!(
                "{key} was consumed by the source run and must be observed in the target"
            ),
        })
        .collect();

    let relevant_failures = relevant_failures(capsule, target);

    // Every difference owes an obligation, because a difference the intervention *can* be
    // re-parameterised for still has to be re-parameterised. The obligation names what must change; the
    // status below says whether the difference is bridgeable at all.
    let mut required_adaptations: Vec<AdaptationObligation> = delta
        .different
        .iter()
        .map(|difference| AdaptationObligation {
            key: difference.key.clone(),
            detail: if bridgeable(&difference.key) {
                format!(
                    "re-derive {} for the target: the source used {:?} and the target is {:?}",
                    difference.key, difference.expected, difference.actual
                )
            } else {
                format!(
                    "{} was {:?} in the source and is {:?} in the target, and neither the author nor the target domain declared a way to bridge it",
                    difference.key, difference.expected, difference.actual
                )
            },
        })
        .collect();

    // A failure whose condition the target also establishes is work owed before the intervention is
    // proposed again. It is an obligation and not only a sentence in the explanation, so that a consumer
    // cannot acknowledge the failure and then instantiate anyway.
    for failure in &relevant_failures {
        required_adaptations.push(AdaptationObligation {
            key: failure.condition_key.clone(),
            detail: format!(
                "a previous attempt failed under this exact condition, and it is present again: {}",
                failure.detail
            ),
        });
    }

    let covered_differences: Vec<String> = delta
        .different
        .iter()
        .filter(|difference| bridgeable(&difference.key))
        .map(|difference| difference.key.clone())
        .collect();

    let required_assurances: Vec<AssuranceObligation> =
        vec![AssuranceObligation {
            detail: "the target outcome is verified by UNI in the target context, whatever the source proved"
                .to_owned(),
            bound_to: target.context_id.clone(),
        }];

    // Precedence is fixed and stated, because a status is a decision about which failure is more serious.
    // A known violation outranks an unobserved condition: we know it is wrong rather than not yet known.
    let status = if capsule.applicability_conditions.is_empty()
        || capsule.source_context.coverage == ApplicabilityCoverage::Opaque
    {
        TransferStatus::InsufficientInformation
    } else if !relevant_failures.is_empty() {
        // Every declared condition holds and the recorded history still says the intervention is unsafe
        // here. That is not a clean reuse and it is not a refusal: it is work to look at before the
        // intervention is proposed again.
        TransferStatus::AdaptationRequired
    } else if !delta.different.is_empty() {
        // A difference on a condition the author did not declare adaptable, with no adaptation supplied,
        // is a violation of the source's own requirement. That is what `incompatible` means, and it is
        // not a request for more evidence.
        if delta
            .different
            .iter()
            .all(|difference| bridgeable(&difference.key))
        {
            TransferStatus::AdaptationRequired
        } else {
            TransferStatus::Incompatible
        }
    } else if !delta.unknown.is_empty() || !delta.unavailable.is_empty() {
        TransferStatus::AdditionalEvidenceRequired
    } else {
        TransferStatus::DirectlyReusable
    };

    let invalidated: Vec<String> = if status == TransferStatus::DirectlyReusable {
        Vec::new()
    } else {
        vec![capsule.intervention.clone()]
    };

    let explanation = explain(
        capsule,
        target,
        &delta,
        status,
        &covered_differences,
        &required_adaptations,
        &relevant_failures,
    );

    TransferPlan {
        experience_id: capsule.id.clone(),
        delta: delta.clone(),
        source_revision: capsule.revision,
        target_context_id: target.context_id.clone(),
        target_context_revision: target.revision,
        status,
        reusable_components: if status == TransferStatus::DirectlyReusable {
            vec![capsule.problem.clone(), capsule.intervention.clone()]
        } else {
            vec![capsule.problem.clone()]
        },
        invalidated_components: invalidated,
        required_adaptations,
        required_observations: delta
            .unknown
            .iter()
            .map(|unknown| ObservationObligation {
                key: unknown.key.clone(),
                detail: format!("{} has not been observed in the target", unknown.key),
            })
            .chain(reobserve)
            .collect(),
        required_assurances,
        relevant_prior_failures: relevant_failures,
        explanation,
    }
}

enum Outcome {
    Holds(serde_json::Value),
    Differs(serde_json::Value),
    Unknown(UnknownReason),
    Unavailable(String),
}

fn evaluate(
    condition: &ApplicabilityCondition,
    target: &crate::experience::ContextSnapshot,
) -> Outcome {
    // A predicate reference is unevaluable regardless of whether the key is present, so it is answered
    // before any lookup. It is not a violation and it is not a match.
    if condition.kind == ConditionKind::PredicateRef {
        return Outcome::Unavailable(format!(
            "{} names a predicate this planner cannot evaluate",
            condition.key
        ));
    }

    // A capability condition is answered from the capability map, never from a fact of the same name. A
    // system that reads them interchangeably will treat a fact as a capability.
    if matches!(
        condition.kind,
        ConditionKind::CapabilityAvailable | ConditionKind::CapabilityUnavailable
    ) {
        return match target.capabilities.get(&condition.key) {
            Some(CapabilityState::Available) => match condition.kind {
                ConditionKind::CapabilityAvailable => {
                    Outcome::Holds(serde_json::json!("available"))
                }
                _ => Outcome::Differs(serde_json::json!("available")),
            },
            Some(CapabilityState::Unavailable) | Some(CapabilityState::Unsupported) => {
                match condition.kind {
                    ConditionKind::CapabilityAvailable => {
                        Outcome::Differs(serde_json::json!("unavailable"))
                    }
                    _ => Outcome::Holds(serde_json::json!("unavailable")),
                }
            }
            // Not observed is unknown. It is never "unavailable".
            Some(CapabilityState::NotObserved) | None => {
                Outcome::Unknown(UnknownReason::CapabilityNotObserved)
            }
        };
    }

    let fact = match target.facts.get(&condition.key) {
        Some(fact) => fact,
        None => return Outcome::Unknown(UnknownReason::KeyAbsentFromDeclaration),
    };
    let value = match fact {
        Fact::Known(value) => value,
        // Explicitly absent is an answer. It can satisfy `Absent` and it can violate `Equals`.
        Fact::Absent => {
            return match condition.kind {
                ConditionKind::Absent => Outcome::Holds(serde_json::json!(null)),
                ConditionKind::Present => Outcome::Differs(serde_json::json!(null)),
                _ => Outcome::Differs(serde_json::json!(null)),
            };
        }
        Fact::Unknown => return Outcome::Unknown(UnknownReason::FactUnknown),
    };

    let expected = condition
        .expected
        .clone()
        .unwrap_or(serde_json::Value::Null);
    let holds = match condition.kind {
        ConditionKind::Equals => value == &expected,
        ConditionKind::NotEquals => value != &expected,
        ConditionKind::Present => true,
        ConditionKind::Absent => false,
        ConditionKind::Gte => {
            numeric(value).is_some_and(|v| numeric(&expected).is_some_and(|e| v >= e))
        }
        ConditionKind::Lte => {
            numeric(value).is_some_and(|v| numeric(&expected).is_some_and(|e| v <= e))
        }
        ConditionKind::MemberOf => match expected.as_array() {
            Some(members) => members.contains(value),
            None => false,
        },
        ConditionKind::PredicateRef
        | ConditionKind::CapabilityAvailable
        | ConditionKind::CapabilityUnavailable => false,
    };
    if holds {
        Outcome::Holds(value.clone())
    } else {
        Outcome::Differs(value.clone())
    }
}

fn numeric(value: &serde_json::Value) -> Option<i64> {
    value.as_i64()
}

/// A prior failure is relevant when the target establishes the condition it was recorded under. It is not
/// relevant because the experience looks similar.
fn relevant_failures(
    capsule: &ExperienceCapsule,
    target: &crate::experience::ContextSnapshot,
) -> Vec<FailureObservation> {
    capsule
        .known_failures
        .iter()
        .filter(|failure| {
            target
                .facts
                .get(&failure.condition_key)
                .is_some_and(|fact| fact == &Fact::Known(failure.condition_value.clone()))
        })
        .cloned()
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn explain(
    capsule: &ExperienceCapsule,
    target: &crate::experience::ContextSnapshot,
    delta: &ContextDelta,
    status: TransferStatus,
    covered: &[String],
    uncovered: &[AdaptationObligation],
    failures: &[FailureObservation],
) -> StructuredExplanation {
    let why_not_directly = match status {
        TransferStatus::DirectlyReusable => {
            "every declared condition holds in the target".to_owned()
        }
        TransferStatus::AdaptationRequired => format!(
            "{} differ(s) and a declared adaptation covers it",
            covered.len()
        ),
        TransferStatus::AdditionalEvidenceRequired => format!(
            "{} declared condition(s) are not observed in the target yet",
            delta.unknown.len() + delta.unavailable.len()
        ),
        TransferStatus::Incompatible => {
            format!("{} differ(s) with no declared adaptation", uncovered.len())
        }
        TransferStatus::InsufficientInformation => {
            "the capsule declares no applicability conditions, or declares its coverage as opaque"
                .to_owned()
        }
    };
    StructuredExplanation {
        why_relevant: format!(
            "the experience addresses the same problem in a context that shares {} of {} declared condition(s)",
            delta.same.len(),
            capsule.applicability_conditions.len()
        ),
        why_not_directly_reusable: why_not_directly,
        what_differs: delta
            .different
            .iter()
            .map(|difference| {
                format!(
                    "{}: {:?} in the source, {:?} in the target",
                    difference.key, difference.expected, difference.actual
                )
            })
            .collect(),
        what_is_unknown: delta
            .unknown
            .iter()
            .map(|unknown| format!("{} has not been observed", unknown.key))
            .chain(
                delta
                    .unavailable
                    .iter()
                    .map(|unavailable| unavailable.reason.clone()),
            )
            .collect(),
        what_can_be_reused: if status == TransferStatus::DirectlyReusable {
            vec![capsule.problem.clone(), capsule.intervention.clone()]
        } else {
            vec![capsule.problem.clone()]
        },
        what_must_be_redone: if status == TransferStatus::DirectlyReusable {
            capsule
                .dependencies
                .iter()
                .map(|key| format!("observe {key} in the target"))
                .collect()
        } else {
            vec![format!("the intervention itself: {}", capsule.intervention)]
        },
        what_previous_failure_matters: failures
            .iter()
            .map(|failure| {
                format!(
                    "a previous attempt failed under {} = {:?}: {}",
                    failure.condition_key, failure.condition_value, failure.detail
                )
            })
            .collect(),
        what_target_assurance_is_still_required: vec![format!(
            "UNI verification in {}, whatever the source proved",
            target.context_id
        )],
    }
}

/// Whether a plan is still current. Bound to both revisions, so a plan made against a moved world is
/// stale rather than quietly applicable.
pub fn plan_is_current(
    plan: &TransferPlan,
    capsule: &ExperienceCapsule,
    target: &crate::experience::ContextSnapshot,
) -> bool {
    plan.source_revision == capsule.revision && plan.target_context_revision == target.revision
}

/// A new target-side candidate produced from a plan.
///
/// New identity, source references preserved, and **no source assurance copied**. The target candidate
/// starts with an empty assurance list, which is the point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiatedTransfer {
    pub id: String,
    pub source_capsule_id: String,
    pub source_revision: u64,
    pub plan_status: TransferStatus,
    pub target_context_id: String,
    pub target_context_revision: u64,
    pub applied_adaptations: Vec<String>,
    pub unresolved_assumptions: Vec<String>,
    /// Always empty on creation. A target candidate has not been verified yet.
    pub assurance_refs: Vec<String>,
    pub provenance: ProvenanceRecord,
}

/// Turn a current, permitting plan into a target candidate.
///
/// Refuses a stale plan and refuses a status that does not permit a candidate. Never mutates the source.
pub fn instantiate_transfer(
    plan: &TransferPlan,
    capsule: &ExperienceCapsule,
    target: &crate::experience::ContextSnapshot,
) -> Result<InstantiatedTransfer, InstantiationError> {
    if !plan_is_current(plan, capsule, target) {
        return Err(InstantiationError::StalePlan);
    }
    if !plan.status.permits_candidate() {
        return Err(InstantiationError::StatusRefusesCandidate(plan.status));
    }
    Ok(InstantiatedTransfer {
        id: format!("{}/{}", plan.experience_id, plan.source_revision),
        source_capsule_id: capsule.id.clone(),
        source_revision: capsule.revision,
        plan_status: plan.status,
        target_context_id: target.context_id.clone(),
        target_context_revision: target.revision,
        applied_adaptations: Vec::new(),
        unresolved_assumptions: capsule.assumptions.clone(),
        // Deliberately empty. This is the assertion that source assurance does not become target
        // assurance, and it is a construction site rather than a rule someone has to remember.
        assurance_refs: Vec::new(),
        provenance: ProvenanceRecord {
            author: capsule.provenance.author.clone(),
            activity: format!("instantiate:{}", capsule.provenance.activity),
            source_revision: capsule.revision,
            used_refs: vec![
                format!("capsule:{}@{}", capsule.id, capsule.revision),
                format!("context:{}@{}", target.context_id, target.revision),
            ],
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstantiationError {
    StalePlan,
    StatusRefusesCandidate(TransferStatus),
}

/// A recorded transfer. The outcome distinctions are the reason M6 could exist later; tranche 1 records
/// them because a failure recorded as a success is worse than no record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferOutcome {
    Succeeded,
    SucceededAfterAdaptation,
    Failed,
    Abandoned,
    Blocked,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TransferAttempt {
    pub id: String,
    pub capsule_ref: String,
    pub target_context_ref: String,
    pub plan_status: TransferStatus,
    pub outcome: TransferOutcome,
    /// Evidence obtained in the **target**, which is never the source's evidence by another name.
    #[serde(default)]
    pub target_evidence_refs: Vec<String>,
    #[serde(default)]
    pub target_assurance_refs: Vec<String>,
    pub failure: Option<FailureObservation>,
    pub provenance: ProvenanceRecord,
}
