//! Tranche 2 and 3: the incremental engine against an independent full recompute.
//!
//! The central property, for a closed deterministic scope, targets T and the same
//! current inputs S1:
//!
//! ```text
//! observable(incremental(S0, changes, T, profile)) == observable(full_recompute(S1, T, profile))
//! ```
//!
//! It is tested under stated assumptions, not asserted as a theorem. The oracle
//! in `impact_fixture::oracle` is a full recompute that receives no
//! `EvaluationRecord`, no index and no invalidation function, so it cannot call
//! the incremental strategy even by accident.
//!
//! The essential cases also carry manual expectations, so a mistake common to an
//! evaluator cannot make both paths equally wrong.

mod support;

use support::impact_core::{
    Coverage, Currency, Disposition, EvaluationRecord, NodeNature, Profile, Revised, revise,
};
use support::impact_fixture as fixture;

/// The observable result of a pass: values, normalized obligations, coverage and
/// currency. Run identifiers, durations and timestamps are not compared.
#[derive(Debug, PartialEq, Eq)]
struct Observable {
    outputs: Vec<(String, String)>,
    obligations: Vec<String>,
    coverage: Vec<(String, String)>,
    currency: Vec<(String, String)>,
}

fn observable(revised: &Revised) -> Observable {
    let mut outputs: Vec<(String, String)> = revised
        .findings
        .iter()
        .filter_map(|(id, finding)| {
            finding
                .output
                .as_ref()
                .map(|value| (id.clone(), value.to_string()))
        })
        .collect();
    outputs.sort();

    let mut obligations: Vec<String> = revised
        .obligations
        .iter()
        .map(|obligation| format!("{}:{}", obligation.target, obligation.reason_code))
        .collect();
    obligations.sort();

    let coverage: Vec<(String, String)> = revised
        .findings
        .iter()
        .map(|(id, finding)| (id.clone(), format!("{:?}", finding.coverage)))
        .collect();
    let currency: Vec<(String, String)> = revised
        .findings
        .iter()
        .map(|(id, finding)| (id.clone(), format!("{:?}", finding.currency)))
        .collect();

    Observable {
        outputs,
        obligations,
        coverage,
        currency,
    }
}

fn oracle_observable(
    recomputed: &std::collections::BTreeMap<String, support::impact_core::engine::Evaluated>,
) -> Observable {
    let mut outputs: Vec<(String, String)> = recomputed
        .iter()
        .map(|(id, evaluated)| (id.clone(), evaluated.value.to_string()))
        .collect();
    outputs.sort();
    Observable {
        outputs,
        obligations: Vec::new(),
        coverage: Vec::new(),
        currency: Vec::new(),
    }
}

/// Records the fixture's derived nodes at a snapshot, by full recompute, so the
/// incremental pass starts from a realistic base.
fn records_at(
    snapshot: &support::impact_core::Snapshot,
) -> std::collections::BTreeMap<String, EvaluationRecord> {
    let owned: Vec<String> = fixture::DERIVED.iter().map(|id| (*id).to_owned()).collect();
    fixture::oracle(snapshot, &owned)
        .expect("the oracle evaluates the fixture")
        .into_iter()
        .map(|(id, evaluated)| (id, evaluated.record))
        .collect()
}

fn revise_from(
    base: &support::impact_core::Snapshot,
    target: &support::impact_core::Snapshot,
    targets: &[&str],
) -> Revised {
    let owned: Vec<String> = targets.iter().map(|id| (*id).to_owned()).collect();
    revise(
        base,
        target,
        &records_at(base),
        &owned,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the incremental pass runs")
}

#[test]
fn mutation_1_limit_100_to_90_recomputes_both_and_keeps_the_same_verdicts() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    // Manual expectation before any comparison: both comparisons were
    // re-evaluated, and they stay respectively true and false.
    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].output,
        Some(serde_json::json!({"eligible": true})),
        "cost_a 80 is still under a limit of 90"
    );
    assert_eq!(
        revised.findings[fixture::ELIGIBLE_B].output,
        Some(serde_json::json!({"eligible": false})),
        "cost_b 110 is still over a limit of 90"
    );
    assert_eq!(
        revised.findings[fixture::SUPPORT].disposition,
        Disposition::ReusedAfterCheck,
        "the support consumer reads only the two booleans, and neither moved, so it is not run"
    );
    assert_eq!(
        revised.evaluations, 2,
        "only the two comparisons ran; the consumer of their unchanged results did not"
    );
}

#[test]
fn mutation_1_matches_the_full_recompute() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");
    let targets: Vec<String> = fixture::DERIVED.iter().map(|id| (*id).to_owned()).collect();

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);
    let owned: Vec<String> = fixture::DERIVED.iter().map(|id| (*id).to_owned()).collect();
    let oracle = fixture::oracle(&s1, &owned).expect("the oracle runs");

    let from_incremental = observable(&revised);
    let from_full = oracle_observable(&oracle);

    assert_eq!(
        from_incremental.outputs, from_full.outputs,
        "the produced values must be the same by either route"
    );
    for target in &targets {
        assert!(from_full.outputs.iter().any(|(id, _)| id == target));
    }
}

#[test]
fn mutation_2_limit_90_to_70_flips_a_and_leaves_the_decision_alone() {
    let s0 = fixture::snapshot(0, 90, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 70, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &[fixture::ELIGIBLE_A, fixture::SUPPORT]);

    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].output,
        Some(serde_json::json!({"eligible": false})),
        "cost_a 80 is now over a limit of 70"
    );
    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].disposition,
        Disposition::RecomputedChanged
    );
    assert_eq!(
        revised.findings[fixture::SUPPORT].output.as_ref().unwrap()["supports"],
        serde_json::json!(["a:false", "b:false"]),
        "both options are now unsupported, and the engine did not pick one"
    );
    assert!(
        !revised
            .obligations
            .iter()
            .any(|obligation| obligation.target == fixture::DECISION),
        "the engine must not authorise a new decision for a human"
    );
}

#[test]
fn mutation_3_a_presentation_label_does_not_disturb_the_arithmetic() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 80, 110, "published");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    assert_eq!(
        revised.evaluations, 0,
        "no arithmetic evaluator reads the label, so none may run"
    );
    assert_eq!(revised.reused, 3);
    for id in fixture::DERIVED {
        assert_eq!(
            revised.findings[id].disposition,
            Disposition::UnchangedInScope,
            "{id} consumes nothing that moved"
        );
    }
    assert_eq!(
        revised.findings[fixture::SUPPORT].currency,
        Currency::Reusable,
        "identical content is reusable, and that is not the same as satisfied"
    );
    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].coverage,
        Coverage::ClosedProfile
    );
}

#[test]
fn an_unchanged_snapshot_recomputes_nothing() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    assert_eq!(revised.evaluations, 0);
    assert_eq!(revised.reused, 3);
}

#[test]
fn a_changed_input_with_an_unchanged_output_stops_propagation() {
    // cost_a moves from 80 to 85. The limit is 100, so `eligible_a` stays true.
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 85, 110, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].disposition,
        Disposition::RecomputedSame,
        "the consumed value moved but the produced value did not"
    );
    assert_eq!(
        revised.findings[fixture::SUPPORT].disposition,
        Disposition::ReusedAfterCheck,
        "the consumer of an unchanged boolean must not be run at all"
    );
    assert_eq!(
        revised.evaluations, 1,
        "only the one comparison that read the number ran"
    );
    assert_eq!(revised.reused, 2);
}

#[test]
fn a_consumer_with_two_affected_parents_is_not_reused_when_only_one_is_equivalent() {
    // A diamond. cost_a moves but stays under the limit, so its boolean holds.
    // cost_b crosses the limit, so its boolean does not.
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 120, 85, 120, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].disposition,
        Disposition::RecomputedSame
    );
    assert_eq!(
        revised.findings[fixture::ELIGIBLE_B].disposition,
        Disposition::RecomputedChanged,
        "equivalence on the first parent must not erase the second parent's impact"
    );
    assert_eq!(
        revised.findings[fixture::SUPPORT].disposition,
        Disposition::RecomputedChanged,
        "the consumer is examined once, over both of its dependencies, and the second one moved"
    );
    assert_eq!(
        revised.findings[fixture::SUPPORT].output.as_ref().unwrap()["supports"],
        serde_json::json!(["a:true", "b:true"]),
        "and it sees the aggregate, not just the parent that held"
    );
    assert_eq!(revised.evaluations, 3);
}

#[test]
fn returning_to_an_earlier_value_is_a_new_revision_not_a_rewind() {
    let s0 = fixture::snapshot(0, 70, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].disposition,
        Disposition::RecomputedChanged,
        "a value that comes back is recomputed, and the history is not rewound"
    );
    assert_eq!(revised.base_revision, 0);
    assert_eq!(revised.target_revision, 1);
}

#[test]
fn an_observed_absence_is_not_left_silently_clean() {
    // A snapshot where the limit has not been observed yet.
    let mut s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    s0.nodes.remove(fixture::LIMIT);
    s0.natures.remove(fixture::LIMIT);
    let s1 = fixture::snapshot(1, 100, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    let explanation = revised
        .why(fixture::ELIGIBLE_A)
        .expect("the missing operand is explained");
    assert_eq!(
        explanation.code, "recomputed_changed",
        "an absence that became a value is a change, not a no-op"
    );
    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].output,
        Some(serde_json::json!({"eligible": true})),
        "the comparison now has both operands"
    );
}

#[test]
fn a_tombstoned_read_is_not_silently_skipped() {
    let mut s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    s0.nodes.insert(
        fixture::LIMIT.to_owned(),
        support::impact_core::NodeVersion::tombstone(2, "producer"),
    );
    s0.nodes.insert(
        fixture::COST_B.to_owned(),
        support::impact_core::NodeVersion::new(
            1,
            "money/1",
            serde_json::json!({"value": 110}),
            "producer",
        ),
    );
    let s1 = fixture::snapshot(1, 100, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &[fixture::ELIGIBLE_A]);

    assert!(
        revised.why(fixture::ELIGIBLE_A).is_some(),
        "a removed dependency has to produce a disposition, not a silent reuse"
    );
}

#[test]
fn a_recorded_decision_is_never_replaced_by_the_engine() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 70, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &[fixture::SUPPORT, fixture::DECISION]);

    let decision = revised
        .findings
        .get(fixture::DECISION)
        .expect("the decision is reported");
    assert_eq!(
        decision.disposition,
        Disposition::NeedsHumanReview,
        "a decision whose support moved becomes work for a human"
    );
    let recorded = decision
        .output
        .as_ref()
        .expect("the recorded decision is reported as it stands");
    assert_eq!(recorded["chosen"], serde_json::json!("a"), "unchanged");
    assert_eq!(
        recorded["justification"],
        serde_json::json!("option a was supported when the decision was taken"),
        "the wording of a recorded decision is never rewritten by the engine"
    );
    assert_eq!(
        recorded["recorded_at_revision"],
        serde_json::json!(0),
        "nor is its date moved"
    );
    assert!(
        revised
            .obligations
            .iter()
            .any(|obligation| obligation.target == fixture::DECISION
                && obligation.work == support::impact_core::WorkType::HumanReview),
        "and it produces the obligation rather than acting"
    );
    assert_eq!(
        s1.node(fixture::DECISION)
            .map(|node| node.payload["chosen"].clone()),
        Some(serde_json::json!("a")),
        "the recorded decision is untouched in the snapshot"
    );
}

#[test]
fn an_evaluator_a_consumer_did_not_grant_cannot_claim_the_closed_profile() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");

    // A trust configuration that grants nothing.
    let outcome = revise(
        &s0,
        &s1,
        &records_at(&s0),
        &[fixture::ELIGIBLE_A.to_owned()],
        &fixture::evaluators(),
        &support::impact_core::TrustConfiguration::default(),
        fixture::limits(),
    );

    let error = match outcome {
        Ok(_) => panic!("an ungranted evaluator must not be treated as trusted"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("grants at most opaque"),
        "the refusal must name the shortfall, got {error}"
    );
}

#[test]
fn a_moved_evaluator_version_invalidates_the_record_instead_of_reusing_the_cache() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 80, 110, "draft");
    let mut records = records_at(&s0);

    // The recorded evaluation was produced by a different evaluator build.
    records
        .get_mut(fixture::ELIGIBLE_A)
        .unwrap()
        .evaluator_version = "0".into();

    let revised = revise(
        &s0,
        &s1,
        &records,
        &fixture::DERIVED
            .iter()
            .map(|id| (*id).to_owned())
            .collect::<Vec<_>>(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the pass runs");

    assert_eq!(
        revised.findings[fixture::ELIGIBLE_A].disposition,
        Disposition::RecomputedSame,
        "the value happens to be the same, but it was recomputed rather than reused"
    );
    assert!(
        revised
            .why(fixture::ELIGIBLE_A)
            .expect("explained")
            .trigger
            .as_deref()
            .is_some_and(|trigger| trigger.contains("eligible_a@0")),
        "and the trigger names the moved definition"
    );
    assert_eq!(
        revised.findings[fixture::SUPPORT].disposition,
        Disposition::ReusedAfterCheck,
        "the consumer's own inputs did not move, so it is still reusable"
    );
}

#[test]
fn why_and_why_reused_answer_the_two_questions() {
    // Mutation 1 moves the limit without changing either boolean, so the support
    // consumer is genuinely reused while the comparisons are recomputed.
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");

    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    let concerned = revised.why(fixture::ELIGIBLE_A).expect("why");
    assert!(!concerned.code.is_empty());
    assert!(!concerned.condition_to_continue.is_empty());
    assert!(concerned.path.contains(&fixture::ELIGIBLE_A.to_owned()));

    // A node nothing reaches, and a node a change reaches that held, answer the
    // same question with different reasons.
    let kept = revised.why_reused(fixture::SUPPORT).expect("why_reused");
    assert_eq!(
        kept.code, "consumed_facets_unchanged",
        "the change reached it, and its consumed booleans did not move"
    );
    assert!(
        kept.path.contains(&fixture::ELIGIBLE_A.to_owned()),
        "and the explanation names what reached it"
    );

    // A recomputation that produced the same value also kept the node, and says so.
    let recomputed_same = revised.why_reused(fixture::ELIGIBLE_A).expect("kept");
    assert_eq!(recomputed_same.code, "recomputed_same");

    // A node whose value moved is not reported as kept.
    let moved = fixture::snapshot(1, 70, 80, 110, "draft");
    let changed = revise_from(
        &fixture::snapshot(0, 100, 80, 110, "draft"),
        &moved,
        &fixture::DERIVED,
    );
    assert!(
        changed.why_reused(fixture::SUPPORT).is_none(),
        "a consumer whose aggregate moved was not kept for its old value"
    );
    assert!(changed.why(fixture::SUPPORT).is_some());
}

#[test]
fn a_stale_plan_may_not_be_published_as_current() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");
    let revised = revise_from(&s0, &s1, &fixture::DERIVED);

    assert!(support::impact_core::assert_publishable(&revised, 1).is_ok());

    // Something moved the world on before publication.
    let error = match support::impact_core::assert_publishable(&revised, 2) {
        Ok(()) => panic!("a plan computed against another revision must not publish"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("stale") || error.to_string().contains("revision"));
}

#[test]
fn a_profile_a_consumer_does_not_cover_is_not_reported_as_closed() {
    // Nothing moved, so the record is reused rather than recomputed, and the
    // coverage it was recorded under is what the consumer learns.
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 80, 110, "draft");
    let mut records = records_at(&s0);
    records.get_mut(fixture::ELIGIBLE_A).unwrap().profile = Profile::Opaque;

    let revised = revise(
        &s0,
        &s1,
        &records,
        &[fixture::ELIGIBLE_A.to_owned()],
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the pass runs");

    assert_ne!(
        revised.findings[fixture::ELIGIBLE_A].coverage,
        Coverage::ClosedProfile,
        "an opaque record must not be promoted to a closed profile by reuse"
    );
}

#[test]
fn an_observed_input_is_not_treated_as_recomputable() {
    let snapshot = fixture::snapshot(0, 100, 80, 110, "draft");

    assert!(support::impact_core::is_recomputable(
        snapshot.nature(fixture::ELIGIBLE_A)
    ));
    assert!(!support::impact_core::is_recomputable(
        snapshot.nature(fixture::LIMIT)
    ));
    assert!(!support::impact_core::is_recomputable(
        snapshot.nature(fixture::COST_A)
    ));
    assert!(!support::impact_core::is_recomputable(
        snapshot.nature(fixture::DECISION)
    ));
    assert_eq!(
        snapshot.nature(fixture::DECISION),
        Some(NodeNature::Decision),
        "a human decision is recorded, not derived"
    );
}
