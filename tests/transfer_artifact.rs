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
        let rendered = rendered_verdict(&human, name);
        match &condition["met"] {
            // A condition nobody can reach is not a condition that failed, and the document has to keep
            // the two apart: rendering both as no is how a blocked slice comes to look like a settled one.
            Value::Bool(false) => assert_eq!(
                rendered.as_deref(),
                Some("**unreachable**"),
                "{name} is unreachable, so RESULTS.md must not render it as a failure"
            ),
            Value::String(reason) if reason == "not_met" => assert_eq!(
                rendered.as_deref(),
                Some("**no**"),
                "{name} is not met in results.json, so RESULTS.md must not claim it is"
            ),
            Value::String(reason) if reason == "undecidable" => assert_eq!(
                rendered.as_deref(),
                Some("**undecidable**"),
                "{name} is undecidable, so RESULTS.md must not present it as settled either way"
            ),
            _ => {}
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

const COMPLEXITY_CONDITION: &str =
    "the competent baseline does not give the same result at lower complexity";

/// Condition 5 was recorded as `undecidable`, on the stated ground that a line count does not answer
/// whether the baseline is lower complexity. The brief nominates the same thing twice over: `core_loc`
/// is one of its own secondary metrics, and its own reduction trigger is a baseline reaching the same
/// safety and reuse at materially lower complexity. Declining to score the trigger on the metric the
/// trigger is written in is a gate that was written down and not executed, which is the third time this
/// campaign has produced one.
#[test]
fn the_complexity_condition_is_scored_rather_than_declined() {
    let recorded = recorded();

    let condition = recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions")
        .iter()
        .find(|condition| condition["condition"].as_str() == Some(COMPLEXITY_CONDITION))
        .expect("the complexity condition is one of the seven");

    assert_ne!(
        condition["met"],
        Value::String("undecidable".to_owned()),
        "the condition is decidable: the brief nominates core_loc as a metric and writes its reduction \
         trigger in terms of complexity, so the recorded verdict may not be that it cannot be scored"
    );
}

#[test]
fn the_complexity_verdict_follows_from_the_recorded_numbers() {
    let recorded = recorded();

    let kernel = recorded["lineCounts"]["kernelSurface"]
        .as_u64()
        .expect("the Kernel surface is counted");
    let baseline = recorded["lineCounts"]["baselineA"]
        .as_u64()
        .expect("the baseline surface is counted");
    assert!(
        baseline < kernel,
        "fixture: the baseline is meant to be smaller"
    );

    let condition = recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions")
        .iter()
        .find(|condition| condition["condition"].as_str() == Some(COMPLEXITY_CONDITION))
        .expect("the complexity condition is one of the seven");

    // Same result, established by the tie recorded above the gate, and fewer lines. The brief's condition
    // is that the baseline does **not** give the same result at lower complexity, so it is not met.
    assert_eq!(
        condition["met"],
        Value::String("not_met".to_owned()),
        "the baseline gives the same result in {baseline} lines against the Kernel's {kernel}, which is \
         what the condition says must not happen"
    );
}

#[test]
fn the_complexity_verdict_names_the_ratio_and_survives_the_threshold() {
    let recorded = recorded();
    let detail = recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions")
        .iter()
        .find(|condition| condition["condition"].as_str() == Some(COMPLEXITY_CONDITION))
        .expect("the complexity condition is one of the seven")["detail"]
        .as_str()
        .expect("the condition carries its reason")
        .to_owned();

    let kernel = recorded["lineCounts"]["kernelSurface"]
        .as_u64()
        .unwrap_or(0) as f64;
    let baseline = recorded["lineCounts"]["baselineA"].as_u64().unwrap_or(1) as f64;
    let ratio = kernel / baseline;

    assert!(
        detail.contains(&format!("{ratio:.2}")),
        "the reason states the ratio it depends on, so a reader can check the arithmetic: {detail}"
    );
    assert!(
        ratio < 2.0,
        "the ratio is under two, so any threshold a reader might pick for 'clearly lower' is reached and \
         the verdict does not depend on where the threshold is put"
    );
}

#[test]
fn the_verdict_line_counts_every_condition_rather_than_asserting_three() {
    let recorded = recorded();
    let verdict = recorded["continuationGate"]["verdict"]
        .as_str()
        .expect("the gate states its verdict")
        .to_owned();

    let conditions = recorded["continuationGate"]["conditions"]
        .as_array()
        .expect("the gate carries its conditions");
    let met = conditions
        .iter()
        .filter(|condition| condition["met"] == Value::Bool(true))
        .count();
    let not_met = conditions
        .iter()
        .filter(|condition| condition["met"] == Value::String("not_met".to_owned()))
        .count();
    let unreachable = conditions
        .iter()
        .filter(|condition| condition["met"] == Value::Bool(false))
        .count();

    assert!(
        verdict.contains(&format!("{met} met")),
        "the verdict says how many are met and {met} are"
    );
    assert!(
        verdict.contains(&format!("{not_met} not met")),
        "the verdict says how many are not met and {not_met} are"
    );
    assert!(
        verdict.contains(&format!("{unreachable} unreachable")),
        "the verdict says how many are unreachable and {unreachable} are"
    );
    assert_eq!(
        met + not_met + unreachable,
        conditions.len(),
        "and the three counts must cover every condition, or one is being left out of the arithmetic"
    );
}
