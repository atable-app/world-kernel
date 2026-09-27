//! The comparison driver, shared by the test suite and the results runner.
//!
//! The test asserts the outcomes. The runner emits them. Both call the same code, so a number in
//! `results.json` cannot drift away from the number the test just measured.

use std::collections::BTreeMap;

use super::impact_core::{
    Evaluated, EvaluationRecord, NodeId, Revised, Snapshot, full_recompute, revise,
};
use serde_json::Value;

use super::application_incremental::{AppCache, revise_app, revise_b3};
use super::impact_fixture as fixture;

pub fn targets() -> Vec<NodeId> {
    fixture::DERIVED.iter().map(|id| (*id).to_owned()).collect()
}

/// R3: the full recompute. Its work is the reference for avoided work.
pub fn run_r3(
    snapshot: &Snapshot,
) -> Result<(BTreeMap<NodeId, Value>, usize), super::impact_core::Error> {
    let recomputed: BTreeMap<NodeId, Evaluated> = full_recompute(
        snapshot,
        &targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )?;
    let values = recomputed
        .iter()
        .map(|(id, evaluated)| (id.clone(), evaluated.value.clone()))
        .collect();
    Ok((values, targets().len()))
}

pub struct ApplicationRun {
    pub values: BTreeMap<NodeId, Value>,
    pub executions: usize,
    pub reused: usize,
}

pub fn run_a3(
    base: &Snapshot,
    target: &Snapshot,
) -> Result<ApplicationRun, super::impact_core::Error> {
    let mut cache = AppCache::new();
    // The application's own first pass establishes the cache, as an application
    // would have done before the conditions changed.
    let warm = revise_app(
        base,
        &targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )?;
    let outcome = revise_app(
        target,
        &targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )?;
    // Only the second pass is measured. The warm-up is what every system needs
    // before the conditions change, so counting it would flatter whichever
    // system happened to be measured differently.
    let _ = warm;
    Ok(ApplicationRun {
        values: outcome.values,
        executions: outcome.executions,
        reused: outcome.reused,
    })
}

pub fn run_b3(
    base: &Snapshot,
    target: &Snapshot,
    stale: &[String],
) -> Result<ApplicationRun, super::impact_core::Error> {
    let mut cache = AppCache::new();
    // The warm-up builds the cache before anything changes. The declaration
    // applies to the pass that would otherwise reuse it.
    let warm = revise_app(
        base,
        &targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )?;
    let outcome = revise_b3(
        target,
        &targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
        stale,
    )?;
    let _ = warm;
    Ok(ApplicationRun {
        values: outcome.values,
        executions: outcome.executions,
        reused: outcome.reused,
    })
}

pub fn run_c3(
    base: &Snapshot,
    target: &Snapshot,
) -> Result<(BTreeMap<NodeId, Value>, usize, usize), super::impact_core::Error> {
    let recorded: BTreeMap<NodeId, EvaluationRecord> = full_recompute(
        base,
        &targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )?
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect();

    let Revised {
        findings,
        evaluations,
        reused,
        ..
    } = revise(
        base,
        target,
        &recorded,
        &targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )?;

    let values = findings
        .iter()
        .filter_map(|(id, finding)| {
            finding
                .output
                .as_ref()
                .map(|value| (id.clone(), value.clone()))
        })
        .collect();
    Ok((values, evaluations, reused))
}

/// The comparison on one scenario. `expected` is the manual expectation for the
/// essential cases, so a mistake common to an evaluator cannot make all four
/// systems equally wrong.
pub struct Comparison {
    pub name: &'static str,
    pub base: Snapshot,
    pub target: Snapshot,
    pub expected: Vec<(NodeId, Value)>,
}

pub fn comparisons() -> Vec<Comparison> {
    vec![
        Comparison {
            name: "mutation_1_limit_100_to_90",
            base: fixture::snapshot(0, 100, 80, 110, "draft"),
            target: fixture::snapshot(1, 90, 80, 110, "draft"),
            expected: vec![
                (
                    fixture::ELIGIBLE_A.to_owned(),
                    serde_json::json!({"eligible": true}),
                ),
                (
                    fixture::ELIGIBLE_B.to_owned(),
                    serde_json::json!({"eligible": false}),
                ),
            ],
        },
        Comparison {
            name: "mutation_2_limit_90_to_70",
            base: fixture::snapshot(0, 90, 80, 110, "draft"),
            target: fixture::snapshot(1, 70, 80, 110, "draft"),
            expected: vec![(
                fixture::ELIGIBLE_A.to_owned(),
                serde_json::json!({"eligible": false}),
            )],
        },
        Comparison {
            name: "mutation_3_label_change",
            base: fixture::snapshot(0, 100, 80, 110, "draft"),
            target: fixture::snapshot(1, 100, 80, 110, "published"),
            expected: vec![(
                fixture::ELIGIBLE_A.to_owned(),
                serde_json::json!({"eligible": true}),
            )],
        },
        Comparison {
            name: "mutation_5_changed_input_same_output",
            base: fixture::snapshot(0, 100, 80, 110, "draft"),
            target: fixture::snapshot(1, 100, 85, 110, "draft"),
            expected: vec![(
                fixture::ELIGIBLE_A.to_owned(),
                serde_json::json!({"eligible": true}),
            )],
        },
        Comparison {
            name: "mutation_6_diamond",
            base: fixture::snapshot(0, 100, 80, 110, "draft"),
            target: fixture::snapshot(1, 120, 85, 120, "draft"),
            expected: vec![
                (
                    fixture::ELIGIBLE_A.to_owned(),
                    serde_json::json!({"eligible": true}),
                ),
                (
                    fixture::ELIGIBLE_B.to_owned(),
                    serde_json::json!({"eligible": true}),
                ),
            ],
        },
        Comparison {
            name: "unconsumed_field_change",
            base: fixture::snapshot_with_note(0, 100, "first"),
            target: fixture::snapshot_with_note(1, 100, "second"),
            expected: vec![
                (
                    fixture::ELIGIBLE_A.to_owned(),
                    serde_json::json!({"eligible": true}),
                ),
                (
                    fixture::ELIGIBLE_B.to_owned(),
                    serde_json::json!({"eligible": false}),
                ),
            ],
        },
        Comparison {
            name: "mutation_7_unchanged_world",
            base: fixture::snapshot(0, 100, 80, 110, "draft"),
            target: fixture::snapshot(1, 100, 80, 110, "draft"),
            expected: vec![(
                fixture::ELIGIBLE_A.to_owned(),
                serde_json::json!({"eligible": true}),
            )],
        },
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    pub scenario: String,
    pub r3_correct: bool,
    pub a3_correct: bool,
    pub b3_correct: bool,
    pub c3_correct: bool,
    pub a3_executions: usize,
    pub b3_executions: usize,
    pub c3_executions: usize,
    pub reference_executions: usize,
}

pub fn run_all() -> Vec<Row> {
    let mut rows = Vec::new();
    for comparison in comparisons() {
        let (r3_values, reference_executions) = run_r3(&comparison.target).expect("R3 runs");
        let a3 = run_a3(&comparison.base, &comparison.target).expect("A3 runs");
        let b3 = run_b3(&comparison.base, &comparison.target, &[]).expect("B3 runs");
        let (c3_values, c3_executions, _) =
            run_c3(&comparison.base, &comparison.target).expect("C3 runs");

        let expected: BTreeMap<NodeId, Value> = comparison.expected.iter().cloned().collect();
        let agrees = |values: &BTreeMap<NodeId, Value>| -> bool {
            expected
                .iter()
                .all(|(id, value)| values.get(id) == Some(value))
        };

        rows.push(Row {
            scenario: comparison.name.to_owned(),
            r3_correct: agrees(&r3_values),
            a3_correct: agrees(&a3.values),
            b3_correct: agrees(&b3.values),
            c3_correct: agrees(&c3_values),
            a3_executions: a3.executions,
            b3_executions: b3.executions,
            c3_executions,
            reference_executions,
        });
    }
    rows
}

/// What consuming a UNI staleness answer costs, measured on an unchanged world.
///
/// The scenarios in `comparisons` declare no assurance, so B3 there is literally
/// A3 with an empty list and the two are the same program. That is not evidence
/// that B3 is free; it is evidence that the fixture has nothing to declare. This
/// function gives B3 a reference it actually relied on, and reports what that
/// costs and what it buys.
pub fn stale_declaration_cost() -> (usize, usize, bool, bool) {
    let base = fixture::snapshot(0, 100, 80, 110, "draft");
    let target = fixture::snapshot(1, 100, 80, 110, "draft");

    let clean = run_a3(&base, &target).expect("A3 runs");
    // The support object reads eligible_a and eligible_b.
    let stale = run_b3(&base, &target, &[fixture::ELIGIBLE_A.to_owned()]).expect("B3 runs");

    (
        clean.executions,
        stale.executions,
        clean.values == stale.values,
        clean.executions < stale.executions,
    )
}
