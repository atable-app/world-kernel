//! The closed transfer fixture, an independent oracle, and a competent non-Kernel baseline.
//!
//! Three separate programs, on purpose.
//!
//! - The fixture builds the closed worlds the protocol fixes. It is invented arithmetic: not Sarah, not
//!   IntentLane, not a real budget.
//! - The oracle is a separate program over plain values that computes the expected status directly. It
//!   calls nothing in `world_kernel::transfer`, so a bug in the planner and a matching bug in the
//!   expectation cannot cancel out.
//! - Baseline A is a competent retrieval-plus-gate with no Kernel dependency. It is not a strawman: it
//!   does metadata filtering, deterministic comparison and has its own explicit unknown state, because
//!   the M1 lesson was that a strawman invalidates the experiment.

use std::collections::BTreeMap;

use serde_json::json;
use world_kernel::experience::{
    ApplicabilityCondition, ApplicabilityCoverage, CapabilityState, ContextSnapshot,
    ExperienceCapsule, ExperienceOutcome, Fact, FailureObservation, ProvenanceRecord,
};

pub const CAPACITY: &str = "capacity";
pub const LOAD: &str = "load";
pub const MINIMUM_RESERVE: &str = "minimum_reserve";

/// The source world from the protocol: `capacity 100`, `load 80`, `minimum_reserve 10`.
pub fn source_context() -> ContextSnapshot {
    ContextSnapshot::new("world:source", 4)
        .with_fact(CAPACITY, Fact::Known(json!(100)))
        .with_fact(LOAD, Fact::Known(json!(80)))
        .with_fact(MINIMUM_RESERVE, Fact::Known(json!(10)))
        .with_capability("runtime.storage.local", CapabilityState::Available)
        .with_capability("single_writer", CapabilityState::Available)
}

/// Target B: capacity and load identical to the source, reserve tripled. Superficially a near-perfect
/// match, which is the trap.
pub fn target_b() -> ContextSnapshot {
    ContextSnapshot::new("world:target-b", 7)
        .with_fact(CAPACITY, Fact::Known(json!(100)))
        .with_fact(LOAD, Fact::Known(json!(80)))
        .with_fact(MINIMUM_RESERVE, Fact::Known(json!(30)))
        .with_capability("runtime.storage.local", CapabilityState::Available)
        .with_capability("single_writer", CapabilityState::Available)
}

/// Target C: every declared condition holds, with a lighter load.
pub fn target_c() -> ContextSnapshot {
    ContextSnapshot::new("world:target-c", 2)
        .with_fact(CAPACITY, Fact::Known(json!(100)))
        .with_fact(LOAD, Fact::Known(json!(70)))
        .with_fact(MINIMUM_RESERVE, Fact::Known(json!(10)))
        .with_capability("runtime.storage.local", CapabilityState::Available)
        .with_capability("single_writer", CapabilityState::Available)
}

/// The deceptive-similarity pair: same framework, different runtime, the capability gone.
pub fn storage_source() -> ContextSnapshot {
    ContextSnapshot::new("world:storage-source", 1)
        .with_fact("framework", Fact::Known(json!("React")))
        .with_fact("runtime", Fact::Known(json!("browser")))
        .with_capability("runtime.storage.local", CapabilityState::Available)
        .with_capability("single_writer", CapabilityState::Available)
}

pub fn storage_target() -> ContextSnapshot {
    ContextSnapshot::new("world:storage-server", 1)
        .with_fact("framework", Fact::Known(json!("React")))
        .with_fact("runtime", Fact::Known(json!("server")))
        .with_capability("runtime.storage.local", CapabilityState::Unsupported)
        .with_capability("single_writer", CapabilityState::Available)
}

/// A target that has never observed the capability the source requires.
pub fn unobserved_target() -> ContextSnapshot {
    ContextSnapshot::new("world:unobserved", 1)
        .with_fact("framework", Fact::Known(json!("React")))
        .with_fact("runtime", Fact::Known(json!("browser")))
        .with_capability("runtime.storage.local", CapabilityState::Available)
        .with_capability(
            "capability.app_intent_schema_x",
            CapabilityState::NotObserved,
        )
}

fn provenance(author: &str, activity: &str) -> ProvenanceRecord {
    ProvenanceRecord {
        author: author.to_owned(),
        activity: activity.to_owned(),
        source_revision: 1,
        used_refs: Vec::new(),
    }
}

/// The reserve experience from the protocol's first red test.
pub fn reserve_capsule() -> ExperienceCapsule {
    ExperienceCapsule::new(
        "exp/reserve",
        3,
        {
            let mut context = source_context();
            context.coverage = ApplicabilityCoverage::ClosedDeclared;
            context
        },
        world_kernel::experience::Intervention::new(
            "admit a load that sits just under capacity",
            "reserve 10 units before admission",
        ),
        // The intervention hard-codes a reserve of 10, so it is only valid where the minimum reserve is
        // exactly 10. Declaring `gte 10` would be a different experience: one that holds at 30 too, and
        // would therefore license an intervention that reserves less than the target demands.
        vec![
            ApplicabilityCondition::adaptable_equals(MINIMUM_RESERVE, json!(10)),
            ApplicabilityCondition::equals(CAPACITY, json!(100)),
        ],
        ExperienceOutcome {
            verdict: "accepted".to_owned(),
            detail: "no overflow".to_owned(),
            limits: vec![
                "does not establish anything about a target with a different minimum reserve"
                    .to_owned(),
            ],
        },
        provenance("planning-agent", "reserve-before-admission"),
    )
    .with_dependency(MINIMUM_RESERVE)
    .with_assurance("uni-evidence:source-only")
}

/// The deceptive-similarity experience: it claims the same framework, and that is the lure.
pub fn storage_capsule() -> ExperienceCapsule {
    ExperienceCapsule::new(
        "exp/local-storage",
        1,
        {
            let mut context = storage_source();
            context.coverage = ApplicabilityCoverage::ClosedDeclared;
            context
        },
        world_kernel::experience::Intervention::new(
            "persist a small preference in local storage",
            "write to runtime.storage.local",
        ),
        vec![
            ApplicabilityCondition::equals("framework", json!("React")),
            ApplicabilityCondition::equals("runtime", json!("browser")),
            ApplicabilityCondition::capability_available("runtime.storage.local"),
            ApplicabilityCondition::capability_available("single_writer"),
        ],
        ExperienceOutcome {
            verdict: "accepted".to_owned(),
            detail: "written and read back".to_owned(),
            limits: vec!["established only in a browser runtime".to_owned()],
        },
        provenance("client-agent", "persist-preference"),
    )
    .with_dependency("runtime.storage.local")
}

/// The same experience with the capability it needs never observed in the target.
pub fn unobserved_capsule() -> ExperienceCapsule {
    let mut capsule = storage_capsule();
    capsule.applicability_conditions = vec![
        ApplicabilityCondition::equals("framework", json!("React")),
        ApplicabilityCondition::capability_available("capability.app_intent_schema_x"),
    ];
    capsule
}

/// An experience whose recorded failure must resurface when the condition returns.
pub fn single_writer_capsule() -> ExperienceCapsule {
    ExperienceCapsule::new(
        "exp/single-writer",
        2,
        {
            let mut context = storage_source();
            context.coverage = ApplicabilityCoverage::ClosedDeclared;
            context
        },
        world_kernel::experience::Intervention::new(
            "update a shared record in place",
            "write the record in place without a lock",
        ),
        vec![ApplicabilityCondition::capability_available(
            "single_writer",
        )],
        ExperienceOutcome {
            verdict: "accepted".to_owned(),
            detail: "succeeded in the source".to_owned(),
            limits: vec!["a previous transfer of this failed under multiple writers".to_owned()],
        },
        provenance("runtime-agent", "in-place-update"),
    )
    .with_failure(FailureObservation {
        id: "fail/1".to_owned(),
        condition_key: "single_writer".to_owned(),
        condition_value: json!("false"),
        detail: "an in-place write lost a concurrent update".to_owned(),
    })
}

/// A target that establishes the same failing condition.
pub fn multi_writer_target() -> ContextSnapshot {
    ContextSnapshot::new("world:multi-writer", 3)
        .with_fact("single_writer", Fact::Known(json!("false")))
        .with_capability("single_writer", CapabilityState::Available)
}

// ---------------------------------------------------------------------------------------------
// The independent oracle. Plain values, no Kernel types beyond the context it is handed.
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleStatus {
    DirectlyReusable,
    AdaptationRequired,
    AdditionalEvidenceRequired,
    Incompatible,
    InsufficientInformation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleExpectation {
    pub status: OracleStatus,
    pub differing_keys: Vec<String>,
    pub unknown_keys: Vec<String>,
    pub adaptations_needed: Vec<String>,
}

impl OracleExpectation {
    /// Every obligation the planner must produce, derived here from first principles.
    pub fn all_obligations(&self) -> Vec<String> {
        let mut obligations: Vec<String> = self
            .unknown_keys
            .iter()
            .map(|key| format!("observe:{key}"))
            .collect();
        obligations.extend(
            self.adaptations_needed
                .iter()
                .map(|key| format!("adapt:{key}")),
        );
        obligations
    }
}

/// What a `gte` condition needs, as a plain triple the oracle can reason about.
pub struct DeclaredRule {
    pub key: String,
    pub kind: &'static str,
    pub value: Option<i64>,
    /// Whether the intervention carries a parameter that can be re-derived for this key. The oracle
    /// implements the same specification as the planner, so it needs the same notion.
    pub adaptable: bool,
}

/// Compute the expected answer from the declared rules, the target, and the declared adaptations.
///
/// Written as a second implementation of the same specification over `BTreeMap<String, i64>` facts and
/// a small capability map, so it shares no code path with the planner.
pub fn oracle(
    rules: &[DeclaredRule],
    target_facts: &BTreeMap<String, AValue>,
    target_capabilities: &BTreeMap<String, &'static str>,
    declared_keys: &[&str],
    adaptation_keys: &[&str],
) -> OracleExpectation {
    if rules.is_empty() || declared_keys.is_empty() {
        return OracleExpectation {
            status: OracleStatus::InsufficientInformation,
            differing_keys: Vec::new(),
            unknown_keys: Vec::new(),
            adaptations_needed: Vec::new(),
        };
    }

    let mut differing = Vec::new();
    let mut unknown = Vec::new();
    let mut uncovered = Vec::new();

    for rule in rules {
        if rule.kind == "capability_available" {
            match target_capabilities.get(rule.key.as_str()) {
                Some(&"available") => {}
                Some(&"not_observed") | None => unknown.push(rule.key.clone()),
                Some(_) => {
                    // A missing capability is never re-parameterised, whatever the author declared.
                    differing.push(rule.key.clone());
                    if !adaptation_keys.contains(&rule.key.as_str()) {
                        uncovered.push(rule.key.clone());
                    }
                }
            }
            continue;
        }
        let Some(expected) = rule.value else {
            unknown.push(rule.key.clone());
            continue;
        };
        match target_facts.get(&rule.key) {
            // Not declared, or declared as unknown. Neither is an answer.
            None | Some(AValue::Unknown) => unknown.push(rule.key.clone()),
            // Declared and established as absent. That is an answer, and it violates a required value.
            Some(AValue::Absent) => {
                differing.push(rule.key.clone());
                if !adaptation_keys.contains(&rule.key.as_str()) {
                    uncovered.push(rule.key.clone());
                }
            }
            Some(AValue::Known(actual)) => {
                let holds = match rule.kind {
                    "equals" => *actual == expected,
                    "gte" => *actual >= expected,
                    _ => false,
                };
                if !holds {
                    differing.push(rule.key.clone());
                    if !rule.adaptable && !adaptation_keys.contains(&rule.key.as_str()) {
                        uncovered.push(rule.key.clone());
                    }
                }
            }
        }
    }

    let status = if !uncovered.is_empty() {
        OracleStatus::Incompatible
    } else if !differing.is_empty() {
        OracleStatus::AdaptationRequired
    } else if !unknown.is_empty() {
        OracleStatus::AdditionalEvidenceRequired
    } else {
        OracleStatus::DirectlyReusable
    };

    OracleExpectation {
        status,
        differing_keys: differing,
        unknown_keys: unknown,
        adaptations_needed: uncovered,
    }
}

/// Extract the planner's answer into the oracle's vocabulary, for a comparison that is about values and
/// not about types.
pub fn to_oracle(status: world_kernel::transfer::TransferStatus) -> OracleStatus {
    use world_kernel::transfer::TransferStatus;
    match status {
        TransferStatus::DirectlyReusable => OracleStatus::DirectlyReusable,
        TransferStatus::AdaptationRequired => OracleStatus::AdaptationRequired,
        TransferStatus::AdditionalEvidenceRequired => OracleStatus::AdditionalEvidenceRequired,
        TransferStatus::Incompatible => OracleStatus::Incompatible,
        TransferStatus::InsufficientInformation => OracleStatus::InsufficientInformation,
    }
}

// ---------------------------------------------------------------------------------------------
// Baseline A: retrieval plus an application gate, with no Kernel dependency.
// ---------------------------------------------------------------------------------------------

/// A field in a baseline record, with the same four states the Kernel uses. A competent baseline keeps
/// the unknown state; the M1 lesson is that a baseline which drops it is not a competitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AValue {
    Known(i64),
    Absent,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ACondition {
    pub key: String,
    pub kind: &'static str,
    pub value: i64,
    /// Whether the intervention carries a parameter that can be re-derived for this key. A baseline that
    /// omits this does not fail loudly; it reports an adaptable difference as an incompatibility, which
    /// is a false incompatibility and one of the metrics the brief names.
    pub adaptable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ACapsule {
    pub id: String,
    /// The keys retrieval filters on. Deliberately coarse: this is what a good retrieval index does.
    pub filter_keys: Vec<String>,
    pub conditions: Vec<ACondition>,
    pub capability_conditions: Vec<String>,
}

impl ACapsule {
    /// Retrieve by metadata. Returns the candidates that share a filter key, ranked by how many they
    /// share, which is the whole method and is the reason similar-looking targets retrieve confidently.
    pub fn retrieve<'a>(&'a self, target: &BTreeMap<String, AValue>) -> Vec<(&'a ACapsule, usize)> {
        let mut scored: Vec<(&ACapsule, usize)> = vec![(self, self.filter_keys.len())];
        scored.retain(|(capsule, _)| {
            capsule
                .filter_keys
                .iter()
                .all(|key| target.contains_key(key))
        });
        scored
    }
}

/// The application gate: the deterministic comparison, over the capsule's declared conditions.
///
/// Returns a status in the oracle's vocabulary plus the obligations it thinks are owed. It has no Kernel
/// types and no Kernel rules; it is the same specification written a second time, which is the point of
/// a baseline in this repository.
pub fn a_gate(
    capsule: &ACapsule,
    target: &BTreeMap<String, AValue>,
    target_capabilities: &BTreeMap<String, &'static str>,
    adaptation_keys: &[&str],
) -> OracleExpectation {
    let mut differing = Vec::new();
    let mut unknown = Vec::new();
    let mut uncovered = Vec::new();

    for condition in &capsule.conditions {
        match target.get(&condition.key) {
            None | Some(AValue::Unknown) => unknown.push(condition.key.clone()),
            Some(AValue::Absent) => {
                differing.push(condition.key.clone());
                if !condition.adaptable && !adaptation_keys.contains(&condition.key.as_str()) {
                    uncovered.push(condition.key.clone());
                }
            }
            Some(AValue::Known(actual)) => {
                let holds = match condition.kind {
                    "equals" => *actual == condition.value,
                    "gte" => *actual >= condition.value,
                    _ => false,
                };
                if !holds {
                    differing.push(condition.key.clone());
                    if !condition.adaptable && !adaptation_keys.contains(&condition.key.as_str()) {
                        uncovered.push(condition.key.clone());
                    }
                }
            }
        }
    }

    for key in &capsule.capability_conditions {
        match target_capabilities.get(key.as_str()) {
            Some(&"available") => {}
            // Not observed is unknown. A competent baseline does not turn a missing observation into a
            // refusal, because that is how a good idea becomes a false incompatibility.
            Some(&"not_observed") | None => unknown.push(key.clone()),
            Some(_) => {
                differing.push(key.clone());
                if !adaptation_keys.contains(&key.as_str()) {
                    uncovered.push(key.clone());
                }
            }
        }
    }

    if capsule.conditions.is_empty() && capsule.capability_conditions.is_empty() {
        return OracleExpectation {
            status: OracleStatus::InsufficientInformation,
            differing_keys: Vec::new(),
            unknown_keys: Vec::new(),
            adaptations_needed: Vec::new(),
        };
    }

    let status = if !uncovered.is_empty() {
        OracleStatus::Incompatible
    } else if !differing.is_empty() {
        OracleStatus::AdaptationRequired
    } else if !unknown.is_empty() {
        OracleStatus::AdditionalEvidenceRequired
    } else {
        OracleStatus::DirectlyReusable
    };

    OracleExpectation {
        status,
        differing_keys: differing,
        unknown_keys: unknown,
        adaptations_needed: uncovered,
    }
}

/// The baseline's view of a context: a flat map, with the four states preserved.
pub fn a_view(
    context: &ContextSnapshot,
) -> (BTreeMap<String, AValue>, BTreeMap<String, &'static str>) {
    let mut facts = BTreeMap::new();
    for (key, fact) in &context.facts {
        let value = match fact {
            Fact::Known(serde_json::Value::Number(number)) => number.as_i64().map(AValue::Known),
            Fact::Absent => Some(AValue::Absent),
            // Unknown, and anything the baseline cannot represent, stays unknown. A baseline that
            // coerced an unrepresentable value into a fact would be inventing evidence.
            Fact::Known(_) | Fact::Unknown => None,
        };
        facts.insert(key.clone(), value.unwrap_or(AValue::Unknown));
    }
    let mut capabilities = BTreeMap::new();
    for (key, state) in &context.capabilities {
        capabilities.insert(
            key.clone(),
            match state {
                CapabilityState::Available => "available",
                CapabilityState::Unavailable => "unavailable",
                CapabilityState::Unsupported => "unsupported",
                CapabilityState::NotObserved => "not_observed",
            },
        );
    }
    (facts, capabilities)
}
