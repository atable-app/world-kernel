//! The Alpha/Beta fixture and an oracle that never sees the expected answer.
//!
//! This is invented arithmetic for tests. It is not Sarah, not IntentLane, not a real budget and not a
//! real decision. The oracle is deliberately written as a separate, simpler program over plain integers,
//! so that a bug in `src/branch.rs` and a bug in the expectation cannot cancel out. It reads no expected
//! value, no case id and no property named `expected`.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use world_kernel::branch::{Branch, Rule};
use world_kernel::impact::{NodeNature, NodeVersion, Snapshot};

pub const CAPACITY: &str = "capacity";
pub const LOAD_A: &str = "load_a";
pub const LOAD_B: &str = "load_b";

/// The base world: capacity 100, two loads of 40, one rule over their sum.
pub fn s0() -> Snapshot {
    world("world:branch-demo", 0, 100, 40, 40)
}

/// A world with the three named values and the rule node. `revision` is the world's own revision, which
/// is not the same thing as a branch revision or a content digest.
pub fn world(id: &str, revision: u64, capacity: i64, load_a: i64, load_b: i64) -> Snapshot {
    Snapshot::new(id, revision)
        .with_node(
            CAPACITY,
            NodeNature::Observed,
            NodeVersion::new(1, "capacity/1", json!({"value": capacity}), "planning"),
        )
        .with_node(
            LOAD_A,
            NodeNature::Observed,
            NodeVersion::new(1, "load/1", json!({"value": load_a}), "planning"),
        )
        .with_node(
            LOAD_B,
            NodeNature::Observed,
            NodeVersion::new(1, "load/1", json!({"value": load_b}), "planning"),
        )
        .with_node(
            "total",
            NodeNature::Derived,
            NodeVersion::new(1, "total/1", json!({"value": load_a + load_b}), "total"),
        )
}

/// A branch off `base` with one load changed. The base is copied, so a later move of the world cannot
/// reach into it.
pub fn branch_with(id: &str, base: &Snapshot, load: &str, value: i64) -> Branch {
    let mut branch = Branch::fork(id, base);
    let revision = branch.head.revision + 1;
    branch.head = branch.head.clone().with_node(
        load,
        NodeNature::Observed,
        NodeVersion::new(2, "planning/2", json!({"value": value}), "planning"),
    );
    branch.head.revision = revision;
    branch
}

/// A branch that changes both loads. Tranche 1 adopts from one source at a time, so a combination within
/// a single proposal needs one branch that moved both fields, not two branches.
pub fn branch_with_both(id: &str, base: &Snapshot, load_a: i64, load_b: i64) -> Branch {
    let mut branch = branch_with(id, base, load_a_field(), load_a);
    branch.head = branch.head.clone().with_node(
        load_b_field(),
        NodeNature::Observed,
        NodeVersion::new(2, "planning/2", json!({"value": load_b}), "planning"),
    );
    branch
}

fn load_a_field() -> &'static str {
    LOAD_A
}

fn load_b_field() -> &'static str {
    LOAD_B
}

/// The domain's rule, expressed over the candidate's own values.
pub fn capacity_rule() -> Rule {
    ("load_sum_within_capacity", |snapshot: &Snapshot| {
        let value = |id: &str| -> Option<i64> {
            snapshot
                .node(id)
                .and_then(|node| node.payload.get("value"))
                .and_then(Value::as_i64)
        };
        match (value(CAPACITY), value(LOAD_A), value(LOAD_B)) {
            (Some(capacity), Some(a), Some(b)) if a + b > capacity => Some(format!(
                "load_a {a} + load_b {b} = {} exceeds capacity {capacity}",
                a + b
            )),
            _ => None,
        }
    })
}

pub fn rules() -> Vec<Rule> {
    vec![capacity_rule()]
}

/// The oracle: a separate program over plain integers.
///
/// It materialises the values, applies the requested selection against the source branch's own view of
/// the base, and recomputes the sum from scratch. It calls nothing in `world_kernel::branch`, so a bug in
/// the composition planner cannot hide behind a matching bug here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleExpectation {
    pub capacity: i64,
    pub load_a: i64,
    pub load_b: i64,
    pub satisfies: bool,
    pub reason: String,
}

/// Recompute what a proposal would produce, independently.
pub fn oracle(
    target: &(i64, i64, i64),
    source_view: &(i64, i64, i64),
    base: &(i64, i64, i64),
    take_a: bool,
    take_b: bool,
) -> OracleExpectation {
    let (capacity, target_a, target_b) = *target;
    let (source_capacity, source_a, source_b) = *source_view;
    let (base_capacity, base_a, base_b) = *base;

    // The target diverged on a field if its value differs from the shared base's.
    let target_changed_a = target_a != base_a;
    let target_changed_b = target_b != base_b;

    let load_a = if take_a && !target_changed_a {
        source_a
    } else {
        target_a
    };
    let load_b = if take_b && !target_changed_b {
        source_b
    } else {
        target_b
    };
    let capacity = if source_capacity != base_capacity && capacity == base_capacity {
        source_capacity
    } else {
        capacity
    };

    let total = load_a + load_b;
    let satisfies = total <= capacity;
    OracleExpectation {
        capacity,
        load_a,
        load_b,
        satisfies,
        reason: if satisfies {
            format!("{load_a} + {load_b} = {total} fits within {capacity}")
        } else {
            format!("{load_a} + {load_b} = {total} exceeds capacity {capacity}")
        },
    }
}

/// Read a snapshot's three values back, for comparing an oracle expectation with a real candidate.
pub fn values_of(snapshot: &Snapshot) -> (i64, i64, i64) {
    let value = |id: &str| -> i64 {
        snapshot
            .node(id)
            .and_then(|node| node.payload.get("value"))
            .and_then(Value::as_i64)
            .unwrap_or_else(|| panic!("{id} has no integer value"))
    };
    (value(CAPACITY), value(LOAD_A), value(LOAD_B))
}

/// A store of branches, so a test can hold the two alternatives without inventing a persistence layer.
pub fn store(entries: Vec<Branch>) -> BTreeMap<String, Branch> {
    entries
        .into_iter()
        .map(|branch| (branch.id.clone(), branch))
        .collect()
}
