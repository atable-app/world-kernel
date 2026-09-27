//! M5 tranche 1: the closed corpus, with a competent baseline measured beside the Kernel.
//!
//! The families are the brief's: exact, adaptable, deceptive similarity, and insufficient or unknown.
//! The safety metric the brief names is a **false direct transfer**, and on a closed corpus the Kernel
//! and the baseline must both be zero.
//!
//! What this measurement cannot settle, and says so: a tie here says the two systems agree on this
//! corpus, not that the Kernel is worthless or valuable. The line counts are in the artifact.

mod support;

use std::collections::BTreeMap;

use serde_json::json;
use support::transfer_core::{DeclaredAdaptation, TransferStatus, plan_transfer};
use support::transfer_fixture as fixture;
use world_kernel::experience::{
    ApplicabilityCondition, ApplicabilityCoverage, CapabilityState, ConditionKind, ContextSnapshot,
    ExperienceCapsule, ExperienceOutcome, Fact, ProvenanceRecord,
};

/// One closed case. The expectation is derived from the case's declared shape, never read from the
/// system under test.
struct Case {
    family: &'static str,
    name: String,
    capsule: ExperienceCapsule,
    target: ContextSnapshot,
    adaptation_keys: Vec<String>,
    expected: TransferStatus,
}

fn capsule(
    id: &str,
    conditions: Vec<ApplicabilityCondition>,
    context: ContextSnapshot,
) -> ExperienceCapsule {
    let mut context = context;
    context.coverage = ApplicabilityCoverage::ClosedDeclared;
    ExperienceCapsule::new(
        id,
        1,
        context,
        world_kernel::experience::Intervention::new("a problem", "an intervention"),
        conditions,
        ExperienceOutcome {
            verdict: "accepted".to_owned(),
            detail: "in the source".to_owned(),
            limits: vec!["established in the source only".to_owned()],
        },
        ProvenanceRecord {
            author: "test".to_owned(),
            activity: "benchmark".to_owned(),
            source_revision: 1,
            used_refs: Vec::new(),
        },
    )
    .with_dependency("observed_key")
}

fn context(
    id: &str,
    pairs: &[(&str, i64)],
    capabilities: &[(&str, CapabilityState)],
) -> ContextSnapshot {
    let mut snapshot = ContextSnapshot::new(id, 1);
    for (key, value) in pairs {
        snapshot = snapshot.with_fact(key, Fact::Known(json!(value)));
    }
    for (key, state) in capabilities {
        snapshot = snapshot.with_capability(key, state.clone());
    }
    snapshot
}

/// The closed corpus. Four families, generated from the shapes rather than hand-listed, so the counts
/// are the same structure the brief describes without being a claim of statistical power.
fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();

    // Family A, exact transfer: every declared condition holds.
    for index in 0..6 {
        let key = format!("load_{index}");
        let source = context("world:a-src", &[("capacity", 100), (&key, 80)], &[]);
        let target = context("world:a-tgt", &[("capacity", 100), (&key, 80)], &[]);
        cases.push(Case {
            family: "A exact",
            name: format!("exact {key}"),
            capsule: capsule(
                "exp/a",
                vec![ApplicabilityCondition::equals("capacity", json!(100))],
                source,
            ),
            target,
            adaptation_keys: Vec::new(),
            expected: TransferStatus::DirectlyReusable,
        });
    }

    // Family B, adaptable: a declared parameter moved.
    for index in 0..6 {
        let key = format!("minimum_reserve_{index}");
        let source = context("world:b-src", &[(&key, 10)], &[]);
        let target = context("world:b-tgt", &[(&key, 30)], &[]);
        cases.push(Case {
            family: "B adaptable",
            name: format!("adaptable {key}"),
            capsule: capsule(
                "exp/b",
                vec![ApplicabilityCondition::adaptable_equals(&key, json!(10))],
                source,
            ),
            target,
            adaptation_keys: Vec::new(),
            expected: TransferStatus::AdaptationRequired,
        });
    }

    // Family C, deceptive similarity: the shared key is the loud one, the conflicting one is quiet.
    for index in 0..6 {
        let capability = format!("cap.{index}");
        let source = context(
            "world:c-src",
            &[("framework", index as i64)],
            &[(capability.as_str(), CapabilityState::Available)],
        );
        let target = context(
            "world:c-tgt",
            &[("framework", index as i64)],
            &[(capability.as_str(), CapabilityState::Unsupported)],
        );
        cases.push(Case {
            family: "C deceptive",
            name: format!("deceptive {capability}"),
            capsule: capsule(
                "exp/c",
                vec![
                    ApplicabilityCondition::equals("framework", json!(index as i64)),
                    ApplicabilityCondition::capability_available(&capability),
                ],
                source,
            ),
            target,
            adaptation_keys: Vec::new(),
            expected: TransferStatus::Incompatible,
        });
    }

    // Family D, insufficient or unknown: nobody looked, so nobody may conclude.
    for index in 0..6 {
        let capability = format!("pending.{index}");
        let source = context(
            "world:d-src",
            &[("framework", index as i64)],
            &[(capability.as_str(), CapabilityState::Available)],
        );
        let mut target = context("world:d-tgt", &[("framework", index as i64)], &[]);
        target = target.with_capability(capability.as_str(), CapabilityState::NotObserved);
        cases.push(Case {
            family: "D unknown",
            name: format!("unknown {capability}"),
            capsule: capsule(
                "exp/d",
                vec![
                    ApplicabilityCondition::equals("framework", json!(index as i64)),
                    ApplicabilityCondition::capability_available(&capability),
                ],
                source,
            ),
            target,
            adaptation_keys: Vec::new(),
            expected: TransferStatus::AdditionalEvidenceRequired,
        });
    }

    // Family D, the insufficient half: the source itself declares nothing.
    for index in 0..6 {
        let source = context("world:e-src", &[("k", index as i64)], &[]);
        let target = context("world:e-tgt", &[("k", index as i64)], &[]);
        cases.push(Case {
            family: "D unknown",
            name: format!("no declared conditions {index}"),
            capsule: capsule("exp/e", vec![], source),
            target,
            adaptation_keys: Vec::new(),
            expected: TransferStatus::InsufficientInformation,
        });
    }

    cases
}

#[test]
fn the_kernel_produces_no_false_direct_transfer_on_the_closed_corpus() {
    let mut false_direct = Vec::new();
    for case in corpus() {
        let adaptations: Vec<DeclaredAdaptation> = case
            .adaptation_keys
            .iter()
            .map(|key| DeclaredAdaptation {
                key: key.clone(),
                detail: format!("bridge {key}"),
            })
            .collect();
        let plan = plan_transfer(&case.capsule, &case.target, &adaptations);
        if plan.status == TransferStatus::DirectlyReusable
            && case.expected != TransferStatus::DirectlyReusable
        {
            false_direct.push(case.name.clone());
        }
    }
    assert!(
        false_direct.is_empty(),
        "a false direct transfer is the one unacceptable outcome: {false_direct:?}"
    );
}

#[test]
fn the_kernel_reaches_the_expected_status_on_every_case() {
    for case in corpus() {
        let adaptations: Vec<DeclaredAdaptation> = case
            .adaptation_keys
            .iter()
            .map(|key| DeclaredAdaptation {
                key: key.clone(),
                detail: format!("bridge {key}"),
            })
            .collect();
        let plan = plan_transfer(&case.capsule, &case.target, &adaptations);
        assert_eq!(
            plan.status, case.expected,
            "{}: the planner disagreed with the case's declared shape",
            case.name
        );
    }
}

#[test]
fn the_competent_baseline_ties_the_kernel_and_the_tie_is_recorded() {
    let mut disagreements = Vec::new();
    let mut compared = 0;
    for case in corpus() {
        // The baseline's view of the same declared conditions.
        let (facts, capabilities) = fixture::a_view(&case.target);
        let a_capsule = fixture::ACapsule {
            id: case.capsule.id.clone(),
            filter_keys: case.capsule.source_context.facts.keys().cloned().collect(),
            conditions: case
                .capsule
                .applicability_conditions
                .iter()
                .filter(|condition| condition.kind != ConditionKind::CapabilityAvailable)
                .map(|condition| fixture::ACondition {
                    key: condition.key.clone(),
                    kind: match condition.kind {
                        ConditionKind::Gte => "gte",
                        _ => "equals",
                    },
                    value: condition
                        .expected
                        .as_ref()
                        .and_then(|value| value.as_i64())
                        .unwrap_or_default(),
                    adaptable: condition.adaptable,
                })
                .collect(),
            capability_conditions: case
                .capsule
                .applicability_conditions
                .iter()
                .filter(|condition| condition.kind == ConditionKind::CapabilityAvailable)
                .map(|condition| condition.key.clone())
                .collect(),
        };
        let adaptations: Vec<&str> = case.adaptation_keys.iter().map(String::as_str).collect();
        let baseline = fixture::a_gate(&a_capsule, &facts, &capabilities, &adaptations);
        let plan = plan_transfer(&case.capsule, &case.target, &[]);
        compared += 1;
        if fixture::to_oracle(plan.status) != baseline.status {
            disagreements.push((case.name.clone(), plan.status, baseline.status));
        }
    }
    assert!(
        disagreements.is_empty(),
        "a competent baseline must not disagree, and a disagreement means one of them is wrong: {disagreements:?}"
    );
    assert_eq!(compared, 30, "30 closed cases across four families");
}

#[test]
fn every_case_carries_its_target_assurance_obligation() {
    for case in corpus() {
        let plan = plan_transfer(&case.capsule, &case.target, &[]);
        assert_eq!(
            plan.required_assurances.len(),
            1,
            "{}: nothing skips target assurance",
            case.name
        );
        assert_eq!(
            plan.required_assurances[0].bound_to, case.target.context_id,
            "{}: assurance is bound to the target",
            case.name
        );
    }
}

/// The secondary measurement the brief asks for, stated honestly: how much target work the plan avoids
/// and what it costs in lines. Not converted to money or time.
#[test]
fn the_delta_reports_avoided_work_rather_than_a_safety_score() {
    let mut same_total = 0;
    let mut different_total = 0;
    let mut unknown_total = 0;
    for case in corpus() {
        let plan = plan_transfer(&case.capsule, &case.target, &[]);
        same_total += plan.delta.same.len();
        different_total += plan.delta.different.len();
        unknown_total += plan.delta.unknown.len();
    }
    // The exact family contributes matches; the deceptive family contributes capability differences; the
    // unknown family contributes unobserved conditions. Nothing contributes a fabricated match.
    assert!(
        same_total >= 6,
        "at least the exact family matched something: {same_total}"
    );
    assert!(
        different_total >= 6,
        "at least the deceptive family differed: {different_total}"
    );
    assert!(
        unknown_total >= 6,
        "at least the unknown family stayed unknown: {unknown_total}"
    );

    let families: BTreeMap<&str, usize> =
        corpus().iter().fold(BTreeMap::new(), |mut counts, case| {
            *counts.entry(case.family).or_insert(0) += 1;
            counts
        });
    assert_eq!(families.get("A exact"), Some(&6));
    assert_eq!(families.get("B adaptable"), Some(&6));
    assert_eq!(families.get("C deceptive"), Some(&6));
    assert_eq!(families.get("D unknown"), Some(&12));
}

/// The one capability difference this corpus found, stated as a measurement rather than a claim.
///
/// A baseline gate that does not model a **declared parameter** reports an adaptable difference as an
/// incompatibility. That is a false incompatibility: it refuses work that was safe to do. The Kernel does
/// not make this mistake, because adaptability is a field on the condition and defaults to false, so the
/// conservative answer is the one you get by not declaring it.
///
/// This is recorded as a found difference, not as a win. It is one boolean on one struct, and a competent
/// baseline adds it, as the tie above already shows.
#[test]
fn a_baseline_without_declared_parameters_reports_false_incompatibilities() {
    let mut false_incompatibility = 0;
    let mut cases_with_an_adaptable_difference = 0;

    for case in corpus() {
        let plan = plan_transfer(&case.capsule, &case.target, &[]);
        if plan.status != TransferStatus::AdaptationRequired {
            continue;
        }
        cases_with_an_adaptable_difference += 1;

        let (facts, capabilities) = fixture::a_view(&case.target);
        // The same gate, with the one concept missing.
        let naive = fixture::ACapsule {
            id: case.capsule.id.clone(),
            filter_keys: case.capsule.source_context.facts.keys().cloned().collect(),
            conditions: case
                .capsule
                .applicability_conditions
                .iter()
                .filter(|condition| condition.kind != ConditionKind::CapabilityAvailable)
                .map(|condition| fixture::ACondition {
                    key: condition.key.clone(),
                    kind: "equals",
                    value: condition
                        .expected
                        .as_ref()
                        .and_then(|value| value.as_i64())
                        .unwrap_or_default(),
                    adaptable: false,
                })
                .collect(),
            capability_conditions: case
                .capsule
                .applicability_conditions
                .iter()
                .filter(|condition| condition.kind == ConditionKind::CapabilityAvailable)
                .map(|condition| condition.key.clone())
                .collect(),
        };
        if fixture::a_gate(&naive, &facts, &capabilities, &[]).status
            == fixture::OracleStatus::Incompatible
        {
            false_incompatibility += 1;
        }
    }

    assert_eq!(
        cases_with_an_adaptable_difference, 6,
        "the adaptable family is where this shows up"
    );
    assert_eq!(
        false_incompatibility, 6,
        "a gate without the concept refuses every adaptable difference, which is the metric the brief warns about"
    );
}
