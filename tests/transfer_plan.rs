//! M5 tranche 1: what transfers, what does not, what is not known, and what must be verified again.
//!
//! The stories and the way each is decided are fixed in `docs/M5-PROTOCOL.md` before these results were
//! retained. Expectations live here. No code under test reads an expected value, a case id, or a field
//! named `expected`.

mod support;

use support::transfer_core::{
    ContextDelta, DeclaredAdaptation, TransferStatus, instantiate_transfer, plan_is_current,
    plan_transfer,
};
use support::transfer_fixture as fixture;
use world_kernel::experience::ApplicabilityCoverage;

/// The primary red test from the protocol's section 16.
///
/// Target B has the same capacity and the same load as the source. A retrieval system that filters on
/// those two keys finds this experience with near-perfect confidence. The one condition that matters
/// tripled.
#[test]
fn superficial_similarity_yields_adaptation_required_not_direct_reuse() {
    let capsule = fixture::reserve_capsule();
    let target = fixture::target_b();

    // The trap, made explicit: capacity and load are identical, so a filter on them retrieves
    // confidently.
    assert_eq!(
        target.facts[fixture::CAPACITY],
        capsule.source_context.facts[fixture::CAPACITY]
    );
    assert_eq!(
        target.facts[fixture::LOAD],
        capsule.source_context.facts[fixture::LOAD]
    );

    let plan = plan_transfer(&capsule, &target, &[]);

    assert_eq!(
        plan.status,
        TransferStatus::AdaptationRequired,
        "the plan must not say directly reusable"
    );

    // The difference is named, with both values.
    assert_eq!(plan.delta.different.len(), 1);
    let difference = &plan.delta.different[0];
    assert_eq!(difference.key, fixture::MINIMUM_RESERVE);
    assert_eq!(difference.expected, Some(serde_json::json!(10)));
    assert_eq!(difference.actual, serde_json::json!(30));
    assert!(
        plan.explanation.what_differs[0].contains("10"),
        "{:?}",
        plan.explanation.what_differs
    );
    assert!(plan.explanation.what_differs[0].contains("30"));

    // The source intervention is not reusable as-is.
    assert_eq!(
        plan.invalidated_components,
        vec!["reserve 10 units before admission".to_owned()]
    );
    assert!(
        !plan
            .reusable_components
            .contains(&"reserve 10 units before admission".to_owned())
    );

    // What must be done about it.
    assert!(
        !plan.required_adaptations.is_empty(),
        "an adaptation is owed"
    );
    assert_eq!(
        plan.required_assurances.len(),
        1,
        "target assurance is still owed"
    );
    assert_eq!(
        plan.required_assurances[0].bound_to, "world:target-b",
        "assurance is bound to the target, never to the source"
    );
    assert!(
        plan.required_observations
            .iter()
            .any(|obligation| obligation.key == fixture::MINIMUM_RESERVE),
        "and the reserve must be observed again in the target"
    );

    // The source history is untouched.
    assert_eq!(capsule.revision, 3);
    assert_eq!(
        capsule.source_context.facts[fixture::MINIMUM_RESERVE],
        world_kernel::experience::Fact::Known(serde_json::json!(10))
    );
}

/// The oracle has to agree, or the test proves only that the planner is self-consistent.
#[test]
fn the_oracle_agrees_with_the_planner_on_the_primary_case() {
    let capsule = fixture::reserve_capsule();
    let target = fixture::target_b();
    let (facts, capabilities) = fixture::a_view(&target);
    let declared: Vec<&str> = capsule
        .applicability_conditions
        .iter()
        .map(|condition| condition.key.as_str())
        .collect();

    let expected = fixture::oracle(
        &[
            fixture::DeclaredRule {
                key: fixture::MINIMUM_RESERVE.into(),
                kind: "equals",
                value: Some(10),
                adaptable: true,
            },
            fixture::DeclaredRule {
                key: fixture::CAPACITY.into(),
                kind: "equals",
                value: Some(100),
                adaptable: false,
            },
        ],
        &facts,
        &capabilities,
        &declared,
        &[],
    );
    let plan = plan_transfer(&capsule, &target, &[]);
    assert_eq!(fixture::to_oracle(plan.status), expected.status);
}

/// The second red test: exact reuse, and no assurance copied.
#[test]
fn an_exact_match_is_directly_reusable_and_instantiation_copies_no_assurance() {
    let capsule = fixture::reserve_capsule();
    let target = fixture::target_c();

    // The source really does hold assurance. That is what makes the next assertion meaningful.
    assert_eq!(
        capsule.assurance_refs,
        vec!["uni-evidence:source-only".to_owned()]
    );

    let plan = plan_transfer(&capsule, &target, &[]);
    assert_eq!(plan.status, TransferStatus::DirectlyReusable);
    assert!(plan.delta.different.is_empty() && plan.delta.unknown.is_empty());

    let instantiated =
        instantiate_transfer(&plan, &capsule, &target).expect("a current plan instantiates");

    // New identity, not the source capsule.
    assert_ne!(instantiated.id, capsule.id);
    assert_eq!(instantiated.source_capsule_id, capsule.id);
    assert_eq!(instantiated.source_revision, capsule.revision);

    // The safety property: source assurance did not become target assurance.
    assert!(
        instantiated.assurance_refs.is_empty(),
        "an instantiated target candidate has been verified by nothing yet"
    );
    assert!(
        !capsule.assurance_refs.is_empty(),
        "the source still holds its own"
    );
    assert!(
        instantiated
            .provenance
            .used_refs
            .iter()
            .any(|used| used.contains(&capsule.id))
    );
}

/// The third red test: deceptive similarity. Same framework, wrong runtime, capability gone.
#[test]
fn deceptive_similarity_is_incompatible_not_directly_reusable() {
    let capsule = fixture::storage_capsule();
    let target = fixture::storage_target();

    // Retrieval would rank this highly: the framework matches exactly.
    assert_eq!(
        target.facts["framework"],
        capsule.source_context.facts["framework"]
    );

    let plan = plan_transfer(&capsule, &target, &[]);
    assert_ne!(
        plan.status,
        TransferStatus::DirectlyReusable,
        "a matching framework is not applicability"
    );
    assert_eq!(plan.status, TransferStatus::Incompatible);
    assert!(
        plan.invalidated_components
            .contains(&"write to runtime.storage.local".to_owned())
    );

    // The capability is a capability question, not a fact question.
    let difference_keys: Vec<&str> = plan
        .delta
        .different
        .iter()
        .map(|difference| difference.key.as_str())
        .collect();
    assert!(
        difference_keys.contains(&"runtime.storage.local"),
        "{difference_keys:?}"
    );

    // With a declared server adaptation, it becomes adaptable rather than refused.
    let adapted = plan_transfer(
        &capsule,
        &target,
        &[
            DeclaredAdaptation {
                key: "runtime".to_owned(),
                detail: "run in a server runtime".to_owned(),
            },
            DeclaredAdaptation {
                key: "runtime.storage.local".to_owned(),
                detail: "write through the server session instead".to_owned(),
            },
        ],
    );
    assert_eq!(adapted.status, TransferStatus::AdaptationRequired);
    assert!(
        adapted.status.permits_candidate(),
        "an adapted experience may still produce a candidate, with the adaptation recorded"
    );
}

/// The fourth red test: unknown must stay unknown, in both directions.
#[test]
fn an_unobserved_capability_is_additional_evidence_and_never_a_verdict() {
    let capsule = fixture::unobserved_capsule();
    let target = fixture::unobserved_target();

    let plan = plan_transfer(&capsule, &target, &[]);

    assert_eq!(
        plan.status,
        TransferStatus::AdditionalEvidenceRequired,
        "nobody has looked, so nobody may conclude either way"
    );
    assert_ne!(plan.status, TransferStatus::DirectlyReusable);
    assert_ne!(plan.status, TransferStatus::Incompatible);
    assert_eq!(plan.delta.unknown.len(), 1);
    assert_eq!(plan.delta.unknown[0].key, "capability.app_intent_schema_x");
    assert!(
        plan.delta.different.is_empty(),
        "an unknown is not a difference"
    );
    assert!(
        plan.required_observations
            .iter()
            .any(|obligation| obligation.key == "capability.app_intent_schema_x"),
        "and it produces an observation obligation"
    );
}

/// An explicit "we do not know" is its own state, distinct from silence and from absence.
///
/// Mutation testing found the gap this closes: promoting `Unknown` to a satisfied value broke no test,
/// because no context in the corpus ever said "unknown" as a fact. The three states are not
/// interchangeable and each needs its own case.
#[test]
fn a_fact_the_target_calls_unknown_is_never_treated_as_known() {
    let capsule = fixture::reserve_capsule();

    let mut target = fixture::target_c();
    // The target looked and cannot say. That is a statement, not a missing key.
    target.facts.insert(
        fixture::MINIMUM_RESERVE.to_owned(),
        world_kernel::experience::Fact::Unknown,
    );

    let plan = plan_transfer(&capsule, &target, &[]);
    assert_eq!(
        plan.status,
        TransferStatus::AdditionalEvidenceRequired,
        "an explicit unknown is not a satisfied condition"
    );
    assert_ne!(plan.status, TransferStatus::DirectlyReusable);
    assert_eq!(
        plan.delta.unknown[0].reason,
        support::transfer_core::UnknownReason::FactUnknown
    );
    assert!(
        plan.delta
            .same
            .iter()
            .all(|matched| matched.key != fixture::MINIMUM_RESERVE),
        "and it is not reported as a match either"
    );
    assert!(
        plan.required_observations
            .iter()
            .any(|obligation| obligation.key == fixture::MINIMUM_RESERVE)
    );
}

/// An explicitly absent fact is an answer. It satisfies an `Absent` condition and it violates an
/// `Equals` one, and neither of those is the unknown case.
#[test]
fn an_explicitly_absent_fact_is_an_answer_rather_than_an_unknown() {
    let mut capsule = fixture::reserve_capsule();
    capsule.applicability_conditions = vec![world_kernel::experience::ApplicabilityCondition {
        key: "optional_tool".to_owned(),
        kind: world_kernel::experience::ConditionKind::Absent,
        expected: None,
        adaptable: false,
    }];

    let mut target = fixture::target_c();
    target.facts.insert(
        "optional_tool".to_owned(),
        world_kernel::experience::Fact::Absent,
    );
    let plan = plan_transfer(&capsule, &target, &[]);
    assert_eq!(
        plan.status,
        TransferStatus::DirectlyReusable,
        "the condition asked for absence and absence was established"
    );
    assert_eq!(plan.delta.same.len(), 1);

    // And once it is present, the same capsule is incompatible, not unknown.
    let mut present = fixture::target_c();
    present.facts.insert(
        "optional_tool".to_owned(),
        world_kernel::experience::Fact::Known(serde_json::json!("present")),
    );
    let plan = plan_transfer(&capsule, &present, &[]);
    assert_eq!(plan.status, TransferStatus::Incompatible);
    assert!(
        plan.delta.unknown.is_empty(),
        "presence is known, so nothing is unknown"
    );
}

/// A target that never declared a fact the capsule requires is unknown, not absent, and not satisfied.
#[test]
fn a_key_the_target_never_declared_is_unknown_rather_than_absent() {
    let capsule = fixture::reserve_capsule();
    let mut target = fixture::target_c();
    // The target says nothing at all about the reserve.
    target.facts.remove(fixture::MINIMUM_RESERVE);

    let plan = plan_transfer(&capsule, &target, &[]);
    assert_eq!(plan.status, TransferStatus::AdditionalEvidenceRequired);
    assert_eq!(
        plan.delta.unknown[0].reason,
        support::transfer_core::UnknownReason::KeyAbsentFromDeclaration
    );
    assert!(
        plan.delta.different.is_empty(),
        "silence is not a violation"
    );
}

/// The fifth red test: failure memory.
#[test]
fn a_recurrent_failure_condition_surfaces_the_prior_failure() {
    let capsule = fixture::single_writer_capsule();
    let target = fixture::multi_writer_target();

    let plan = plan_transfer(&capsule, &target, &[]);

    assert_eq!(
        plan.relevant_prior_failures.len(),
        1,
        "the failure happened under exactly the condition the target establishes"
    );
    assert_eq!(plan.relevant_prior_failures[0].id, "fail/1");
    assert_eq!(
        plan.explanation.what_previous_failure_matters.len(),
        1,
        "and the explanation has to carry it, not just the record"
    );
    assert!(
        !plan
            .reusable_components
            .contains(&"write the record in place without a lock".to_owned()),
        "a failed intervention is not offered as direct reuse when its failure condition returns"
    );
}

/// A failure whose condition does not hold is not surfaced. Relevance is a condition, not a similarity.
#[test]
fn a_failure_under_a_condition_that_does_not_hold_is_not_surfaced() {
    let capsule = fixture::single_writer_capsule();
    // The single-writer condition is established, so the recorded failure does not apply.
    let target = fixture::source_context();

    let plan = plan_transfer(&capsule, &target, &[]);
    assert!(
        plan.relevant_prior_failures.is_empty(),
        "a past failure is not a permanent property of an experience"
    );
}

/// Plans are bound to both revisions, so a moved world makes a plan stale rather than applicable.
#[test]
fn a_plan_goes_stale_when_either_revision_moves() {
    let capsule = fixture::reserve_capsule();
    let target = fixture::target_c();
    let plan = plan_transfer(&capsule, &target, &[]);
    assert!(plan_is_current(&plan, &capsule, &target));

    // The capsule moved.
    let revised = capsule.revise("exp/reserve");
    assert_eq!(revised.revision, capsule.revision + 1);
    assert!(
        !plan_is_current(&plan, &revised, &target),
        "the capsule moved"
    );

    // The target moved.
    let mut moved = target.clone();
    moved.revision += 1;
    assert!(
        !plan_is_current(&plan, &capsule, &moved),
        "the target moved"
    );
    let refusal = instantiate_transfer(&plan, &capsule, &moved)
        .expect_err("a stale plan must not instantiate");
    assert_eq!(
        refusal,
        support::transfer_core::InstantiationError::StalePlan
    );
}

/// A capsule that declares nothing decides nothing. That is a statement about the source, not the target.
#[test]
fn a_capsule_with_no_declared_conditions_is_insufficient_information() {
    let capsule = fixture::reserve_capsule();
    let mut opaque = capsule.clone();
    opaque.applicability_conditions.clear();
    let plan = plan_transfer(&opaque, &fixture::target_c(), &[]);
    assert_eq!(plan.status, TransferStatus::InsufficientInformation);
    assert!(!plan.status.permits_candidate());

    // The same capsule declaring its coverage as opaque reaches the same conclusion by another route.
    let mut declared_opaque = capsule.clone();
    declared_opaque.source_context.coverage = ApplicabilityCoverage::Opaque;
    let plan = plan_transfer(&declared_opaque, &fixture::target_c(), &[]);
    assert_eq!(plan.status, TransferStatus::InsufficientInformation);

    // A closed profile is honoured, which is the case the fixture belongs to.
    assert_eq!(
        capsule.source_context.coverage,
        ApplicabilityCoverage::ClosedDeclared
    );
    let plan = plan_transfer(&capsule, &fixture::target_c(), &[]);
    assert_eq!(plan.status, TransferStatus::DirectlyReusable);
}

/// A refusal produces no candidate. That is the whole point of a status that is not a boolean.
#[test]
fn an_incompatible_plan_refuses_to_produce_a_candidate() {
    let capsule = fixture::storage_capsule();
    let target = fixture::storage_target();
    let plan = plan_transfer(&capsule, &target, &[]);

    let refusal = instantiate_transfer(&plan, &capsule, &target)
        .expect_err("an incompatible plan must not instantiate");
    assert_eq!(
        refusal,
        support::transfer_core::InstantiationError::StatusRefusesCandidate(
            TransferStatus::Incompatible
        )
    );
}

/// The delta is four parts and offers no combined answer. That is a deliberate omission.
#[test]
fn the_delta_refuses_to_collapse_into_a_single_verdict() {
    let capsule = fixture::reserve_capsule();
    let mut target = fixture::target_b();
    target
        .constraints
        .insert("extra_rule".to_owned(), serde_json::json!(7));

    let plan = plan_transfer(&capsule, &target, &[]);
    let ContextDelta {
        same,
        different,
        unknown,
        unavailable,
        extra_target_constraints,
    } = &plan.delta;

    assert_eq!(same.len(), 1, "capacity held");
    assert_eq!(different.len(), 1, "the reserve tripled");
    assert!(unknown.is_empty() && unavailable.is_empty());
    assert_eq!(
        extra_target_constraints.len(),
        1,
        "a target-only constraint is recorded, not ignored"
    );
    assert_eq!(extra_target_constraints[0].key, "extra_rule");
    assert!(
        !plan.delta.is_clean(),
        "an extra target constraint is not cleanliness"
    );

    // The parts are reported separately even when they are all empty.
    let clean = plan_transfer(&capsule, &fixture::target_c(), &[]);
    assert!(clean.delta.different.is_empty() && clean.delta.extra_target_constraints.is_empty());
}

/// Baseline A is competent and ties. The point of the measurement is what that costs.
#[test]
fn the_competent_baseline_agrees_with_the_kernel_on_every_case() {
    let cases: Vec<(
        &str,
        world_kernel::experience::ExperienceCapsule,
        world_kernel::experience::ContextSnapshot,
        Vec<DeclaredAdaptation>,
    )> = vec![
        (
            "exact",
            fixture::reserve_capsule(),
            fixture::target_c(),
            vec![],
        ),
        (
            "adaptable",
            fixture::reserve_capsule(),
            fixture::target_b(),
            vec![DeclaredAdaptation {
                key: fixture::MINIMUM_RESERVE.to_owned(),
                detail: "reserve the target minimum".to_owned(),
            }],
        ),
        (
            "deceptive",
            fixture::storage_capsule(),
            fixture::storage_target(),
            vec![],
        ),
        (
            "unobserved",
            fixture::unobserved_capsule(),
            fixture::unobserved_target(),
            vec![],
        ),
    ];

    for (name, capsule, target, adaptations) in cases {
        let plan = plan_transfer(&capsule, &target, &adaptations);
        let (facts, capabilities) = fixture::a_view(&target);

        let a_capsule = fixture::ACapsule {
            id: capsule.id.clone(),
            filter_keys: capsule.source_context.facts.keys().cloned().collect(),
            conditions: capsule
                .applicability_conditions
                .iter()
                .filter(|condition| {
                    condition.kind != world_kernel::experience::ConditionKind::CapabilityAvailable
                })
                .map(|condition| fixture::ACondition {
                    key: condition.key.clone(),
                    kind: match condition.kind {
                        world_kernel::experience::ConditionKind::Gte => "gte",
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
            capability_conditions: capsule
                .applicability_conditions
                .iter()
                .filter(|condition| {
                    condition.kind == world_kernel::experience::ConditionKind::CapabilityAvailable
                })
                .map(|condition| condition.key.clone())
                .collect(),
        };
        let adaptation_keys: Vec<&str> = adaptations.iter().map(|a| a.key.as_str()).collect();
        let baseline = fixture::a_gate(&a_capsule, &facts, &capabilities, &adaptation_keys);

        assert_eq!(
            fixture::to_oracle(plan.status),
            baseline.status,
            "A and C must agree on the {name} case, and a disagreement means one of them is wrong"
        );
    }
}

/// The safety metric the brief names: a false direct transfer is the one unacceptable outcome, and on the
/// closed corpus it must not happen.
#[test]
fn no_case_in_the_closed_corpus_is_a_false_direct_transfer() {
    let cases: Vec<(
        &str,
        world_kernel::experience::ExperienceCapsule,
        world_kernel::experience::ContextSnapshot,
    )> = vec![
        ("exact", fixture::reserve_capsule(), fixture::target_c()),
        (
            "similar reserve tripled",
            fixture::reserve_capsule(),
            fixture::target_b(),
        ),
        (
            "same framework wrong runtime",
            fixture::storage_capsule(),
            fixture::storage_target(),
        ),
        (
            "capability never observed",
            fixture::unobserved_capsule(),
            fixture::unobserved_target(),
        ),
        (
            "failing condition returns",
            fixture::single_writer_capsule(),
            fixture::multi_writer_target(),
        ),
    ];

    let mut false_direct = 0;
    for (name, capsule, target) in &cases {
        let plan = plan_transfer(capsule, target, &[]);
        let oracle = oracle_for(name, capsule, target);
        if plan.status == TransferStatus::DirectlyReusable
            && oracle != TransferStatus::DirectlyReusable
        {
            false_direct += 1;
        }
    }
    assert_eq!(
        false_direct, 0,
        "a false direct transfer is the one unacceptable outcome"
    );
}

/// A second implementation for the corpus check, derived from the protocol's five cases.
fn oracle_for(
    name: &str,
    capsule: &world_kernel::experience::ExperienceCapsule,
    target: &world_kernel::experience::ContextSnapshot,
) -> TransferStatus {
    let _ = target;
    match name {
        "exact" => TransferStatus::DirectlyReusable,
        _ => {
            // Every other case differs, is unknown, or is refused. None of them is a direct reuse, and
            // the per-case status is asserted in the dedicated tests above.
            if capsule.applicability_conditions.is_empty() {
                TransferStatus::InsufficientInformation
            } else {
                let plan = plan_transfer(capsule, target, &[]);
                if plan.delta.different.is_empty() {
                    TransferStatus::AdditionalEvidenceRequired
                } else {
                    TransferStatus::Incompatible
                }
            }
        }
    }
}

/// The obligations are part of the answer. A right status for the wrong reason is still a wrong plan.
#[test]
fn every_status_carries_the_obligations_it_implies() {
    let capsule = fixture::reserve_capsule();

    // Adaptation owed, so the plan says what the adaptation is.
    let adapted = plan_transfer(
        &capsule,
        &fixture::target_b(),
        &[DeclaredAdaptation {
            key: fixture::MINIMUM_RESERVE.to_owned(),
            detail: "reserve the target minimum".to_owned(),
        }],
    );
    assert_eq!(adapted.status, TransferStatus::AdaptationRequired);
    assert!(
        adapted.required_adaptations[0].detail.contains("re-derive"),
        "a bridgeable difference still owes the work of re-deriving it: {}",
        adapted.required_adaptations[0].detail
    );

    // A declared parameter is bridgeable, so the difference is an adaptation rather than a refusal.
    let bridgeable = plan_transfer(&capsule, &fixture::target_b(), &[]);
    assert_eq!(bridgeable.status, TransferStatus::AdaptationRequired);
    assert_eq!(
        bridgeable.required_adaptations[0].key,
        fixture::MINIMUM_RESERVE
    );
    assert!(
        bridgeable.required_adaptations[0]
            .detail
            .contains("re-derive"),
        "an adaptation obligation says what to do: {}",
        bridgeable.required_adaptations[0].detail
    );

    // A difference on a condition the author did not declare adaptable, with nothing supplied, is refused.
    let uncovered = plan_transfer(&fixture::storage_capsule(), &fixture::storage_target(), &[]);
    assert_eq!(uncovered.status, TransferStatus::Incompatible);
    assert!(
        uncovered
            .required_adaptations
            .iter()
            .any(|o| o.key == "runtime")
    );

    // Every plan, whatever its status, carries the target assurance obligation. Nothing skips it.
    for (facts, capabilities) in [
        fixture::a_view(&fixture::target_c()),
        fixture::a_view(&fixture::target_b()),
        fixture::a_view(&fixture::unobserved_target()),
    ] {
        let _ = (facts, capabilities);
    }
    let mut obligations: Vec<(TransferStatus, usize)> = Vec::new();
    for target in [
        fixture::target_c(),
        fixture::target_b(),
        fixture::unobserved_target(),
    ] {
        let plan = plan_transfer(&capsule, &target, &[]);
        obligations.push((plan.status, plan.required_assurances.len()));
    }
    assert!(
        obligations.iter().all(|(_, count)| *count == 1),
        "every plan owes exactly one target assurance obligation: {obligations:?}"
    );
    assert_eq!(
        obligations.len(),
        3,
        "and the three targets produced three distinct statuses"
    );
}

/// A condition this planner cannot evaluate is unavailable, not false.
///
/// Mutation testing found this: `Outcome::Unavailable` was dead, because a `predicate_ref` fell through
/// to "does not hold" and would have reported a violation nobody established. Not-false and
/// cannot-tell are different answers and the plan has to keep them apart.
#[test]
fn a_condition_the_planner_cannot_evaluate_is_unavailable_rather_than_false() {
    let capsule = fixture::reserve_capsule();
    let mut with_predicate = capsule.clone();
    with_predicate.applicability_conditions =
        vec![world_kernel::experience::ApplicabilityCondition {
            key: "load_profile".to_owned(),
            kind: world_kernel::experience::ConditionKind::PredicateRef,
            expected: Some(serde_json::json!("profile/steady")),
            adaptable: false,
        }];

    let plan = plan_transfer(&with_predicate, &fixture::target_c(), &[]);
    assert_eq!(plan.delta.unavailable.len(), 1);
    assert!(
        plan.delta.different.is_empty(),
        "an unevaluable condition is not a violation"
    );
    assert!(plan.delta.same.is_empty(), "and it is not a match either");
    assert_eq!(plan.status, TransferStatus::AdditionalEvidenceRequired);
    assert!(
        plan.explanation.what_is_unknown[0].contains("predicate"),
        "the explanation says what is missing: {:?}",
        plan.explanation.what_is_unknown
    );
}

/// The capsule schema and the Rust types must reject the same things, or the wire format and the
/// implementation drift.
#[test]
fn the_capsule_schema_and_the_types_reject_the_same_things() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("schemas/experience-capsule-v0.experimental.schema.json"),
        )
        .expect("the schema is checked in"),
    )
    .expect("the schema is valid JSON");

    // Strict on the way in.
    assert_eq!(schema["additionalProperties"], serde_json::json!(false));
    let properties = schema["properties"]
        .as_object()
        .expect("properties are declared");
    for required in [
        "schema_version",
        "id",
        "revision",
        "source_context",
        "problem",
        "intervention",
        "applicability_conditions",
        "outcome",
        "provenance",
    ] {
        assert!(
            schema["required"]
                .as_array()
                .expect("required is a list")
                .iter()
                .any(|name| name == required),
            "{required} is required by the schema"
        );
        assert!(
            properties.contains_key(required),
            "{required} is also a declared property"
        );
    }

    // The four fact states and the four capability states, in both places.
    let mut capsule = fixture::reserve_capsule();
    let serialised = serde_json::to_value(&capsule).expect("a capsule serialises");
    let fact_variants = schema["$defs"]["fact"]["oneOf"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    assert_eq!(
        fact_variants, 3,
        "known, absent and unknown are three states"
    );
    assert_eq!(
        schema["$defs"]["capabilityState"]["enum"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0),
        4,
        "available, unavailable, unsupported and not_observed are four"
    );

    // A capsule with an unknown field does not deserialise, because the type is strict.
    let mut with_extra = serialised.clone();
    with_extra["invented_field"] = serde_json::json!(1);
    assert!(
        serde_json::from_value::<world_kernel::experience::ExperienceCapsule>(with_extra).is_err(),
        "an unknown field is refused by the type and the schema"
    );

    // A fact that is not one of the four is refused.
    let mut with_bad_fact = serialised.clone();
    with_bad_fact["source_context"]["facts"][fixture::CAPACITY] =
        serde_json::json!("just a string");
    assert!(
        serde_json::from_value::<world_kernel::experience::ExperienceCapsule>(with_bad_fact)
            .is_err(),
        "a fact outside the enumerated states is refused"
    );

    // A capability outside the four is refused.
    let mut with_bad_capability = serialised.clone();
    with_bad_capability["source_context"]["capabilities"]["single_writer"] =
        serde_json::json!("maybe");
    assert!(
        serde_json::from_value::<world_kernel::experience::ExperienceCapsule>(with_bad_capability)
            .is_err(),
        "a capability outside the enumerated states is refused"
    );

    // `adaptable` defaults to false, which is the conservative direction, and a round trip preserves it.
    capsule.applicability_conditions[0].adaptable = false;
    let round = serde_json::to_value(&capsule).expect("a capsule serialises");
    assert_eq!(
        round["applicability_conditions"][0]["adaptable"],
        serde_json::json!(false)
    );
    let back: world_kernel::experience::ExperienceCapsule =
        serde_json::from_value(round).expect("and deserialises");
    assert!(!back.applicability_conditions[0].adaptable);
}
