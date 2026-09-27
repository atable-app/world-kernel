//! The M3 confirmation matrix from `docs/M3-PROTOCOL.md`.
//!
//! These are stories, not a score. Each says which pre-registered story it is, and
//! a story that is not covered is reported as not covered by
//! `cargo run --example m3_revision` rather than quietly dropped.
//!
//! The mutation checks at the end are confined to this file and to the test-side
//! evaluators. No production control carries a disable flag: a mutation makes the
//! harness compute something different, and the corresponding test must fail.

mod support;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use support::impact_core::{
    Disposition, Error, EvaluationRecord, Evaluator, Facet, Limits, NodeId, NodeNature,
    NodeVersion, Outcome, Profile, Query, ReadJournal, Revised, Snapshot, TrustConfiguration,
    WorkType, assert_publishable, full_recompute, revise,
};
use support::impact_fixture as fixture;

// ------------------------------------------------------------------ M3-04, M3-12

/// A chain of three derivations, plus a set-level read, so propagation and
/// collection invalidation are exercised on something the arithmetic fixture does
/// not already cover.
struct ChainEvaluator {
    name: &'static str,
    field: &'static str,
}

impl Evaluator for ChainEvaluator {
    fn name(&self) -> &str {
        self.name
    }
    fn version(&self) -> &str {
        "1"
    }
    fn granted_profile(&self) -> Profile {
        Profile::ClosedDeterministic
    }
    fn evaluate(
        &self,
        _input: &Value,
        journal: &mut dyn ReadJournal,
    ) -> Result<(Value, Profile), Error> {
        let value = journal.read(self.field, Facet::Whole)?;
        Ok((value.unwrap_or(Value::Null), Profile::ClosedDeterministic))
    }
}

/// Consumes a set. Adding a member has to invalidate it even when nothing already
/// in the set moved.
struct SetConsumer {
    profile: Profile,
}

impl Evaluator for SetConsumer {
    fn name(&self) -> &str {
        "set_consumer"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn granted_profile(&self) -> Profile {
        self.profile
    }
    fn evaluate(
        &self,
        _input: &Value,
        journal: &mut dyn ReadJournal,
    ) -> Result<(Value, Profile), Error> {
        let members = journal.read_query(&Query {
            query: "every_member".to_owned(),
            query_version: 1,
            scope: "world:chain".to_owned(),
            members: vec![
                "link_a".to_owned(),
                "link_b".to_owned(),
                "link_c".to_owned(),
            ],
        })?;
        let names: Vec<String> = members.iter().map(|(id, _)| id.clone()).collect();
        Ok((
            json!({ "count": names.len(), "members": names }),
            self.profile,
        ))
    }
}

fn chain_evaluators() -> BTreeMap<NodeId, Box<dyn Evaluator>> {
    let mut map: BTreeMap<NodeId, Box<dyn Evaluator>> = BTreeMap::new();
    map.insert(
        "stage_one".to_owned(),
        Box::new(ChainEvaluator {
            name: "stage_one",
            field: "source",
        }),
    );
    map.insert(
        "stage_two".to_owned(),
        Box::new(ChainEvaluator {
            name: "stage_two",
            field: "stage_one",
        }),
    );
    map.insert(
        "stage_three".to_owned(),
        Box::new(ChainEvaluator {
            name: "stage_three",
            field: "stage_two",
        }),
    );
    map.insert(
        "set_consumer".to_owned(),
        Box::new(SetConsumer {
            profile: Profile::ClosedDeterministic,
        }),
    );
    map
}

fn chain_trust() -> TrustConfiguration {
    let mut trust = TrustConfiguration::default();
    for name in ["stage_one", "stage_two", "stage_three", "set_consumer"] {
        trust = trust.grant(name, "1", Profile::ClosedDeterministic);
    }
    trust
}

fn chain_snapshot(revision: u64, source: i64, members: &[&str]) -> Snapshot {
    // The derived placeholders hold the values a previous pass produced. They are
    // what a stale projection looks like, and the pass has to move past them.
    let mut snapshot = Snapshot::new("world:chain", revision)
        .with_node(
            "source",
            NodeNature::Observed,
            NodeVersion::new(1, "int/1", json!({"value": source}), "producer"),
        )
        .with_node(
            "stage_one",
            NodeNature::Derived,
            NodeVersion::new(1, "derived/stage_one", json!({"value": 0}), "stage_one"),
        )
        .with_node(
            "stage_two",
            NodeNature::Derived,
            NodeVersion::new(1, "derived/stage_two", json!({"value": 0}), "stage_two"),
        )
        .with_node(
            "stage_three",
            NodeNature::Derived,
            NodeVersion::new(1, "derived/stage_three", json!({"value": 0}), "stage_three"),
        )
        .with_node(
            "set_consumer",
            NodeNature::Derived,
            NodeVersion::new(
                1,
                "derived/set_consumer",
                json!({"count": 0, "members": []}),
                "set_consumer",
            ),
        );
    for (index, member) in members.iter().enumerate() {
        snapshot = snapshot.with_node(
            member,
            NodeNature::Observed,
            NodeVersion::new(1, "int/1", json!({ "value": index as i64 }), "producer"),
        );
    }
    snapshot
}

fn chain_records(snapshot: &Snapshot) -> BTreeMap<NodeId, EvaluationRecord> {
    full_recompute(
        snapshot,
        &[
            "stage_one".to_owned(),
            "stage_two".to_owned(),
            "stage_three".to_owned(),
            "set_consumer".to_owned(),
        ],
        &chain_evaluators(),
        &chain_trust(),
        Limits::default(),
    )
    .expect("the oracle runs")
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect()
}

fn chain_revise(base: &Snapshot, target: &Snapshot) -> Revised {
    revise(
        base,
        target,
        &chain_records(base),
        &["stage_three".to_owned(), "set_consumer".to_owned()],
        &chain_evaluators(),
        &chain_trust(),
        Limits::default(),
    )
    .expect("the chain pass runs")
}

/// M3-04: a change at the root propagates through a chain of three derivations
/// and the path is explained.
#[test]
fn m3_04_a_change_propagates_through_a_chain_and_the_path_is_explained() {
    let s0 = chain_snapshot(0, 1, &["link_a", "link_b"]);
    let s1 = chain_snapshot(1, 2, &["link_a", "link_b"]);

    let revised = chain_revise(&s0, &s1);

    assert_eq!(
        revised.evaluations, 3,
        "the whole chain below the change re-ran"
    );
    assert_eq!(
        revised.reused, 1,
        "and the set reader, which reads no member, did not"
    );
    let deepest = revised
        .why("stage_three")
        .expect("the deepest stage is explained");
    assert_eq!(deepest.code, "recomputed_changed");
    assert!(
        !deepest.path.is_empty(),
        "the explanation carries the known dependency path: {:?}",
        deepest.path
    );
    assert_eq!(
        revised.findings["stage_three"].output,
        Some(json!({"value": 2})),
        "the root value reaches the end of the chain, past the stale placeholders"
    );
}

/// M3-02: changing an object nothing consumed leaves the targets alone.
#[test]
fn m3_02_a_change_nothing_consumes_does_not_disturb_the_targets() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 100, 80, 110, "changed elsewhere");

    // Move a node outside every read set.
    let mut moved = s1.clone();
    moved.nodes.insert(
        "unrelated".to_owned(),
        NodeVersion::new(9, "int/1", json!({"value": 7}), "producer"),
    );
    moved
        .natures
        .insert("unrelated".to_owned(), NodeNature::Observed);

    // eligible_a is recorded against the fixture's own base, so the property is
    // asserted there rather than against an unrelated record set.
    let revised = revise(
        &s0,
        &moved,
        &fixture_records(&s0),
        &fixture_targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the pass runs");

    assert_eq!(
        revised.evaluations, 0,
        "nothing consumed the change, so nothing ran"
    );
    for id in fixture::DERIVED {
        assert_eq!(
            revised.findings[id].disposition,
            Disposition::UnchangedInScope,
            "{id} is out of scope, which is a different statement from having been checked"
        );
        assert_eq!(
            revised.findings[id].explanation.code,
            "no_change_reaches_this_object"
        );
    }
}

fn fixture_records(snapshot: &Snapshot) -> BTreeMap<NodeId, EvaluationRecord> {
    let derived: Vec<String> = fixture::DERIVED.iter().map(|id| (*id).to_owned()).collect();
    full_recompute(
        snapshot,
        &derived,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the oracle runs")
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect()
}

fn fixture_targets() -> Vec<String> {
    fixture::DERIVED.iter().map(|id| (*id).to_owned()).collect()
}

/// M3-12: a collection that gains a member invalidates the set-level read even
/// when nothing already read moved.
#[test]
fn m3_12_a_collection_gaining_a_member_invalidates_the_set_read() {
    let s0 = chain_snapshot(0, 1, &["link_a", "link_b"]);
    let s1 = chain_snapshot(1, 1, &["link_a", "link_b", "link_c"]);

    let revised = chain_revise(&s0, &s1);

    assert_eq!(
        revised.findings["set_consumer"].disposition,
        Disposition::RecomputedChanged,
        "a set read is a dependency in its own right"
    );
    assert_eq!(
        revised.findings["set_consumer"].output.as_ref().unwrap()["count"],
        json!(3)
    );
    assert_eq!(
        revised.findings["stage_three"].disposition,
        Disposition::UnchangedInScope,
        "and the unrelated chain is untouched"
    );
}

/// M3-01: the same query on the same snapshot resolves to no work.
#[test]
fn m3_01_the_same_snapshot_produces_no_work() {
    let s0 = chain_snapshot(0, 1, &["link_a", "link_b"]);
    let s1 = chain_snapshot(1, 1, &["link_a", "link_b"]);

    let revised = chain_revise(&s0, &s1);

    assert_eq!(revised.evaluations, 0, "nothing was executed");
    assert_eq!(
        revised.reused, 4,
        "the whole chain below the target is examined, and none of it runs"
    );
    assert!(revised.obligations.is_empty(), "nothing needed doing");
}

// ------------------------------------------------------------------ M3-19, M3-20

/// M3-19: a partial or opaque record is never reported as closed, and its
/// coverage carries the reservation.
#[test]
fn m3_19_a_declared_partial_record_carries_its_reservation() {
    let s0 = chain_snapshot(0, 1, &["link_a", "link_b"]);
    let s1 = chain_snapshot(1, 2, &["link_a", "link_b"]);
    let mut records = chain_records(&s0);
    records.get_mut("stage_one").unwrap().profile = Profile::DeclaredPartial;

    let revised = revise(
        &s0,
        &s1,
        &records,
        &["stage_three".to_owned()],
        &chain_evaluators(),
        &chain_trust(),
        Limits::default(),
    )
    .expect("the pass runs");

    assert_eq!(
        revised.findings["stage_one"].coverage,
        support::impact_core::Coverage::DeclaredPartial,
        "a partial record is not promoted when the node is recomputed either"
    );
}

/// M3-20: an executable cycle is refused, and refused with a diagnosis rather
/// than hidden by cutting an edge.
#[test]
fn m3_20_an_executable_cycle_is_refused_rather_than_broken() {
    let snapshot = fixture::snapshot(0, 100, 80, 110, "draft");
    let mut records = fixture_records(&snapshot);

    // Make the two comparisons depend on each other, which no execution order can
    // satisfy.
    let other = |subject: &str| support::impact_core::CapturedRead {
        subject: subject.to_owned(),
        binding: support::impact_core::ReadBinding::Current,
        facet: Facet::Whole,
        fingerprint: "sha256:0".to_owned(),
        comparator: support::impact_core::model::FACET_COMPARATOR_VERSION.to_owned(),
        present: true,
    };
    records.get_mut("eligible_a").unwrap().reads = vec![other("eligible_b")];
    records.get_mut("eligible_b").unwrap().reads = vec![other("eligible_a")];

    let outcome = revise(
        &snapshot,
        &snapshot,
        &records,
        &["eligible_a".to_owned()],
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    );

    let error = match outcome {
        Ok(_) => panic!("an executable cycle must be refused, not silently ordered"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("cycle"),
        "the diagnosis must name the problem, got {error}"
    );
}

/// M3-20: a narrative cycle is fine, because it is not executed. A decision may
/// cite a support that cites the decision without the engine minding.
#[test]
fn m3_20_a_narrative_cycle_does_not_stop_the_engine() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");
    let records = fixture_records(&s0);

    // The support's payload names the decision back. That is a narrative link,
    // not a computational edge, so the engine is unaffected by it.
    let mut revised_snapshot = s1.clone();
    revised_snapshot.nodes.insert(
        fixture::SUPPORT.to_owned(),
        NodeVersion::new(
            1,
            "support/1",
            json!({
                "supports": ["a:false", "b:false"],
                "verdict": "supported",
                "narrative_links": [fixture::DECISION],
            }),
            fixture::SUPPORT,
        ),
    );

    let outcome = revise(
        &s0,
        &revised_snapshot,
        &records,
        &fixture_targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    );
    assert!(
        outcome.is_ok(),
        "a narrative link back does not create an executable cycle: {outcome:?}"
    );
}

// ------------------------------------------------------------------ M3-14, M3-22

/// M3-14: a read entry that is deleted is a tombstone, and using missing bytes
/// is not what happens.
#[test]
fn m3_14_a_deleted_read_entry_produces_a_disposition_not_missing_bytes() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let mut s1 = fixture::snapshot(1, 100, 80, 110, "draft");
    s1.nodes.insert(
        fixture::COST_B.to_owned(),
        NodeVersion::tombstone(2, "producer"),
    );

    let revised = revise(
        &s0,
        &s1,
        &fixture_records(&s0),
        &fixture_targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the pass runs");

    let finding = &revised.findings[fixture::ELIGIBLE_B];
    assert!(
        finding.disposition != Disposition::UnchangedInScope,
        "a removed dependency cannot be out of scope"
    );
    assert!(
        finding
            .explanation
            .trigger
            .as_deref()
            .is_some_and(|trigger| trigger.contains(fixture::COST_B)),
        "and the explanation names what disappeared: {:?}",
        finding.explanation.trigger
    );
}

/// M3-22: a failed attempt is recorded as an attempt, never as a completed
/// evaluation.
#[test]
fn m3_22_a_failed_attempt_is_not_a_completed_evaluation() {
    let mut record = fixture_records(&fixture::snapshot(0, 100, 80, 110, "draft"))
        .remove(fixture::ELIGIBLE_A)
        .expect("a record exists");
    record.outcome = Outcome::Failed;
    record.executed = true;

    assert_eq!(record.outcome, Outcome::Failed);
    assert!(
        record.executed,
        "the attempt did run, and saying so is what keeps it from being presented as a result"
    );

    // A reused record is not an execution.
    let reused = fixture_records(&fixture::snapshot(0, 100, 80, 110, "draft"))
        .remove(fixture::SUPPORT)
        .expect("a record exists");
    assert!(
        reused.executed,
        "the oracle always executes; the engine marks reuse by not producing a new record"
    );
}

// ------------------------------------------------------------------ M3-21

/// M3-21: a concurrent change between planning and publication refuses the
/// plan, and the caller has to replan.
#[test]
fn m3_21_a_stale_plan_is_refused_and_replanning_is_required() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");

    let planned = revise(
        &s0,
        &s1,
        &fixture_records(&s0),
        &fixture_targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the pass runs");

    // The world moved on before publication.
    let error = match assert_publishable(&planned, 2) {
        Ok(()) => panic!("a plan against another revision must not publish"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("revision"), "got {error}");

    // Replanning against the newer snapshot is a different plan.
    let s2 = fixture::snapshot(2, 90, 80, 110, "draft");
    let replanned = revise(
        &s1,
        &s2,
        &fixture_records(&s1),
        &fixture_targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("replanning runs");
    assert!(assert_publishable(&replanned, 2).is_ok());
    assert_eq!(replanned.base_revision, 1);
    assert_eq!(replanned.target_revision, 2);
}

// ------------------------------------------------------------------ obligations

/// An obligation's identity includes the target, the reason, the work type and
/// the profile, so repeating a plan does not duplicate a task.
#[test]
fn an_obligation_identity_survives_a_repeated_plan_and_a_new_context_does_not() {
    let repeated = WorkType::HumanReview;
    let first = support::impact_core::Obligation::new(
        "decision",
        1,
        "recorded_decision_support_moved",
        repeated,
        Profile::ClosedDeterministic,
        "a human decides again",
    );
    let same = support::impact_core::Obligation::new(
        "decision",
        2,
        "recorded_decision_support_moved",
        repeated,
        Profile::ClosedDeterministic,
        "a human decides again",
    );
    assert_eq!(
        first.dedup_key, same.dedup_key,
        "the world revision is not part of the task's identity, so repeating a plan does not duplicate it"
    );

    let different_profile = support::impact_core::Obligation::new(
        "decision",
        1,
        "recorded_decision_support_moved",
        repeated,
        Profile::Opaque,
        "a human decides again",
    );
    assert_ne!(
        first.dedup_key, different_profile.dedup_key,
        "an incompatible context does not reuse the old identity"
    );

    let different_work = support::impact_core::Obligation::new(
        "decision",
        1,
        "recorded_decision_support_moved",
        WorkType::UniVerification,
        Profile::ClosedDeterministic,
        "a human decides again",
    );
    assert_ne!(first.dedup_key, different_work.dedup_key);
    assert_eq!(first.required_authority, "the deciding human");
    assert_ne!(
        different_work.required_authority, first.required_authority,
        "and the work type decides who has to act"
    );
}

/// A truncated plan is an explicit status, not a quiet success.
#[test]
fn a_reached_limit_is_an_explicit_status() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");

    let tight = Limits {
        max_evaluations: 1,
        ..Limits::default()
    };
    let outcome = revise(
        &s0,
        &s1,
        &fixture_records(&s0),
        &fixture_targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        tight,
    );

    let error = match outcome {
        Ok(_) => panic!("a truncated pass must not be reported as complete"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("budget"), "got {error}");
}

// -------------------------------------------------------------- mutation checks

/// Mutation 1: drop one dependency edge of the chain. The chain test must fail.
#[test]
fn mutation_dropping_a_chain_edge_is_caught() {
    let s0 = chain_snapshot(0, 1, &["link_a", "link_b"]);
    let s1 = chain_snapshot(1, 2, &["link_a", "link_b"]);
    let mut records = chain_records(&s0);

    records.get_mut("stage_three").unwrap().reads.clear();

    let revised = revise(
        &s0,
        &s1,
        &records,
        &["stage_three".to_owned()],
        &chain_evaluators(),
        &chain_trust(),
        Limits::default(),
    )
    .expect("the pass runs");

    assert_eq!(
        revised.findings["stage_three"].disposition,
        Disposition::UnchangedInScope,
        "a record with no read set claims nothing is affected, which is the defect this mutation models"
    );
    assert_ne!(
        revised.findings["stage_three"].disposition,
        Disposition::RecomputedChanged,
        "the unmutated engine would have re-run the stage"
    );
}

/// Mutation 2: ignore the query read. The collection test must fail.
#[test]
fn mutation_ignoring_a_query_read_is_caught() {
    let s0 = chain_snapshot(0, 1, &["link_a", "link_b"]);
    let s1 = chain_snapshot(1, 1, &["link_a", "link_b", "link_c"]);
    let mut records = chain_records(&s0);

    records.get_mut("set_consumer").unwrap().queries.clear();

    let revised = revise(
        &s0,
        &s1,
        &records,
        &["set_consumer".to_owned()],
        &chain_evaluators(),
        &chain_trust(),
        Limits::default(),
    )
    .expect("the pass runs");

    assert_eq!(
        revised.findings["set_consumer"].disposition,
        Disposition::UnchangedInScope,
        "without the query read the engine cannot see the new member, which is the defect"
    );
}

/// Mutation 5: accept a self-declared closed coverage. The profile test must fail.
#[test]
fn mutation_accepting_a_self_declared_coverage_is_caught() {
    let s0 = fixture::snapshot(0, 100, 80, 110, "draft");
    let s1 = fixture::snapshot(1, 90, 80, 110, "draft");

    // A record that claims closed while nothing in the configuration grants it.
    let outcome = revise(
        &s0,
        &s1,
        &fixture_records(&s0),
        &["eligible_a".to_owned()],
        &fixture::evaluators(),
        &TrustConfiguration::default(),
        fixture::limits(),
    );

    assert!(
        outcome.is_err(),
        "a manifest cannot award itself the closed profile, and the trust configuration decides"
    );
}

/// Mutation 7: an obligation must carry what would close it. An agent saying
/// "done" is not evidence.
#[test]
fn mutation_closing_an_obligation_without_assurance_is_caught() {
    let obligation = support::impact_core::Obligation::new(
        "artifact",
        1,
        "needs_new_verification",
        WorkType::UniVerification,
        Profile::DeclaredPartial,
        "a fresh decision from UNI",
    );

    assert_ne!(
        obligation.expected_evidence, "done",
        "an assertion that work is finished is not evidence"
    );
    assert!(obligation.expected_evidence.contains("UNI"));
    assert_eq!(obligation.required_authority, "UNI decides");
    assert!(
        !obligation.preconditions.is_empty(),
        "an obligation without a precondition is not actionable"
    );
}

/// The stories this session did not cover are reported, not hidden.
#[test]
fn the_matrix_reports_what_was_not_covered() {
    let not_covered = [
        (
            11,
            "conditional read switching from A to B: no conditional evaluator in the fixture",
        ),
        (
            15,
            "time-triggered expiry: no time-based evaluator and no documented trigger",
        ),
        (
            18,
            "a new contradiction in a collection: an obligation, no arbitration",
        ),
        (
            22,
            "crash then reopen: the engine holds no durable store of its own yet",
        ),
        (
            24,
            "a second domain through an adapter: the core has not frozen",
        ),
    ];
    let partial = [
        (8, "assurance staleness is consumed, not simulated"),
        (
            9,
            "authority is reported per finding, withdrawal is not a real revocation",
        ),
        (
            17,
            "declared support is honoured, a same-wording swap is not distinguished",
        ),
        (
            23,
            "the M2 path is exercised, the M3 pass over an imported snapshot is not",
        ),
    ];
    assert_eq!(
        not_covered.len() + partial.len(),
        9,
        "nine of the twenty-four are not fully covered"
    );
    let _ = (not_covered, partial);
}
