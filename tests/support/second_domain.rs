//! The second domain: message routing, and the adapter that carries it onto the core.
//!
//! M3-24 asks whether a core frozen for the first domain applies to a second one without the core
//! acquiring that domain's rules. The answer has to be checkable rather than asserted, so this file
//! is written against `world_kernel::impact` exactly as the arithmetic fixture, the conditional and
//! timed worlds and the claims world are, and it changes nothing in `src/impact`. The three files
//! there are byte-identical before this one exists and after it does; the cost of the second domain
//! to the core was zero lines, which is what H3-Transfert measures.
//!
//! The shape is deliberately unlike the others so the transfer is not a copy. This world is a
//! *collection with a query*: a set-level read through `ReadJournal::read_query`, which none of the
//! other fixtures exercise, is the entry point of the graph, and the arithmetic fixture has no
//! equivalent. Everything domain-specific is here: what a routing table is, which region a message
//! claims, and what a delivery notice says.
//!
//! The builder and the evaluators share their payload functions, which is the same agreement
//! `m3_stories::adjudicate` keeps between a snapshot's placeholder and the evaluator that will
//! recompute it. Without it the oracle would be comparing a placeholder against a different value
//! and every claim below would be about that instead.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use world_kernel::impact::{
    Evaluator, Facet, Limits, NodeId, NodeNature, NodeVersion, Profile, Query, ReadJournal,
    Revised, Snapshot, TrustConfiguration, full_recompute, revise,
};

pub const WORLD: &str = "world:messages";

pub const MSG_ONE: &str = "message_one";
pub const MSG_TWO: &str = "message_two";
pub const MSG_THREE: &str = "message_three";
pub const ROUTING_TABLE: &str = "routing_table";
pub const PENDING: &str = "pending";
pub const ASSIGNED: &str = "assigned";
pub const DELIVERY: &str = "delivery";
pub const NOTICE: &str = "notice";

pub fn message_ids() -> [&'static str; 3] {
    [MSG_ONE, MSG_TWO, MSG_THREE]
}

/// One message as the adapter describes it.
#[derive(Debug, Clone, Copy)]
pub struct Message {
    pub route: &'static str,
    /// Read by no evaluator in this domain. It exists so a change nobody consumes can be shown to
    /// be a change nobody consumes.
    pub preview: &'static str,
    pub version: u64,
}

impl Message {
    pub fn new(route: &'static str, preview: &'static str) -> Self {
        Self {
            route,
            preview,
            version: 1,
        }
    }

    pub fn at_version(mut self, version: u64) -> Self {
        self.version = version;
        self
    }
}

pub const DEFAULT_MESSAGES: [Message; 3] = [
    Message {
        route: "eu",
        preview: "first",
        version: 1,
    },
    Message {
        route: "us",
        preview: "second",
        version: 1,
    },
    Message {
        route: "eu",
        preview: "third",
        version: 1,
    },
];

// ------------------------------------------------------- the domain's own rules

pub fn pending_payload(awaiting: &[String]) -> Value {
    json!({ "awaiting": awaiting })
}

pub fn assigned_payload(primary: &str, routes: &[String]) -> Value {
    let mut eu: Vec<String> = Vec::new();
    let mut us: Vec<String> = Vec::new();
    for (index, route) in routes.iter().enumerate() {
        let id = message_ids()[index].to_owned();
        if route.as_str() == "us" {
            us.push(id);
        } else {
            eu.push(id);
        }
    }
    json!({ "primary": primary, "eu": eu, "us": us })
}

pub fn delivery_payload(assigned: &Value) -> Value {
    let count = assigned["eu"].as_array().map_or(0, Vec::len)
        + assigned["us"].as_array().map_or(0, Vec::len);
    json!({ "delivered": count })
}

pub fn notice_payload(delivery: &Value) -> Value {
    json!({ "notice": format!("{} messages routed", delivery["delivered"]) })
}

// ------------------------------------------------------------------ the adapter

/// The whole second domain as a snapshot, including the placeholders the evaluators will recompute.
pub fn messages_world(revision: u64, primary: &str, messages: [Message; 3]) -> Snapshot {
    let routes: Vec<String> = messages
        .iter()
        .map(|message| message.route.to_owned())
        .collect();
    let assigned = assigned_payload(primary, &routes);
    let delivery = delivery_payload(&assigned);
    let notice = notice_payload(&delivery);
    let awaiting: Vec<String> = message_ids().iter().map(|id| (*id).to_owned()).collect();

    let mut snapshot = Snapshot::new(WORLD, revision);
    for (index, message) in messages.iter().enumerate() {
        snapshot = snapshot.with_node(
            message_ids()[index],
            NodeNature::Observed,
            NodeVersion::new(
                message.version,
                "message/1",
                json!({
                    "route": message.route,
                    "preview": message.preview,
                    "size": message.preview.len(),
                }),
                "adapter",
            ),
        );
    }
    snapshot
        .with_node(
            ROUTING_TABLE,
            NodeNature::Observed,
            NodeVersion::new(1, "table/1", json!({ "primary": primary }), "adapter"),
        )
        .with_node(
            PENDING,
            NodeNature::Derived,
            NodeVersion::new(1, "derived/1", pending_payload(&awaiting), PENDING),
        )
        .with_node(
            ASSIGNED,
            NodeNature::Derived,
            NodeVersion::new(1, "derived/1", assigned.clone(), ASSIGNED),
        )
        .with_node(
            DELIVERY,
            NodeNature::Derived,
            NodeVersion::new(1, "derived/1", delivery.clone(), DELIVERY),
        )
        .with_node(
            NOTICE,
            NodeNature::Derived,
            NodeVersion::new(1, "derived/1", notice, NOTICE),
        )
}

/// The set the application declares the query over. It is resolved by the caller, so the query
/// stays a set-level declaration rather than an ambient search over the world.
pub fn awaiting_query() -> Query {
    Query {
        query: "messages/awaiting-route".to_owned(),
        query_version: 1,
        scope: WORLD.to_owned(),
        members: message_ids().iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn field(
    journal: &mut dyn ReadJournal,
    subject: &str,
    name: &str,
) -> Result<String, world_kernel::impact::Error> {
    Ok(journal
        .read(subject, Facet::Field(name.to_owned()))?
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "eu".to_owned()))
}

/// Reads the set of messages still awaiting a route.
struct Awaiting;

impl Evaluator for Awaiting {
    fn name(&self) -> &str {
        PENDING
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
        let members = journal.read_query(&awaiting_query())?;
        let awaiting: Vec<String> = members.into_iter().map(|(id, _)| id).collect();
        Ok((pending_payload(&awaiting), Profile::ClosedDeterministic))
    }
}

/// Reads the table and the region each message claims. It never reads `preview`.
struct Assigning;

impl Evaluator for Assigning {
    fn name(&self) -> &str {
        ASSIGNED
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
        let primary = field(journal, ROUTING_TABLE, "primary")?;
        let routes = message_ids()
            .iter()
            .map(|id| field(journal, id, "route"))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            assigned_payload(&primary, &routes),
            Profile::ClosedDeterministic,
        ))
    }
}

struct Delivering;

impl Evaluator for Delivering {
    fn name(&self) -> &str {
        DELIVERY
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
        let assigned = journal.read(ASSIGNED, Facet::Whole)?.unwrap_or(Value::Null);
        Ok((delivery_payload(&assigned), Profile::ClosedDeterministic))
    }
}

struct Noticing;

impl Evaluator for Noticing {
    fn name(&self) -> &str {
        NOTICE
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
        let delivery = journal.read(DELIVERY, Facet::Whole)?.unwrap_or(Value::Null);
        Ok((notice_payload(&delivery), Profile::ClosedDeterministic))
    }
}

pub fn targets() -> Vec<NodeId> {
    vec![
        PENDING.to_owned(),
        ASSIGNED.to_owned(),
        DELIVERY.to_owned(),
        NOTICE.to_owned(),
    ]
}

pub fn evaluators() -> BTreeMap<NodeId, Box<dyn Evaluator>> {
    let mut map: BTreeMap<NodeId, Box<dyn Evaluator>> = BTreeMap::new();
    map.insert(PENDING.to_owned(), Box::new(Awaiting));
    map.insert(ASSIGNED.to_owned(), Box::new(Assigning));
    map.insert(DELIVERY.to_owned(), Box::new(Delivering));
    map.insert(NOTICE.to_owned(), Box::new(Noticing));
    map
}

pub fn trust() -> TrustConfiguration {
    TrustConfiguration::default()
        .grant(PENDING, "1", Profile::ClosedDeterministic)
        .grant(ASSIGNED, "1", Profile::ClosedDeterministic)
        .grant(DELIVERY, "1", Profile::ClosedDeterministic)
        .grant(NOTICE, "1", Profile::ClosedDeterministic)
}

/// The starting records, taken by a full pass over the base snapshot.
pub fn records_at(snapshot: &Snapshot) -> BTreeMap<NodeId, world_kernel::impact::EvaluationRecord> {
    full_recompute(
        snapshot,
        &targets(),
        &evaluators(),
        &trust(),
        Limits::default(),
    )
    .expect("the message routing world evaluates")
    .into_iter()
    .map(|(id, node)| (id, node.record))
    .collect()
}

/// An incremental pass over the second domain.
///
/// The oracle runs beside it, so every caller is comparing the core against a full recompute of the
/// same snapshot rather than against an answer anyone wrote down.
pub fn pass(
    base: &Snapshot,
    target: &Snapshot,
    recorded: &BTreeMap<NodeId, world_kernel::impact::EvaluationRecord>,
) -> (Revised, crate::support::m3_stories::Observable) {
    let revised = revise(
        base,
        target,
        recorded,
        &targets(),
        &evaluators(),
        &trust(),
        Limits::default(),
    )
    .expect("the message routing world is within limits");
    let oracle = full_recompute(
        target,
        &targets(),
        &evaluators(),
        &trust(),
        Limits::default(),
    )
    .expect("the oracle evaluates the same world");
    crate::support::m3_stories::assert_agrees_with_oracle(&revised, &oracle);
    let observable = crate::support::m3_stories::observable(&revised);
    (revised, observable)
}
