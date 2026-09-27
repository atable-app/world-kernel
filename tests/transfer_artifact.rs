//! The human-readable transfer artifact must agree with the JSON it is rendered from.
//!
//! `AGENTS.md` already binds `experiments/incumbent-comparison/results.json` to a fresh measurement, so an
//! artifact that disagrees with its own run is a known failure mode here rather than a new worry. This file
//! covers the other half of the same invariant: the *rendering*, not the measurement.
//!
//! The defect this was written for is specific and was found by reading the checked-in document rather
//! than by running anything. `examples/transfer_benchmark.rs` writes each condition's verdict with
//!
//! ```text
//! match condition["met"].as_str() { Some("true") => "yes", ... _ => "**no**" }
//! ```
//!
//! and the JSON holds `met` as a **boolean** for three of the seven conditions. `as_str()` returns `None`
//! for a boolean, so every met condition fell through to the catch-all and rendered as `**no**`. The
//! summary line underneath the table is a separate string in the same file and read "three met", so the
//! document contradicted itself two paragraphs apart, and it did so on every run.

use std::fs;

use serde_json::Value;

/// What the renderer produces for one recorded condition.
fn rendered_verdict(document: &str, condition: &str) -> Option<String> {
    document
        .lines()
        .find(|line| line.starts_with(&format!("| {} |", condition)))
        .and_then(|line| line.rsplit('|').nth(1).map(|cell| cell.trim().to_owned()))
}

fn recorded() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/experiments/transfer-benchmark/results.json"
    );
    serde_json::from_str(&fs::read_to_string(path).expect("the recorded result is checked in"))
        .expect("the recorded result is valid JSON")
}

fn human() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/experiments/transfer-benchmark/RESULTS.md"
    );
    fs::read_to_string(path).expect("the generated view is checked in")
}

#[test]
fn a_condition_the_json_calls_met_is_rendered_as_met() {
    let recorded = recorded();
    let human = human();

    let met: Vec<String> = recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions")
        .iter()
        .filter(|condition| condition["met"] == Value::Bool(true))
        .map(|condition| {
            condition["condition"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();

    assert!(
        !met.is_empty(),
        "the recorded gate calls at least one condition met, or the metric table above it is wrong too"
    );

    for condition in met {
        assert_eq!(
            rendered_verdict(&human, &condition).as_deref(),
            Some("yes"),
            "{condition} is met in results.json, so RESULTS.md must say yes and not no"
        );
    }
}

#[test]
fn a_condition_the_json_calls_unmet_is_rendered_as_unmet() {
    let recorded = recorded();
    let human = human();

    for condition in recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions")
    {
        let name = condition["condition"].as_str().unwrap_or_default();
        if condition["met"] == Value::Bool(false) {
            assert_eq!(
                rendered_verdict(&human, name).as_deref(),
                Some("**no**"),
                "{name} is not met in results.json, so RESULTS.md must not claim it is"
            );
        }
    }
}

#[test]
fn the_table_and_the_summary_below_it_agree() {
    let recorded = recorded();
    let human = human();

    let met = recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions")
        .iter()
        .filter(|condition| condition["met"] == Value::Bool(true))
        .count();

    // Only the gate's own rows: start at its header and stop at the first blank line. The mutation
    // table further down the document has rows that also end in "yes", and a count that swept the whole
    // file counted those instead.
    let mut rows = 0usize;
    let mut in_table = false;
    for line in human.lines() {
        if line.starts_with("| Condition | Met |") {
            in_table = true;
            continue;
        }
        if in_table {
            if !line.starts_with('|') {
                break;
            }
            if line.trim_end().ends_with("| yes |") {
                rows += 1;
            }
        }
    }
    let rendered_yes = rows;

    assert_eq!(
        rendered_yes, met,
        "the verdict line reports how many conditions are met, so the table above it must render that many as yes"
    );
}
