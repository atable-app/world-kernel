//! Worlds the closed arithmetic fixture does not reach.
//!
//! `impact_fixture` is a fixed arithmetic graph, and three of the pre-registered M3 stories need
//! something it structurally cannot express: a read that switches sides, an observation that expires
//! because time moved rather than because the observation did, and a contradiction between two members
//! of one collection. Those worlds are built here instead of being bolted onto the original fixture, so
//! that no existing expectation moves when one of them is added.
//!
//! Everything domain-specific in this file is domain-specific on purpose. Which claim wins a
//! contradiction is a rule about claims, and it is written here rather than in `src/impact` so that
//! keeping the core free of domain rules stays a checkable claim rather than a habit.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use world_kernel::impact::{
    Evaluator, Facet, Limits, NodeId, NodeNature, NodeVersion, Profile, Query, ReadJournal,
    Revised, Snapshot, TrustConfiguration, full_recompute, revise,
};

/// The observable result of a pass, in the same shape `incremental_revision.rs` compares: values,
/// normalized obligations, coverage and currency. Run identifiers and durations are not compared.
#[derive(Debug, PartialEq, Eq)]
pub struct Observable {
    pub outputs: Vec<(String, String)>,
    pub obligations: Vec<String>,
    pub coverage: Vec<(String, String)>,
    pub currency: Vec<(String, String)>,
}

pub fn observable(revised: &Revised) -> Observable {
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
        .map(|obligation| format!("{}:{}", obligation.target, obligation.work.as_str()))
        .collect();
    obligations.sort();
    let mut coverage: Vec<(String, String)> = revised
        .findings
        .iter()
        .map(|(id, finding)| (id.clone(), format!("{:?}", finding.coverage)))
        .collect();
    coverage.sort();
    let mut currency: Vec<(String, String)> = revised
        .findings
        .iter()
        .map(|(id, finding)| (id.clone(), format!("{:?}", finding.currency)))
        .collect();
    currency.sort();
    Observable {
        outputs,
        obligations,
        coverage,
        currency,
    }
}

/// The oracle's produced values, as the same shape the plan reports.
pub fn oracle_observable(
    evaluated: &BTreeMap<NodeId, world_kernel::impact::Evaluated>,
) -> Vec<(String, String)> {
    let mut outputs: Vec<(String, String)> = evaluated
        .iter()
        .map(|(id, node)| (id.clone(), node.value.to_string()))
        .collect();
    outputs.sort();
    outputs
}

/// The equivalence property, asserted over what the oracle actually produced.
///
/// A recorded decision has no evaluator, so the oracle never produces a value for it and it is not
/// compared here. Its own expectations are asserted by the tests that need it.
pub fn assert_agrees_with_oracle(
    revised: &Revised,
    oracle: &BTreeMap<NodeId, world_kernel::impact::Evaluated>,
) {
    for (id, node) in oracle {
        let finding = revised
            .findings
            .get(id)
            .unwrap_or_else(|| panic!("{id} has a finding"));
        let produced = finding
            .output
            .as_ref()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_owned());
        assert_eq!(
            produced,
            node.value.to_string(),
            "{id} equals the full recompute"
        );
    }
}

// ---------------------------------------------------------------- M3-11: a conditional read

pub const SOURCE_A: &str = "source_a";
pub const SOURCE_B: &str = "source_b";
pub const SOURCE_C: &str = "source_c";
pub const SELECTOR: &str = "selector";
pub const ROUTED: &str = "routed";
pub const ROUTED_CONSUMER: &str = "routed_consumer";
pub const BYSTANDER: &str = "bystander";

/// Reads one side of a conditional and declares which side it took.
///
/// The selector is read through the journal, so a switch invalidates the record by the ordinary
/// dependency rule and the new read set names the side actually consumed. `declare_branch` exists so
/// the record says which side that was instead of leaving it to be inferred by diffing reads.
struct Routed;

impl Evaluator for Routed {
    fn name(&self) -> &str {
        ROUTED
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let selector = journal.read(SELECTOR, Facet::Field("value".to_owned()))?;
        let chosen = match selector.as_ref().and_then(Value::as_str) {
            Some("b") => SOURCE_B,
            // An unknown selector falls back to the first source rather than inventing a third.
            _ => SOURCE_A,
        };
        journal.declare_branch(chosen);
        let value = journal
            .read(chosen, Facet::Field("value".to_owned()))?
            .unwrap_or(Value::Null);
        Ok((
            json!({ "routed": value, "from": chosen }),
            Profile::ClosedDeterministic,
        ))
    }
}

/// Reads the routed value and nothing else.
struct SeenRouted;

impl Evaluator for SeenRouted {
    fn name(&self) -> &str {
        ROUTED_CONSUMER
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let seen = journal
            .read(ROUTED, Facet::Field("routed".to_owned()))?
            .unwrap_or(Value::Null);
        Ok((json!({ "seen": seen }), Profile::ClosedDeterministic))
    }
}

/// Reads a source no conditional touches, so a switch can be shown not to disturb it.
struct Bystander;

impl Evaluator for Bystander {
    fn name(&self) -> &str {
        BYSTANDER
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let value = journal
            .read(SOURCE_C, Facet::Field("value".to_owned()))?
            .unwrap_or(Value::Null);
        Ok((json!({ "seen": value }), Profile::ClosedDeterministic))
    }
}

pub fn conditional_world(revision: u64, selector: &str, source_a: i64, source_b: i64) -> Snapshot {
    // A snapshot carries the derived values a previous pass produced, which is the convention the
    // arithmetic fixture also follows. Without it the oracle would read a placeholder where a
    // consumer reads its dependency, and would disagree with a correct incremental answer.
    let (routed, from) = if selector == "b" {
        (source_b, SOURCE_B)
    } else {
        (source_a, SOURCE_A)
    };
    Snapshot::new("world:conditional", revision)
        .with_node(
            SOURCE_A,
            NodeNature::Observed,
            NodeVersion::new(1, "int/1", json!({"value": source_a}), "producer"),
        )
        .with_node(
            SOURCE_B,
            NodeNature::Observed,
            NodeVersion::new(1, "int/1", json!({"value": source_b}), "producer"),
        )
        .with_node(
            SOURCE_C,
            NodeNature::Observed,
            NodeVersion::new(1, "int/1", json!({"value": 7}), "producer"),
        )
        .with_node(
            SELECTOR,
            NodeNature::Observed,
            NodeVersion::new(1, "text/1", json!({"value": selector}), "producer"),
        )
        .with_node(
            ROUTED,
            NodeNature::Derived,
            NodeVersion::new(
                1,
                "route/1",
                json!({"routed": routed, "from": from}),
                ROUTED,
            ),
        )
        .with_node(
            ROUTED_CONSUMER,
            NodeNature::Derived,
            NodeVersion::new(1, "seen/1", json!({"seen": routed}), ROUTED_CONSUMER),
        )
        .with_node(
            BYSTANDER,
            NodeNature::Derived,
            NodeVersion::new(1, "seen/1", json!({"seen": 7}), BYSTANDER),
        )
}

pub fn conditional_targets() -> Vec<NodeId> {
    vec![
        ROUTED.to_owned(),
        ROUTED_CONSUMER.to_owned(),
        BYSTANDER.to_owned(),
    ]
}

pub fn conditional_evaluators() -> BTreeMap<NodeId, Box<dyn Evaluator>> {
    let mut map: BTreeMap<NodeId, Box<dyn Evaluator>> = BTreeMap::new();
    map.insert(ROUTED.to_owned(), Box::new(Routed));
    map.insert(ROUTED_CONSUMER.to_owned(), Box::new(SeenRouted));
    map.insert(BYSTANDER.to_owned(), Box::new(Bystander));
    map
}

pub fn conditional_trust() -> TrustConfiguration {
    TrustConfiguration::default()
        .grant(ROUTED, "1", Profile::ClosedDeterministic)
        .grant(ROUTED_CONSUMER, "1", Profile::ClosedDeterministic)
        .grant(BYSTANDER, "1", Profile::ClosedDeterministic)
}

/// Records a pass over the conditional world and returns it beside the oracle's answer for the same
/// snapshot. Every conditional test asserts both halves.
pub fn conditional_pass(
    base: &Snapshot,
    target: &Snapshot,
    recorded: &BTreeMap<NodeId, world_kernel::impact::EvaluationRecord>,
) -> (Revised, Observable) {
    let revised = revise(
        base,
        target,
        recorded,
        &conditional_targets(),
        &conditional_evaluators(),
        &conditional_trust(),
        Limits::default(),
    )
    .expect("the conditional world is within limits");
    let oracle = full_recompute(
        target,
        &conditional_targets(),
        &conditional_evaluators(),
        &conditional_trust(),
        Limits::default(),
    )
    .expect("the oracle evaluates the same world");
    assert_agrees_with_oracle(&revised, &oracle);
    let observable = observable(&revised);
    (revised, observable)
}

pub fn conditional_records(
    snapshot: &Snapshot,
) -> BTreeMap<NodeId, world_kernel::impact::EvaluationRecord> {
    full_recompute(
        snapshot,
        &conditional_targets(),
        &conditional_evaluators(),
        &conditional_trust(),
        Limits::default(),
    )
    .expect("the conditional world evaluates")
    .into_iter()
    .map(|(id, node)| (id, node.record))
    .collect()
}

// ---------------------------------------------------------------- M3-15: expiry on explicit time

pub const OBSERVATION: &str = "observation";
pub const CLOCK: &str = "clock";
pub const FRESHNESS: &str = "freshness";
pub const ARCHIVED: &str = "archived";
pub const SUMMARY: &str = "summary";

/// Reads the observation and the clock. Time is an ordinary input here: nothing about the
/// observation changed when it expired, only the clock did, and a cache keyed on the observation
/// alone would serve a value that is no longer true.
struct Freshness;

impl Evaluator for Freshness {
    fn name(&self) -> &str {
        FRESHNESS
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let value = journal.read(OBSERVATION, Facet::Field("value".to_owned()))?;
        let expires_at = journal.read(OBSERVATION, Facet::Field("expires_at".to_owned()))?;
        let now = journal.read(CLOCK, Facet::Field("now".to_owned()))?;
        let present = value.is_some() && expires_at.is_some() && now.is_some();
        let fresh = match (
            expires_at.as_ref().and_then(Value::as_i64),
            now.as_ref().and_then(Value::as_i64),
        ) {
            (Some(expires_at), Some(now)) => present && now <= expires_at,
            // An observation without a readable expiry is not silently fresh.
            _ => {
                if expires_at.is_none() {
                    journal.read_absence(OBSERVATION)?;
                }
                false
            }
        };
        Ok((
            json!({ "value": value, "fresh": fresh }),
            Profile::ClosedDeterministic,
        ))
    }
}

/// Reads only the observation. A clock change must not disturb it.
struct Archived;

impl Evaluator for Archived {
    fn name(&self) -> &str {
        ARCHIVED
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let value = journal
            .read(OBSERVATION, Facet::Field("value".to_owned()))?
            .unwrap_or(Value::Null);
        Ok((json!({ "value": value }), Profile::ClosedDeterministic))
    }
}

/// Reads freshness and nothing else, so a clock move that leaves the verdict alone can be shown to
/// stop propagating.
struct Summary;

impl Evaluator for Summary {
    fn name(&self) -> &str {
        SUMMARY
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let fresh = journal
            .read(FRESHNESS, Facet::Field("fresh".to_owned()))?
            .unwrap_or(Value::Null);
        Ok((json!({ "fresh": fresh }), Profile::ClosedDeterministic))
    }
}

/// `expires_at` and `value` never move between these two calls; only the clock does.
pub fn timed_world(revision: u64, now: i64) -> Snapshot {
    let fresh = now <= 20;
    Snapshot::new("world:timed", revision)
        .with_node(
            OBSERVATION,
            NodeNature::Observed,
            NodeVersion::new(
                1,
                "reading/1",
                json!({"value": 42, "expires_at": 20}),
                "producer",
            ),
        )
        .with_node(
            CLOCK,
            NodeNature::Observed,
            NodeVersion::new(1, "clock/1", json!({"now": now}), "clock"),
        )
        .with_node(
            FRESHNESS,
            NodeNature::Derived,
            NodeVersion::new(
                1,
                "freshness/1",
                json!({"value": 42, "fresh": fresh}),
                FRESHNESS,
            ),
        )
        .with_node(
            ARCHIVED,
            NodeNature::Derived,
            NodeVersion::new(1, "archive/1", json!({"value": 42}), ARCHIVED),
        )
        .with_node(
            SUMMARY,
            NodeNature::Derived,
            NodeVersion::new(1, "summary/1", json!({"fresh": fresh}), SUMMARY),
        )
}

pub fn timed_targets() -> Vec<NodeId> {
    vec![
        FRESHNESS.to_owned(),
        ARCHIVED.to_owned(),
        SUMMARY.to_owned(),
    ]
}

pub fn timed_evaluators() -> BTreeMap<NodeId, Box<dyn Evaluator>> {
    let mut map: BTreeMap<NodeId, Box<dyn Evaluator>> = BTreeMap::new();
    map.insert(FRESHNESS.to_owned(), Box::new(Freshness));
    map.insert(ARCHIVED.to_owned(), Box::new(Archived));
    map.insert(SUMMARY.to_owned(), Box::new(Summary));
    map
}

pub fn timed_trust() -> TrustConfiguration {
    TrustConfiguration::default()
        .grant(FRESHNESS, "1", Profile::ClosedDeterministic)
        .grant(ARCHIVED, "1", Profile::ClosedDeterministic)
        .grant(SUMMARY, "1", Profile::ClosedDeterministic)
}

pub fn timed_records(
    snapshot: &Snapshot,
) -> BTreeMap<NodeId, world_kernel::impact::EvaluationRecord> {
    full_recompute(
        snapshot,
        &timed_targets(),
        &timed_evaluators(),
        &timed_trust(),
        Limits::default(),
    )
    .expect("the timed world evaluates")
    .into_iter()
    .map(|(id, node)| (id, node.record))
    .collect()
}

/// A pass over the timed world, asserted against the oracle before it is returned.
pub fn timed_pass(
    base: &Snapshot,
    target: &Snapshot,
    recorded: &BTreeMap<NodeId, world_kernel::impact::EvaluationRecord>,
) -> (Revised, Observable) {
    let revised = revise(
        base,
        target,
        recorded,
        &timed_targets(),
        &timed_evaluators(),
        &timed_trust(),
        Limits::default(),
    )
    .expect("the timed world is within limits");
    let oracle = full_recompute(
        target,
        &timed_targets(),
        &timed_evaluators(),
        &timed_trust(),
        Limits::default(),
    )
    .expect("the oracle evaluates the same world");
    assert_agrees_with_oracle(&revised, &oracle);
    let observable = observable(&revised);
    (revised, observable)
}

// ---------------------------------------------------------------- M3-18: a contradiction

pub const CLAIM_ALICE: &str = "claim_alice";
pub const CLAIM_BOB: &str = "claim_bob";
pub const ADJUDICATED: &str = "adjudicated";
pub const CLAIM_DECISION: &str = "claim_decision";

/// The declared arbitration rule, in one place.
///
/// The highest declared priority wins; a tie is refused rather than broken arbitrarily; the number of
/// contradictions observed is reported instead of being quietly resolved. Both the evaluator and the
/// snapshot builder call this, so a placeholder can never drift from what the evaluator would produce.
fn adjudicate(subject: Option<String>, claims: Vec<(i64, String, Value)>) -> Value {
    let mut conflicts = 0usize;
    if claims.len() > 1 {
        let first = &claims[0].2;
        conflicts = claims.iter().filter(|(_, _, value)| value != first).count();
    }
    let winner = claims
        .iter()
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    let tied =
        winner.is_some_and(|best| claims.iter().filter(|claim| claim.0 == best.0).count() > 1);
    json!({
        "subject": subject,
        "winner": if tied { Value::Null } else { json!(winner.map(|w| w.1.clone()).unwrap_or_default()) },
        "value": if tied { Value::Null } else { winner.map(|w| w.2.clone()).unwrap_or(Value::Null) },
        "claims": claims.len(),
        "conflicts": conflicts,
        "refused": tied,
    })
}

/// Builds one claim in the shape the adjudicator reads.
fn claim_payload(subject: &str, claimant: &str, priority: i64, value: i64) -> Value {
    json!({"subject": subject, "claimant": claimant, "priority": priority, "value": value})
}

/// Arbitrates the claims in one collection.
///
/// The rule is deliberately a rule about claims rather than about graphs: the claim carrying the
/// higher declared priority wins, a tie is refused rather than broken arbitrarily, and the number of
/// contradictions observed is reported instead of being quietly resolved. It lives in this fixture so
/// that `src/impact` stays free of it.
struct Adjudicator;

impl Evaluator for Adjudicator {
    fn name(&self) -> &str {
        ADJUDICATED
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let members = journal.read_query(&Query {
            query: "every_claim".to_owned(),
            query_version: 1,
            scope: "world:claims".to_owned(),
            members: vec![CLAIM_ALICE.to_owned(), CLAIM_BOB.to_owned()],
        })?;

        let mut subject: Option<String> = None;
        let mut claims: Vec<(i64, String, Value)> = Vec::new();
        for (id, _) in &members {
            let payload = journal.read(id, Facet::Whole)?.unwrap_or(Value::Null);
            let Some((claimed_subject, priority, claimant, value)) = (|| {
                Some((
                    payload.get("subject")?.as_str()?.to_owned(),
                    payload.get("priority")?.as_i64()?,
                    payload.get("claimant")?.as_str()?.to_owned(),
                    payload.get("value")?.clone(),
                ))
            })() else {
                continue;
            };
            if subject
                .as_ref()
                .is_some_and(|previous| previous != &claimed_subject)
            {
                continue;
            }
            subject = Some(claimed_subject);
            claims.push((priority, claimant, value));
        }

        Ok((adjudicate(subject, claims), Profile::ClosedDeterministic))
    }
}

/// The claims world. `bob` carries his declared priority so that a tie can be built as easily as a
/// resolution, because the rule has to be exercised on both sides of it.
pub fn claims_world(revision: u64, bob: Option<i64>) -> Snapshot {
    let alice = claim_payload("plan", "alice", 1, 1);
    let bob_payload = bob.map(|priority| claim_payload("plan", "bob", priority, 2));

    let mut present: Vec<(i64, String, Value)> =
        vec![(1, "alice".to_owned(), alice["value"].clone())];
    if let Some(payload) = &bob_payload {
        present.push((
            payload["priority"].as_i64().expect("a priority"),
            "bob".to_owned(),
            payload["value"].clone(),
        ));
    }
    let placeholder = adjudicate(Some("plan".to_owned()), present);

    let mut snapshot = Snapshot::new("world:claims", revision).with_node(
        CLAIM_ALICE,
        NodeNature::Observed,
        NodeVersion::new(1, "claim/1", alice, "producer"),
    );
    if let Some(payload) = bob_payload {
        snapshot = snapshot.with_node(
            CLAIM_BOB,
            NodeNature::Observed,
            NodeVersion::new(1, "claim/1", payload, "producer"),
        );
    }
    snapshot
        .with_node(
            ADJUDICATED,
            NodeNature::Derived,
            NodeVersion::new(1, "adjudication/1", placeholder, ADJUDICATED),
        )
        .with_node(
            CLAIM_DECISION,
            NodeNature::Decision,
            NodeVersion::new(
                1,
                "decision/1",
                json!({
                    "chosen": "alice",
                    "justification": "alice's claim was the only one recorded",
                    "supports": [ADJUDICATED],
                }),
                "human:owner",
            ),
        )
}

/// The claims world with the decision's declared support chosen by the caller, so a swap can be
/// built while every other byte of the decision stays exactly as it was.
///
/// M3-17 is about a difference that is deliberately invisible to anything comparing wording. The
/// justification and the chosen name below are copied verbatim from `claims_world`; only the support
/// list moves, and only a reader that looks at the declaration rather than at the text can see it.
pub fn claims_world_supporting(revision: u64, bob: Option<i64>, support: &str) -> Snapshot {
    claims_world_with_unrelated(revision, bob).with_node(
        CLAIM_DECISION,
        NodeNature::Decision,
        NodeVersion::new(
            1,
            "decision/1",
            json!({
                "chosen": "alice",
                "justification": "alice's claim was the only one recorded",
                "supports": [support],
            }),
            "human:owner",
        ),
    )
}

/// The unrelated node. A contradiction must not disturb work that never read the collection.
pub const UNRELATED: &str = "unrelated";

struct Unrelated;

impl Evaluator for Unrelated {
    fn name(&self) -> &str {
        UNRELATED
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let value = journal
            .read(CLAIM_ALICE, Facet::Field("priority".to_owned()))?
            .unwrap_or(Value::Null);
        Ok((json!({ "priority": value }), Profile::ClosedDeterministic))
    }
}

pub fn claims_world_with_unrelated(revision: u64, bob: Option<i64>) -> Snapshot {
    let snapshot = claims_world(revision, bob);
    // The unrelated node only ever reads the claim that exists in both worlds, so a second claim
    // arriving is invisible to it.
    snapshot.with_node(
        UNRELATED,
        NodeNature::Derived,
        NodeVersion::new(1, "seen/1", json!({"priority": 1}), UNRELATED),
    )
}

/// What a pass is asked about. The recorded decision is included: it has no evaluator, so the
/// engine must answer for it rather than recompute it.
pub fn claims_targets() -> Vec<NodeId> {
    vec![
        ADJUDICATED.to_owned(),
        CLAIM_DECISION.to_owned(),
        UNRELATED.to_owned(),
    ]
}

/// What the oracle can actually evaluate. It receives no record and runs no evaluator it was not
/// given, so a decision node has no place in it and is checked by its own expectations instead.
pub fn claims_evaluatable_targets() -> Vec<NodeId> {
    vec![ADJUDICATED.to_owned(), UNRELATED.to_owned()]
}

pub fn claims_evaluators() -> BTreeMap<NodeId, Box<dyn Evaluator>> {
    let mut map: BTreeMap<NodeId, Box<dyn Evaluator>> = BTreeMap::new();
    map.insert(ADJUDICATED.to_owned(), Box::new(Adjudicator));
    map.insert(UNRELATED.to_owned(), Box::new(Unrelated));
    map
}

pub fn claims_trust() -> TrustConfiguration {
    TrustConfiguration::default()
        .grant(ADJUDICATED, "1", Profile::ClosedDeterministic)
        .grant(UNRELATED, "1", Profile::ClosedDeterministic)
}

pub fn claims_records(
    snapshot: &Snapshot,
) -> BTreeMap<NodeId, world_kernel::impact::EvaluationRecord> {
    full_recompute(
        snapshot,
        &claims_evaluatable_targets(),
        &claims_evaluators(),
        &claims_trust(),
        Limits::default(),
    )
    .expect("the claims world evaluates")
    .into_iter()
    .map(|(id, node)| (id, node.record))
    .collect()
}

/// A pass over the claims world, asserted against the oracle before it is returned.
pub fn claims_pass(
    base: &Snapshot,
    target: &Snapshot,
    recorded: &BTreeMap<NodeId, world_kernel::impact::EvaluationRecord>,
) -> (Revised, Observable) {
    let revised = revise(
        base,
        target,
        recorded,
        &claims_targets(),
        &claims_evaluators(),
        &claims_trust(),
        Limits::default(),
    )
    .expect("the claims world is within limits");
    let oracle = full_recompute(
        target,
        &claims_evaluatable_targets(),
        &claims_evaluators(),
        &claims_trust(),
        Limits::default(),
    )
    .expect("the oracle evaluates the same world");
    assert_agrees_with_oracle(&revised, &oracle);
    let observable = observable(&revised);
    (revised, observable)
}
