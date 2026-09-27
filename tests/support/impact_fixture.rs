//! The closed arithmetic fixture from `docs/M3-PROTOCOL.md` section 12.
//!
//! This is an invented arithmetic fixture for tests. It is not Sarah, not
//! IntentLane, not a real budget and not a user decision. Its only job is to be
//! small enough that the incremental engine and the full recompute can be
//! compared by hand.
//!
//! `limit = 100`, `cost_a = 80`, `cost_b = 110`, `label = "draft"`.
//! `eligible_a = cost_a <= limit`, `eligible_b = cost_b <= limit`. A support
//! object consumes both booleans and nothing else. `label` exists only so that a
//! presentation change can be shown not to disturb the arithmetic.

use serde_json::{Value, json};
use world_kernel::impact::{
    Evaluator, Facet, Limits, NodeId, NodeNature, NodeVersion, Profile, Query, ReadJournal,
    Snapshot, TrustConfiguration,
};

pub const LIMIT: &str = "limit";
pub const COST_A: &str = "cost_a";
pub const COST_B: &str = "cost_b";
pub const LABEL: &str = "label";
pub const ELIGIBLE_A: &str = "eligible_a";
pub const ELIGIBLE_B: &str = "eligible_b";
pub const SUPPORT: &str = "support_a";
pub const DECISION: &str = "decision";

/// The nodes a consumer of this fixture may ask about.
pub const DERIVED: [&str; 3] = [ELIGIBLE_A, ELIGIBLE_B, SUPPORT];

/// A comparison that reads one facet of one node.
struct Compare {
    subject: &'static str,
    field: &'static str,
}

impl Compare {
    fn new(subject: &'static str, field: &'static str) -> Self {
        Self { subject, field }
    }

    fn run(
        &self,
        journal: &mut dyn ReadJournal,
    ) -> Result<Option<Value>, world_kernel::impact::Error> {
        journal.read(self.subject, Facet::Field(self.field.to_owned()))
    }
}

/// `eligible = cost <= limit`, reading the two numeric fields it actually uses.
pub struct Eligibility {
    pub name: &'static str,
    pub cost: &'static str,
}

impl Evaluator for Eligibility {
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
    ) -> Result<(Value, Profile), world_kernel::impact::Error> {
        let limit = Compare::new(LIMIT, "value").run(journal)?;
        let cost = Compare::new(self.cost, "value").run(journal)?;
        let limit_present = limit.is_some();
        let verdict = match (limit, cost) {
            (Some(limit), Some(cost)) => {
                let (Some(limit), Some(cost)) = (limit.as_f64(), cost.as_f64()) else {
                    return Err(world_kernel::impact::Error::LimitReached(
                        "non numeric fixture value".into(),
                    ));
                };
                json!({ "eligible": cost <= limit })
            }
            // A missing operand is an observed absence, not a silent zero.
            _ => {
                journal.read_absence(if !limit_present { LIMIT } else { self.cost })?;
                json!({ "eligible": null })
            }
        };
        Ok((verdict, Profile::ClosedDeterministic))
    }
}

/// Consumes the two booleans and nothing else. It never reads the numbers, so a
/// limit change that leaves both booleans alone leaves it reusable.
pub struct SupportEvaluator;

impl Evaluator for SupportEvaluator {
    fn name(&self) -> &str {
        SUPPORT
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
        let a = Compare::new(ELIGIBLE_A, "eligible").run(journal)?;
        let b = Compare::new(ELIGIBLE_B, "eligible").run(journal)?;
        // The support set is a set. Its order is not semantic and is normalised.
        let mut members: Vec<String> = Vec::new();
        for (label, value) in [("a", &a), ("b", &b)] {
            match value {
                Some(value) => members.push(format!("{label}:{}", value)),
                None => {
                    journal.read_absence(ELIGIBLE_A)?;
                    members.push(format!("{label}:unknown"));
                }
            }
        }
        members.sort();
        Ok((
            json!({
                "supports": members,
                "verdict": "supported",
            }),
            Profile::ClosedDeterministic,
        ))
    }
}

pub fn evaluators() -> std::collections::BTreeMap<NodeId, Box<dyn Evaluator>> {
    let mut map: std::collections::BTreeMap<NodeId, Box<dyn Evaluator>> =
        std::collections::BTreeMap::new();
    map.insert(
        ELIGIBLE_A.to_owned(),
        Box::new(Eligibility {
            name: ELIGIBLE_A,
            cost: COST_A,
        }),
    );
    map.insert(
        ELIGIBLE_B.to_owned(),
        Box::new(Eligibility {
            name: ELIGIBLE_B,
            cost: COST_B,
        }),
    );
    map.insert(SUPPORT.to_owned(), Box::new(SupportEvaluator));
    map
}

pub fn trust() -> TrustConfiguration {
    trust_for("1")
}

pub fn trust_for(version: &str) -> TrustConfiguration {
    TrustConfiguration::default()
        .grant(ELIGIBLE_A, version, Profile::ClosedDeterministic)
        .grant(ELIGIBLE_B, version, Profile::ClosedDeterministic)
        .grant(SUPPORT, version, Profile::ClosedDeterministic)
}

pub fn limits() -> Limits {
    Limits::default()
}

/// A snapshot built from the four inputs and the derived results they imply.
/// The same fixture with an extra, unconsumed field on the limit node.
///
/// Nothing in the arithmetic reads it, so a whole-value cache invalidates on it
/// and a consumed-facet cache does not. That difference is the point of the
/// field, and it is only visible in a scenario that changes it.
pub fn snapshot_with_note(revision: u64, limit: i64, note: &str) -> Snapshot {
    let mut snapshot = snapshot(revision, limit, 80, 110, "draft");
    snapshot = snapshot.with_node(
        LIMIT,
        NodeNature::Observed,
        NodeVersion::new(
            2,
            "money/1",
            json!({"value": limit, "note": note}),
            "producer",
        ),
    );
    snapshot
}

pub fn snapshot(revision: u64, limit: i64, cost_a: i64, cost_b: i64, label: &str) -> Snapshot {
    let eligible_a = cost_a <= limit;
    let eligible_b = cost_b <= limit;
    let mut supports = vec![format!("a:{}", eligible_a), format!("b:{}", eligible_b)];
    supports.sort();
    Snapshot::new("world:impact-demo", revision)
        .with_node(
            LIMIT,
            NodeNature::Observed,
            NodeVersion::new(1, "money/1", json!({"value": limit}), "producer"),
        )
        .with_node(
            COST_A,
            NodeNature::Observed,
            NodeVersion::new(1, "money/1", json!({"value": cost_a}), "producer"),
        )
        .with_node(
            COST_B,
            NodeNature::Observed,
            NodeVersion::new(1, "money/1", json!({"value": cost_b}), "producer"),
        )
        .with_node(
            LABEL,
            NodeNature::Observed,
            NodeVersion::new(1, "text/1", json!({"value": label}), "producer"),
        )
        .with_node(
            ELIGIBLE_A,
            NodeNature::Derived,
            NodeVersion::new(1, "verdict/1", json!({"eligible": eligible_a}), ELIGIBLE_A),
        )
        .with_node(
            ELIGIBLE_B,
            NodeNature::Derived,
            NodeVersion::new(1, "verdict/1", json!({"eligible": eligible_b}), ELIGIBLE_B),
        )
        .with_node(
            SUPPORT,
            NodeNature::Derived,
            NodeVersion::new(
                1,
                "support/1",
                json!({"supports": supports, "verdict": "supported"}),
                SUPPORT,
            ),
        )
        .with_node(
            DECISION,
            NodeNature::Decision,
            NodeVersion::new(
                1,
                "decision/1",
                json!({
                    "chosen": "a",
                    "justification": "option a was supported when the decision was taken",
                    "recorded_at_revision": 0,
                    "supports": [SUPPORT],
                }),
                "human:owner",
            ),
        )
}

/// A full evaluation of the fixture at one snapshot, as an independent oracle
/// would produce it. It never receives a record.
pub fn oracle(
    snapshot: &Snapshot,
    targets: &[String],
) -> Result<
    std::collections::BTreeMap<NodeId, world_kernel::impact::engine::Evaluated>,
    world_kernel::impact::Error,
> {
    world_kernel::impact::full_recompute(snapshot, targets, &evaluators(), &trust(), limits())
}

/// The set-level read this fixture's support evaluator could make, exposed so a
/// collection change can be exercised.
pub fn support_query() -> Query {
    Query {
        query: "all_eligibility".to_owned(),
        query_version: 1,
        scope: "world:impact-demo".to_owned(),
        members: vec![ELIGIBLE_A.to_owned(), ELIGIBLE_B.to_owned()],
    }
}
