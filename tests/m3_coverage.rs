//! Three pre-registered M3 stories the closed arithmetic fixture could not express.
//!
//! M3-11 a conditional read that switches from A to B, M3-15 an observation that expires because
//! explicit time moved, M3-18 a new contradiction inside a collection. Each test asserts the
//! equivalence property against the oracle as well as its own expectation, because a story that only
//! checks its own guess could be satisfied by two paths being wrong in the same way.

mod support;

use std::{collections::BTreeMap, path::Path};

use serde_json::json;
use support::m3_stories as stories;
use world_kernel::impact::{
    Authority, Currency, Disposition, EvaluationRecord, Limits, NodeId, Snapshot,
    TrustConfiguration, WorkType, revise,
};

// ------------------------------------------------------------------ M3-09

/// The authority that let a record be produced is part of the record's standing, so it is checked
/// when the record is carried forward and not only when the evaluator runs. Nothing has to have
/// moved for a grant to be withdrawn, which is why this pass is over an unchanged world.
#[test]
fn withdrawing_the_grant_that_produced_a_record_stops_it_being_carried_forward() {
    let base = stories::conditional_world(1, "a", 10, 20);
    let target = stories::conditional_world(2, "a", 10, 20);
    let recorded = stories::conditional_records(&base);
    let withdrawn = TrustConfiguration::default();

    let revised = revise(
        &base,
        &target,
        &recorded,
        &stories::conditional_targets(),
        &stories::conditional_evaluators(),
        &withdrawn,
        Limits::default(),
    )
    .expect("a pass that refuses to reuse is still a pass that ran");

    assert_eq!(
        revised.evaluations, 0,
        "nothing moved, so nothing is re-run behind the consumer's back either"
    );
    assert_eq!(
        revised.reused, 0,
        "a record standing on a withdrawn grant is not reused"
    );
    for id in stories::conditional_targets() {
        let finding = &revised.findings[&id];
        assert_eq!(
            finding.authority,
            Authority::Refused,
            "{id} is reported as refused rather than as authorized"
        );
        assert_eq!(
            finding.disposition,
            Disposition::Blocked,
            "{id} can be neither reused nor recomputed, so its work is blocked"
        );
        assert_eq!(
            finding.currency,
            Currency::NeedsRevalidation,
            "{id} is not reusable under an authority it no longer has"
        );
        assert_eq!(
            finding.explanation.code, "recorded_authority_withdrawn",
            "{id} names the reason"
        );
        assert_eq!(
            finding.output,
            recorded[&id].output.clone().into(),
            "{id} still shows what was produced; nothing is erased"
        );
    }
    assert_eq!(
        revised.obligations.len(),
        stories::conditional_targets().len(),
        "one obligation per blocked node, so the consumer is asked rather than deciding alone"
    );
    for obligation in &revised.obligations {
        assert_eq!(obligation.reason_code, "recorded_authority_withdrawn");
        assert_eq!(obligation.work, WorkType::HumanReview);
    }
    assert_eq!(
        revised.records.len(),
        recorded.len(),
        "the record itself survives; what is withdrawn is the right to carry it forward"
    );
}

#[test]
fn a_grant_that_is_still_held_carries_the_record_forward_unchanged() {
    let base = stories::conditional_world(1, "a", 10, 20);
    let target = stories::conditional_world(2, "a", 10, 20);
    let recorded = stories::conditional_records(&base);

    let revised = revise(
        &base,
        &target,
        &recorded,
        &stories::conditional_targets(),
        &stories::conditional_evaluators(),
        &stories::conditional_trust(),
        Limits::default(),
    )
    .expect("the ordinary pass");

    assert_eq!(revised.evaluations, 0);
    assert_eq!(revised.reused, stories::conditional_targets().len());
    for id in stories::conditional_targets() {
        let finding = &revised.findings[&id];
        assert_eq!(finding.authority, Authority::Authorized, "{id}");
        assert_eq!(finding.disposition, Disposition::UnchangedInScope, "{id}");
    }
    assert!(revised.obligations.is_empty());
}

// ------------------------------------------------------------------ M3-11

#[test]
fn a_conditional_read_that_switches_sides_publishes_the_new_read_set() {
    let before = stories::conditional_world(0, "a", 1, 2);
    let after = stories::conditional_world(1, "b", 1, 2);
    let recorded = stories::conditional_records(&before);

    assert_eq!(
        recorded[stories::ROUTED].branches,
        vec!["source_a".to_owned()],
        "the first pass records the side it read"
    );

    let (revised, _) = stories::conditional_pass(&before, &after, &recorded);

    let routed = &revised.findings[stories::ROUTED];
    assert_eq!(
        routed.disposition,
        Disposition::RecomputedChanged,
        "switching the selector moves the consumer"
    );
    let output = routed.output.as_ref().expect("the route produced a value");
    assert_eq!(output["from"], json!("source_b"));
    assert_eq!(output["routed"], json!(2));

    let subjects: Vec<&str> = revised.records[stories::ROUTED]
        .reads
        .iter()
        .map(|read| read.subject.as_str())
        .collect();
    assert!(
        subjects.contains(&stories::SOURCE_B),
        "the new read set names the side now consumed: {subjects:?}"
    );
    assert!(
        !subjects.contains(&stories::SOURCE_A),
        "the side no longer read has left the read set: {subjects:?}"
    );
    assert_eq!(
        revised.records[stories::ROUTED].branches,
        vec!["source_b".to_owned()],
        "the branch is recorded rather than left to be inferred by diffing reads"
    );

    let consumer = &revised.findings[stories::ROUTED_CONSUMER];
    assert_eq!(consumer.disposition, Disposition::RecomputedChanged);
    assert_eq!(
        consumer.output.as_ref().expect("the consumer produced")["seen"],
        json!(2),
        "the switch reached the consumer"
    );
    assert_eq!(revised.evaluations, 2, "two nodes re-ran, not everything");
    assert_eq!(revised.reused, 1, "the bystander did not");
}

#[test]
fn after_switching_the_side_no_longer_read_stops_being_a_dependency() {
    let first = stories::conditional_world(0, "a", 1, 2);
    let switched = stories::conditional_world(1, "b", 1, 2);
    let recorded = stories::conditional_records(&first);
    let step = stories::conditional_pass(&first, &switched, &recorded).0;

    // Only the side that is no longer read moves. A record that still carried it would re-run.
    let moved_offside = stories::conditional_world(2, "b", 999, 2);
    let (revised, _) = stories::conditional_pass(&switched, &moved_offside, &step.records);

    assert_eq!(
        revised.evaluations, 0,
        "nothing re-ran: the abandoned side is not a dependency"
    );
    assert_eq!(revised.reused, 3, "every target was checked and kept");
    for target in [
        stories::ROUTED,
        stories::ROUTED_CONSUMER,
        stories::BYSTANDER,
    ] {
        let finding = &revised.findings[target];
        assert!(
            matches!(
                finding.disposition,
                Disposition::UnchangedInScope | Disposition::ReusedAfterCheck
            ),
            "{target} was kept, not recomputed, and got {:?}",
            finding.disposition
        );
    }
}

#[test]
fn a_switch_back_is_a_new_revision_not_a_rollback() {
    let first = stories::conditional_world(0, "a", 1, 2);
    let switched = stories::conditional_world(1, "b", 1, 2);
    let recorded = stories::conditional_records(&first);
    let step = stories::conditional_pass(&first, &switched, &recorded).0;

    let back = stories::conditional_world(2, "a", 1, 2);
    let (revised, _) = stories::conditional_pass(&switched, &back, &step.records);

    assert_eq!(
        revised.findings[stories::ROUTED].disposition,
        Disposition::RecomputedChanged
    );
    assert_eq!(
        revised.records[stories::ROUTED].branches,
        vec!["source_a".to_owned()]
    );
    assert_eq!(
        revised.records[stories::ROUTED].world_revision,
        2,
        "the record belongs to the revision it was taken at"
    );
}

// ------------------------------------------------------------------ M3-15

#[test]
fn an_observation_expires_when_explicit_time_moves_and_nothing_about_it_changes() {
    let before = stories::timed_world(0, 10);
    let after = stories::timed_world(1, 30);
    let recorded = stories::timed_records(&before);

    assert_eq!(
        before
            .node(stories::OBSERVATION)
            .expect("it exists")
            .payload,
        after.node(stories::OBSERVATION).expect("it exists").payload,
        "the observation itself is byte for byte unchanged"
    );
    assert_ne!(
        before.node(stories::CLOCK).expect("it exists").payload,
        after.node(stories::CLOCK).expect("it exists").payload,
        "only time moved"
    );

    let (revised, _) = stories::timed_pass(&before, &after, &recorded);

    let freshness = &revised.findings[stories::FRESHNESS];
    assert_eq!(freshness.disposition, Disposition::RecomputedChanged);
    assert_eq!(
        freshness.output.as_ref().expect("a value")["fresh"],
        json!(false),
        "the observation is past its expiry"
    );

    // The consumer of freshness follows it; the consumer of the observation alone does not.
    assert_eq!(
        revised.findings[stories::SUMMARY].disposition,
        Disposition::RecomputedChanged
    );
    assert_eq!(
        revised.findings[stories::ARCHIVED].disposition,
        Disposition::UnchangedInScope,
        "a node that never read the clock is untouched by the clock"
    );
    assert_eq!(
        revised.records[stories::ARCHIVED],
        recorded[stories::ARCHIVED],
        "and its record is byte for byte the old one"
    );

    assert_eq!(revised.evaluations, 2, "freshness and its consumer");
    assert_eq!(revised.reused, 1, "the archive");
    assert!(
        revised.obligations.is_empty(),
        "an expiry is not itself work for a human"
    );
}

#[test]
fn a_clock_move_that_leaves_the_verdict_alone_stops_propagating() {
    let before = stories::timed_world(0, 10);
    let after = stories::timed_world(1, 15);
    let recorded = stories::timed_records(&before);

    let (revised, _) = stories::timed_pass(&before, &after, &recorded);

    let freshness = &revised.findings[stories::FRESHNESS];
    assert_eq!(
        freshness.disposition,
        Disposition::RecomputedSame,
        "time moved and the evaluator re-ran, but the verdict did not"
    );
    assert_eq!(
        revised.findings[stories::SUMMARY].disposition,
        Disposition::ReusedAfterCheck,
        "so the consumer of that verdict was checked and kept"
    );
    assert_eq!(revised.evaluations, 1, "only freshness re-ran");
    assert_eq!(revised.reused, 2);
}

// ------------------------------------------------------------------ M3-23

/// Export, import, local change.
///
/// The M2 continuation path is exercised in `continuation_consumer.rs`; what was missing is the
/// incremental pass run over a snapshot that arrived by being read back rather than by being held
/// in memory. The round trip is asserted as lossless first, because a pass that agreed with itself
/// over a degraded snapshot would be the interesting failure and the assertion has to rule it out
/// before anything else can mean anything.
#[test]
fn a_pass_over_an_imported_snapshot_behaves_like_the_pass_that_never_left_memory() {
    let base = stories::conditional_world(1, "a", 10, 20);
    let records = stories::conditional_records(&base);

    let exported = serde_json::to_vec(&base).expect("a snapshot is written down");
    let exported_records = serde_json::to_vec(&records).expect("and so are its records");
    let imported: Snapshot = serde_json::from_slice(&exported).expect("a snapshot is read back");
    let imported_records: BTreeMap<NodeId, EvaluationRecord> =
        serde_json::from_slice(&exported_records).expect("records are read back");

    assert_eq!(
        imported, base,
        "the snapshot survives the trip byte for byte"
    );
    assert_eq!(
        imported_records, records,
        "and so does everything the next pass needs to start from"
    );

    // The local change is a new revision of the same world, made after the import. A snapshot
    // carries the derived values a previous pass produced, so the change is expressed by the same
    // builder rather than by editing one node and leaving its consumers stale.
    let local = stories::conditional_world(2, "a", 11, 20);

    let (revised, observable) = stories::conditional_pass(&imported, &local, &imported_records);
    let (in_memory, expected) = stories::conditional_pass(&base, &local, &records);

    assert_eq!(
        revised, in_memory,
        "the pass over what was imported is the pass over what was never exported"
    );
    assert_eq!(
        observable, expected,
        "including what anyone could observe about it"
    );
    assert!(
        observable
            .outputs
            .iter()
            .any(|(_, value)| value.contains("11")),
        "and the local change is really in it: {observable:?}"
    );
}

// ------------------------------------------------------------------ M3-17

#[test]
fn a_support_the_decision_declared_is_replaced_while_the_wording_stays_identical() {
    let base = stories::claims_world_supporting(1, None, stories::ADJUDICATED);
    let target = stories::claims_world_supporting(2, None, stories::UNRELATED);

    let before = &base
        .node(stories::CLAIM_DECISION)
        .expect("it exists")
        .payload;
    let after = &target
        .node(stories::CLAIM_DECISION)
        .expect("it exists")
        .payload;
    assert_eq!(
        before["justification"], after["justification"],
        "the wording is byte for byte what it was"
    );
    assert_eq!(before["chosen"], after["chosen"], "and so is the choice");
    assert_ne!(
        before["supports"], after["supports"],
        "only the declaration moved, which is the whole of the story"
    );

    let recorded = stories::claims_records(&base);
    let (revised, _) = stories::claims_pass(&base, &target, &recorded);

    let decision = &revised.findings[stories::CLAIM_DECISION];
    assert_eq!(
        decision.disposition,
        Disposition::NeedsHumanReview,
        "a decision that now declares different support cannot be reported as untouched"
    );
    assert_eq!(
        decision.explanation.code, "recorded_decision_support_replaced",
        "the reason names the replacement rather than a change that did not happen"
    );
    assert_eq!(
        decision.authority,
        Authority::Unresolved,
        "the decision no longer stands on what it stood on"
    );
    assert_eq!(
        decision.currency,
        Currency::NeedsRevalidation,
        "and what it stands on has not been checked"
    );
    assert_eq!(
        decision
            .output
            .as_ref()
            .expect("the target's decision is reported"),
        after,
        "the engine reports what is there and substitutes nothing of its own"
    );
    assert!(
        revised.obligations.iter().any(|obligation| {
            obligation.target == stories::CLAIM_DECISION
                && obligation.reason_code == "recorded_decision_support_replaced"
        }),
        "a person is asked what the decision now rests on: {:#?}",
        revised.obligations
    );
    assert_eq!(
        revised.evaluations, 0,
        "replacing a declaration is not a recomputation of anything"
    );
    assert_eq!(
        revised.findings[stories::ADJUDICATED].disposition,
        Disposition::UnchangedInScope,
        "and the node the decision used to name is not dragged into it"
    );
}

#[test]
fn a_support_that_still_names_the_same_node_keeps_the_reason_it_had() {
    let base = stories::claims_world_supporting(1, None, stories::ADJUDICATED);
    let target = stories::claims_world_supporting(2, Some(1), stories::ADJUDICATED);

    let recorded = stories::claims_records(&base);
    let (revised, _) = stories::claims_pass(&base, &target, &recorded);

    let decision = &revised.findings[stories::CLAIM_DECISION];
    assert_eq!(
        decision.disposition,
        Disposition::NeedsHumanReview,
        "the decision's support is still the same node, but that node was reached"
    );
    assert_eq!(
        decision.explanation.code, "recorded_decision_support_moved",
        "the replacement branch must not swallow the movement branch"
    );
    assert_eq!(
        revised.evaluations, 1,
        "only the adjudicated node recomputed"
    );
}

// ------------------------------------------------------------------ M3-18

#[test]
fn a_new_contradiction_in_a_collection_sends_the_recorded_decision_to_review() {
    let before = stories::claims_world_with_unrelated(0, None);
    let after = stories::claims_world_with_unrelated(1, Some(2));
    let recorded = stories::claims_records(&before);

    assert_eq!(
        recorded[stories::ADJUDICATED].output["conflicts"],
        json!(0),
        "one claim is not a contradiction"
    );

    let (revised, _) = stories::claims_pass(&before, &after, &recorded);

    let adjudicated = &revised.findings[stories::ADJUDICATED];
    assert_eq!(adjudicated.disposition, Disposition::RecomputedChanged);
    let output = adjudicated.output.as_ref().expect("a verdict");
    assert_eq!(
        output["conflicts"],
        json!(1),
        "the contradiction is reported, not quietly resolved"
    );
    assert_eq!(output["winner"], json!("bob"));
    assert_eq!(output["value"], json!(2));
    assert_eq!(
        output["refused"],
        json!(false),
        "the declared priority rule resolved it; a tie would refuse instead"
    );

    // The recorded decision is not rewritten. It becomes work.
    let decision = &revised.findings[stories::CLAIM_DECISION];
    assert_eq!(decision.disposition, Disposition::NeedsHumanReview);
    assert_eq!(
        after
            .node(stories::CLAIM_DECISION)
            .expect("it exists")
            .payload,
        before
            .node(stories::CLAIM_DECISION)
            .expect("it exists")
            .payload,
        "the engine did not substitute a new decision"
    );
    assert_eq!(
        revised.obligations.len(),
        1,
        "one obligation, and only one: {:?}",
        revised.obligations
    );
    let obligation = &revised.obligations[0];
    assert_eq!(obligation.target, stories::CLAIM_DECISION);
    assert_eq!(obligation.reason_code, "recorded_decision_support_moved");
    assert_eq!(obligation.work, WorkType::HumanReview);
    assert!(
        !obligation.dedup_key.is_empty(),
        "repeating the plan must not duplicate the task"
    );

    assert_eq!(
        revised.findings[stories::UNRELATED].disposition,
        Disposition::UnchangedInScope,
        "work that never read the collection is not disturbed by it"
    );
    assert_eq!(revised.evaluations, 1, "only the adjudicator re-ran");
}

#[test]
fn a_tie_is_refused_rather_than_broken_by_a_coin_flip() {
    let before = stories::claims_world_with_unrelated(0, None);
    // Bob carries the same declared priority as alice, so the rule has no way to choose.
    let after = stories::claims_world_with_unrelated(1, Some(1));
    let recorded = stories::claims_records(&before);

    let (revised, _) = stories::claims_pass(&before, &after, &recorded);
    let output = revised.findings[stories::ADJUDICATED]
        .output
        .as_ref()
        .expect("a verdict");
    assert_eq!(output["refused"], json!(true));
    assert!(
        output["winner"].is_null(),
        "no winner is invented: {output}"
    );
    assert!(output["value"].is_null());
    assert_eq!(output["conflicts"], json!(1));
}

#[test]
fn the_arbitration_rule_lives_in_the_fixture_and_not_in_the_core() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Domain tokens. The core is allowed to talk about collections, reads and frontiers; it is not
    // allowed to know what a claimant is or which of two claims wins.
    let domain_tokens = [
        "claimant",
        "adjudicat",
        "claim_alice",
        "world:claims",
        "every_claim",
    ];
    for file in ["model.rs", "engine.rs", "mod.rs"] {
        let text = std::fs::read_to_string(root.join("src/impact").join(file))
            .expect("the core source is readable");
        for token in domain_tokens {
            assert!(
                !text.contains(token),
                "src/impact/{file} contains the domain token {token}"
            );
        }
    }
}
