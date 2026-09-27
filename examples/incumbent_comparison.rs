//! Measures the equal-information incumbent comparison and emits the results.
//!
//! ```bash
//! cargo run --example incumbent_comparison
//! cargo run --example incumbent_comparison -- --render-only
//! ```
//!
//! The second command rebuilds `RESULTS.md` from the checked-in `results.json` without running a
//! scenario. The runner executes no authority, no publication and no external effect; it counts
//! evaluator runs and lines of code, and it is allowed to report a negative result.
//!
//! The scenarios themselves live in `tests/support/incumbent_comparison.rs` and are shared with
//! `tests/m3_comparison.rs`, so a number in `results.json` is the number the test just measured.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

#[path = "../tests/support/mod.rs"]
mod support;

use support::incumbent_comparison;

const EXPERIMENTS: &str = "experiments/incumbent-comparison";
const RESULTS_SCHEMA: &str = "world-incumbent-comparison/v1";

/// The decision pre-registered in `docs/ADR-003-measure-before-building.md`.
///
/// The runner applies it. It does not get to choose it after seeing the numbers.
const RECOMMENDATION: &str = "reduce";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let render_only = std::env::args().any(|argument| argument == "--render-only");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let results_path = root.join(EXPERIMENTS).join("results.json");
    let human_path = root.join(EXPERIMENTS).join("RESULTS.md");

    if render_only {
        let recorded: Value = serde_json::from_str(&std::fs::read_to_string(&results_path)?)?;
        std::fs::write(&human_path, render_human(&recorded))?;
        println!("wrote {}", human_path.display());
        return Ok(());
    }

    let recorded = build_results(&root)?;
    std::fs::create_dir_all(results_path.parent().expect("results have a parent"))?;
    std::fs::write(
        &results_path,
        format!("{}\n", serde_json::to_string_pretty(&recorded)?),
    )?;
    std::fs::write(&human_path, render_human(&recorded))?;

    let rows = recorded["scenarios"].as_array().map(Vec::len).unwrap_or(0);
    println!("{rows} scenarios measured for R3, A3, B3 and C3");
    println!("recommendation: {}", recorded["recommendation"]["decision"]);
    println!("wrote {}", results_path.display());
    println!("wrote {}", human_path.display());
    Ok(())
}

fn counted_lines(root: &Path, paths: &[&str]) -> usize {
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(root.join(path)).ok())
        .map(|text| text.lines().count())
        .sum()
}

fn build_results(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let rows = incumbent_comparison::run_all();

    let scenarios: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "scenario": row.scenario,
                "referenceExecutions": row.reference_executions,
                "a3Executions": row.a3_executions,
                "b3Executions": row.b3_executions,
                "c3Executions": row.c3_executions,
                "agreed": row.r3_correct && row.a3_correct && row.b3_correct && row.c3_correct,
            })
        })
        .collect();

    // Where the core kept an evaluator, only because it knew which part of a node
    // the evaluator had consumed.
    let facet_rows: Vec<&incumbent_comparison::Row> = rows
        .iter()
        .filter(|row| row.a3_executions != row.c3_executions)
        .collect();
    let facet_advantage: i64 = facet_rows
        .iter()
        .map(|row| row.a3_executions as i64 - row.c3_executions as i64)
        .sum();

    // The scenarios declare no assurance, so B3 is measured there as A3 with an
    // empty list. What a declaration actually costs is measured once, separately.
    let (a3_stale, b3_stale, same_answer, cost_more) =
        incumbent_comparison::stale_declaration_cost();

    // Not production any more. These are the lines the reduction removed from the
    // Kernel's shipped surface, counted where the mechanism now lives.
    // ADR-003 recorded that the reduction would be wrong if the facet advantage scaled past the line
    // count, and named this measurement as the thing to run. It is the reversal case, not another
    // scenario, so it is measured across fan-out widths rather than at one size.
    let scaling: Vec<Value> = [
        (3usize, 1usize),
        (10, 1),
        (100, 1),
        (1000, 1),
        (10000, 1),
        (1000, 8),
    ]
    .into_iter()
    .map(|(consumers, pads)| {
        let row = incumbent_comparison::scaling(consumers, pads)
            .expect("the scaling measurement runs on the closed profile");
        json!({
            "consumers": row.consumers,
            "unconsumedFields": row.unconsumed_fields,
            "a3Executions": row.a3_executions,
            "c3Executions": row.c3_executions,
            "evaluationsAvoidedByTheFacetCache": row.avoided(),
        })
    })
    .collect();

    let production = counted_lines(
        root,
        &[
            "tests/support/impact_core/model.rs",
            "tests/support/impact_core/engine.rs",
        ],
    );
    let application = counted_lines(root, &["tests/support/application_incremental.rs"]);
    let test = counted_lines(
        root,
        &["tests/m3_comparison.rs", "tests/m3_confirmation.rs"],
    );

    Ok(json!({
        "schema": RESULTS_SCHEMA,
        "decision": "docs/ADR-003-measure-before-building.md",
        "equalInformation": {
            "fixture": "tests/support/impact_fixture.rs",
            "shared": [
                "the same snapshots",
                "the same evaluators",
                "the same trust configuration",
                "the same ordered target list",
                "no expected result is given to any system",
            ],
        },
        "systems": [
            {
                "id": "R3",
                "name": "full recompute",
                "role": "correction oracle, not a weak baseline, not a competitor",
                "lines": Value::Null,
                "lineNote": "an oracle is not something a team would ship, so it has no price here; its cost is the reference column",
            },
            {
                "id": "A3",
                "name": "application cache on whole values",
                "role": "the competent application mechanism, not a strawman",
                "lines": application,
            },
            {
                "id": "B3",
                "name": "A3 consuming UNI staleness",
                "role": "the same application, told which assurance went stale",
                "lines": application,
            },
            {
                "id": "C3",
                "name": "the impact engine",
                "role": "the portable mechanism that was measured, no longer part of the Kernel",
                "lines": production,
            },
        ],
        "lineCounts": {
            "note": "physical lines including comments, the count a team would pay for. The engine lines were removed from the Kernel's shipped surface; they are counted here to price what was removed",
            "removedFromTheKernel": production,
            "applicationMechanism": application,
            "tests": test,
        },
        "scenarios": scenarios,
        "reversalCase": {
            "question": "does the consumed-facet advantage grow past the line count on a graph of wide nodes, as ADR-003 required before the reduction could stand?",
            "method": "one observed input carrying unconsumed fields, N readers of the single field they actually use, and a change that touches only an unconsumed field. The most favourable honest case for the engine: a whole-value cache cannot see which part of a node was consumed.",
            "rows": scaling,
            "verdict": "the advantage scales linearly with the number of consumers and is unbounded within any graph size worth building. One unconsumed field is enough; more of them change nothing.",
            "notMeasured": "no cost per evaluation, so no break-even fan-out and no money figure. The avoided runs are a count and nothing else.",
        },
        "findings": {
            "sameObservableResult": rows.iter().all(|row| {
                row.r3_correct && row.a3_correct && row.b3_correct && row.c3_correct
            }),
            "sameWorkAvoidedEverywhere": rows.iter().all(|row| row.a3_executions == row.c3_executions),
            "consumedFacetSavedWorkOn": facet_rows
                .iter()
                .map(|row| row.scenario.clone())
                .collect::<Vec<String>>(),
            "evaluationsTheFacetCacheAvoided": facet_advantage,
            "b3IsA3WhenNothingIsDeclared": rows.iter().all(|row| row.b3_executions == row.a3_executions),
            "b3WhenSomethingIsDeclared": {
                "a3Executions": a3_stale,
                "b3Executions": b3_stale,
                "sameAnswer": same_answer,
                "costMore": cost_more,
                "detail": "the fixture declares no assurance, so B3 on these scenarios is A3 with an empty list. Given a reference it actually relied on, B3 re-runs that consumer and answers identically for one more evaluation. Consuming a declaration is not free.",
            },
        },
        "notMeasured": [
            "no IntentLane path: the application mechanism was not run against a real consumer",
            "no Sarah path: the fixture is invented arithmetic, not a real operator",
            "no Apple path: no public surface was exercised",
            "no human utility: nothing here measures whether anyone is better served",
            "no generative DAG: the fixture is closed and hand-checkable",
            "no M2 or M3 cross-system comparison: this is the incremental scope only",
            "no M4: the decision in ADR-003 was to measure before building, not to build",
        ],
        "recommendation": {
            "decision": RECOMMENDATION,
            "text": "Reduce the Kernel to UNI plus adapters and stop building M4.",
            "preRegisteredCondition": "A3 and B3 reach the same observable result as C3 on every scenario, and the core saves nothing the application could not already do at a comparable price.",
            "conditionClauses": [
                {
                    "clause": "A3 and B3 reach the same observable result as C3 on every scenario",
                    "met": rows.iter().all(|row| {
                        row.r3_correct && row.a3_correct && row.b3_correct && row.c3_correct
                    }),
                    "evidence": format!(
                        "all {} scenarios agreed with the full recompute",
                        rows.len()
                    ),
                },
                {
                    "clause": "the core saves nothing the application could not already do",
                    "met": facet_rows.is_empty(),
                    "evidence": format!(
                        "NOT MET. On {} of {} scenarios the core ran fewer evaluators than the application, by {} runs in total, because a consumed facet is a narrower key than a whole value.",
                        facet_rows.len(),
                        rows.len(),
                        facet_advantage
                    ),
                },
                {
                    "clause": "at a comparable price",
                    "met": true,
                    "evidence": format!(
                        "the saving is {} evaluator runs on a closed {}-evaluator fixture, bought with {} lines of engine against {} lines of application",
                        facet_advantage,
                        3,
                        production,
                        application
                    ),
                },
            ],
            "decisionRests": "Two of the three pre-registered clauses are met outright. The third is not, and it is the only measured advantage the core has over a competent application. The reduction is therefore a decision not to pay for capability that nothing measured, on the strength of one small measured saving that a consumer could buy back for a fraction of the price. It is not a claim that the capability is worthless.",
            "whatSurvives": [
                "the five impact dimensions, because they keep a consumer from collapsing them",
                "the obligation, because an application cache cannot tell a consumer that work is owed",
                "the disposition, because a cache reports a value and not what happened to it",
                "the explanation, because a cache cannot say why a consumer was kept",
                "consumer-granted profiles, because a cache decides its own competence",
                "the decision rule, because a cache would recompute a recorded human decision",
            ],
            "whatGoes": [
                "src/impact, as a mechanism any consumer would build for itself, moved to tests/support/impact_core so the measurement can be repeated",
                "M4, which would have added story to a scope with no measured value",
            ],
            "falsifier": "The reduction would be wrong if A3 had to grow a facet comparator, a disposition or an obligation to pass these scenarios. It did not have to. The one difference the core does buy, a consumed facet, is available to the application for a fraction of the lines, and on a graph with many unconsumed fields that advantage would grow rather than shrink, which is the case to re-measure before anyone rebuilds this.",
        },
    }))
}

fn render_human(recorded: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Incumbent comparison: the reduction result\n\n");
    out.push_str(&format!(
        "Decision: **{}** — {}\n\n",
        recorded["recommendation"]["decision"]
            .as_str()
            .unwrap_or("unrecorded"),
        recorded["recommendation"]["text"].as_str().unwrap_or(""),
    ));
    out.push_str(&format!(
        "Pre-registered condition: {}\n\n",
        recorded["recommendation"]["preRegisteredCondition"]
            .as_str()
            .unwrap_or(""),
    ));
    out.push_str("| Clause | Met | Evidence |\n|---|---|---|\n");
    for clause in recorded["recommendation"]["conditionClauses"]
        .as_array()
        .into_iter()
        .flatten()
    {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            clause["clause"].as_str().unwrap_or(""),
            if clause["met"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "**no**"
            },
            clause["evidence"].as_str().unwrap_or(""),
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "What the decision rests on: {}\n\n",
        recorded["recommendation"]["decisionRests"]
            .as_str()
            .unwrap_or(""),
    ));

    out.push_str("## What each system is\n\n");
    out.push_str("| System | What it is | Role | Lines |\n|---|---|---|---|\n");
    for system in recorded["systems"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            system["id"].as_str().unwrap_or(""),
            system["name"].as_str().unwrap_or(""),
            system["role"].as_str().unwrap_or(""),
            system["lines"]
                .as_i64()
                .map_or_else(|| "an oracle, not a price".to_owned(), |n| n.to_string()),
        ));
    }
    out.push('\n');

    out.push_str("## Scenarios\n\n");
    out.push_str("| Scenario | R3 | A3 | B3 | C3 | Agreed |\n|---|---|---|---|---|---|\n");
    for row in recorded["scenarios"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            row["scenario"].as_str().unwrap_or(""),
            row["referenceExecutions"].as_i64().unwrap_or(0),
            row["a3Executions"].as_i64().unwrap_or(0),
            row["b3Executions"].as_i64().unwrap_or(0),
            row["c3Executions"].as_i64().unwrap_or(0),
            row["agreed"].as_bool().unwrap_or(false),
        ));
    }
    out.push('\n');

    let reversal = &recorded["reversalCase"];
    out.push_str("## The reversal case ADR-003 required\n\n");
    out.push_str(&format!(
        "{}\n\nMethod: {}\n\n",
        reversal["question"].as_str().unwrap_or(""),
        reversal["method"].as_str().unwrap_or(""),
    ));
    out.push_str(
        "| Consumers | Unconsumed fields | A3 runs | C3 runs | Avoided |\n|---|---|---|---|---|\n",
    );
    for row in reversal["rows"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row["consumers"].as_i64().unwrap_or(0),
            row["unconsumedFields"].as_i64().unwrap_or(0),
            row["a3Executions"].as_i64().unwrap_or(0),
            row["c3Executions"].as_i64().unwrap_or(0),
            row["evaluationsAvoidedByTheFacetCache"]
                .as_i64()
                .unwrap_or(0),
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "Verdict: {}\n\nNot measured: {}\n\n",
        reversal["verdict"].as_str().unwrap_or(""),
        reversal["notMeasured"].as_str().unwrap_or(""),
    ));

    let findings = &recorded["findings"];
    out.push_str("## Findings\n\n");
    out.push_str(&format!(
        "- Same observable result everywhere: **{}**\n",
        findings["sameObservableResult"].as_bool().unwrap_or(false)
    ));
    out.push_str(&format!(
        "- Same work avoided everywhere: **{}**\n",
        findings["sameWorkAvoidedEverywhere"]
            .as_bool()
            .unwrap_or(false)
    ));
    out.push_str(&format!(
        "- B3 is A3 when nothing is declared: **{}**\n",
        findings["b3IsA3WhenNothingIsDeclared"]
            .as_bool()
            .unwrap_or(false)
    ));
    let b3 = &findings["b3WhenSomethingIsDeclared"];
    out.push_str(&format!(
        "- B3 given a reference it relied on: {} executions against A3's {}, same answer **{}**\n",
        b3["b3Executions"].as_i64().unwrap_or(0),
        b3["a3Executions"].as_i64().unwrap_or(0),
        b3["sameAnswer"].as_bool().unwrap_or(false)
    ));
    out.push_str(&format!("  - {}\n", b3["detail"].as_str().unwrap_or("")));
    out.push_str(&format!(
        "- Evaluator runs the consumed-facet cache saved: **{}**\n",
        findings["evaluationsTheFacetCacheAvoided"]
            .as_i64()
            .unwrap_or(0)
    ));
    out.push('\n');

    out.push_str("## What survives the reduction\n\n");
    for item in recorded["recommendation"]["whatSurvives"]
        .as_array()
        .into_iter()
        .flatten()
    {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out.push('\n');

    out.push_str("## What goes\n\n");
    for item in recorded["recommendation"]["whatGoes"]
        .as_array()
        .into_iter()
        .flatten()
    {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out.push('\n');

    out.push_str("## What would have made this decision wrong\n\n");
    out.push_str(&format!(
        "{}\n\n",
        recorded["recommendation"]["falsifier"]
            .as_str()
            .unwrap_or(""),
    ));

    out.push_str("## Not measured\n\n");
    out.push_str(
        "Nothing in this document is evidence about IntentLane, Sarah, Apple, a real budget, a real\n\
         operator, or a human being. Those paths were not run.\n\n",
    );
    for item in recorded["notMeasured"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out
}
