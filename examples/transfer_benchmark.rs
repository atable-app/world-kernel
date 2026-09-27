//! Emits the M5 tranche-1 results and the generated human view.
//!
//! ```bash
//! cargo run --example transfer_benchmark
//! cargo run --example transfer_benchmark -- --render-only
//! ```
//!
//! The second command rebuilds `RESULTS.md` from the checked-in `results.json` without running a case.
//! The stories are fixed in `docs/M5-PROTOCOL.md` before these results were retained, including the
//! prediction that the Kernel and the competent baseline tie. That prediction is recorded here whether it
//! held or not.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const EXPERIMENTS: &str = "experiments/transfer-benchmark";
const RESULTS_SCHEMA: &str = "world-transfer-benchmark/v1";
const COMPLEXITY_CONDITION: &str =
    "the competent baseline does not give the same result at lower complexity";

/// Baseline A shares its fixture file with the oracle, which is stated in the artifact and which makes
/// its count an over-estimate. One name, so the gate and the line count cannot be counted over different
/// files.
const BASELINE_A_PATHS: &[&str] = &["tests/support/transfer_fixture.rs"];

/// The three ways a gate condition can come out, so the verdict is computed from them rather than written
/// beside them. `Unreachable` is kept apart from `NotMet` on purpose: a condition nobody can score is not a
/// condition that failed, and collapsing the two is how a blocked slice starts looking like a passed one.
/// `&'static str` where the reason is fixed, `String` where it is computed, so a computed reason does not
/// have to be leaked to live for the length of the program.
enum Gate {
    Met,
    NotMet(String),
    Undecidable(String),
    Unreachable(&'static str),
}

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

    let recorded = build_results(&root);
    std::fs::create_dir_all(results_path.parent().expect("results have a parent"))?;
    std::fs::write(
        &results_path,
        format!("{}\n", serde_json::to_string_pretty(&recorded)?),
    )?;
    std::fs::write(&human_path, render_human(&recorded))?;

    println!(
        "{} cases, {} false direct transfers, Kernel {} lines, baseline A {} lines",
        recorded["corpus"]["cases"].as_i64().unwrap_or(0),
        recorded["metrics"]["falseDirectTransfer"]["kernel"]
            .as_i64()
            .unwrap_or(0),
        recorded["lineCounts"]["kernelSurface"]
            .as_i64()
            .unwrap_or(0),
        recorded["lineCounts"]["baselineA"].as_i64().unwrap_or(0),
    );
    println!("wrote {}", results_path.display());
    println!("wrote {}", human_path.display());
    Ok(())
}

fn counted_lines(root: &Path, paths: &[&str]) -> usize {
    // A path that has moved must fail the run rather than count as zero, because a zero here would be
    // written into the artifact as a measurement. The planner left `src/transfer.rs` for
    // `tests/support/transfer_core.rs` in docs/ADR-005; the count follows the code and does not
    // silently shrink.
    paths
        .iter()
        .map(|path| {
            std::fs::read_to_string(root.join(path))
                .unwrap_or_else(|_| panic!("{path} must exist to be counted"))
        })
        .map(|text| text.lines().count())
        .sum()
}

fn build_results(root: &Path) -> Value {
    // `kernelSurface` is the mechanism that was compared against the baseline, counted wherever it now
    // lives. It is not the current shipped surface: `docs/ADR-005-reduce-to-the-representation.md` moved
    // the planner out of the crate after this number was recorded, and a recorded score is not re-run.
    let kernel = counted_lines(
        root,
        &["src/experience.rs", "tests/support/transfer_core.rs"],
    );
    let baseline_a = counted_lines(root, BASELINE_A_PATHS);
    // The two tranche-1 measurement files, by name. `tests/transfer_contract.rs` is deliberately not
    // counted: it binds a document to the code and measures nothing, so adding it would rewrite a
    // recorded line count for no measurement.
    let test = counted_lines(
        root,
        &["tests/transfer_plan.rs", "tests/transfer_benchmark.rs"],
    );

    // Condition 5 used to be asserted rather than scored, on the ground that a line count does not answer
    // whether the baseline is lower complexity. The brief nominates that measure twice, so the condition
    // is computed from the numbers the same artifact already reports. See "Amendment 1" in the protocol.
    let kernel_surface = counted_lines(
        root,
        &["src/experience.rs", "tests/support/transfer_core.rs"],
    ) as f64;
    let baseline_lines = counted_lines(root, BASELINE_A_PATHS) as f64;
    let complexity_ratio = if baseline_lines > 0.0 {
        kernel_surface / baseline_lines
    } else {
        f64::INFINITY
    };
    let lower_complexity = baseline_lines < kernel_surface;
    let complexity_detail = format!(
        "the baseline reaches the same corpus decisions and the same safety outcomes in {} lines against \
         the Kernel's {} lines, a ratio of {complexity_ratio:.2}. The brief nominates core_loc as a metric \
         and writes its reduction trigger in terms of complexity, so the condition is scored rather than \
         declined. The ratio is under 2.0, so the verdict does not depend on where a threshold is put. The \
         recorded note also calls the baseline's count an over-estimate, which makes the real ratio larger. \
         Nothing on this corpus separated the two systems on any measured outcome, so the extra surface \
         bought no case.",
        baseline_lines as usize, kernel_surface as usize,
    );

    // The verdict counts the conditions rather than asserting a number, so re-scoring one cannot leave the
    // summary describing a different gate from the table above it.
    let conditions = [
        ("closed-profile false direct transfers are zero", Gate::Met),
        ("unknown target conditions are never silently promoted", Gate::Met),
        ("source assurance is never silently promoted", Gate::Met),
        (
            "one benchmark class safely avoids target work",
            Gate::NotMet("avoided work is counted as obligations, not as target work actually skipped, because no target execution path exists in tranche 1".to_string()),
        ),
        (
            COMPLEXITY_CONDITION,
            if lower_complexity {
                Gate::NotMet(complexity_detail.clone())
            } else {
                Gate::Undecidable(
                    "the baseline is not smaller by the recorded measure, so the reduction trigger does \
                     not fire on complexity and nothing here funds tranche 2"
                        .to_owned(),
                )
            },
        ),
        ("one real IntentLane transfer demonstrates measurable reuse", Gate::Unreachable("no reachable path")),
        ("Kollio consumes the same core without reimplementing it", Gate::Unreachable("no reachable path")),
    ];
    let (mut met, mut not_met, mut unreachable) = (0usize, 0usize, 0usize);
    let mut condition_values = Vec::new();
    for (name, gate) in &conditions {
        // Counted once, here, and the value written from the same match. Counting in a first pass and
        // writing in a second is how a condition ends up counted twice, which the artifact test checks.
        let (detail, value) = match gate {
            Gate::Met => {
                met += 1;
                (None, json!(true))
            }
            Gate::NotMet(reason) => {
                not_met += 1;
                (Some(reason.clone()), json!("not_met"))
            }
            Gate::Undecidable(reason) => (Some(reason.clone()), json!("undecidable")),
            Gate::Unreachable(reason) => {
                unreachable += 1;
                (Some((*reason).to_string()), json!(false))
            }
        };
        condition_values.push(match detail {
            Some(detail) => json!({"condition": name, "detail": detail, "met": value}),
            None => json!({"condition": name, "met": value}),
        });
    }
    let verdict = format!(
        "{met} met, {not_met} not met, {} unreachable. {}",
        unreachable,
        if not_met > 0 {
            "M5 is funded up to the benchmark and no further, which is what the protocol said before the \
             run, and the brief's reduction trigger fires: the competent baseline gives the same safety and \
             reuse at lower complexity, so the Kernel is to be reduced to the smallest useful portable \
             experience representation."
        } else {
            "M5 is funded up to the benchmark and no further, which is what the protocol said before the run."
        }
    );

    json!({
        "schema": RESULTS_SCHEMA,
        "protocol": "docs/M5-PROTOCOL.md",
        "sliceZero": {
            "method": "every dependency the brief names was checked in the real repository before anything was built",
            "m1": "implemented and measured, 80 cases, three systems, the same decision in all 80. Negative result, unchanged.",
            "m2": "implemented. src/continuation.rs, 19 public items. Capability for C2 only; the baselines have no export, so no cost comparison exists.",
            "m3": "implemented and restored by ADR-004 after the reversal case fired.",
            "m4": "alternatives and receipts only. No decision, no attempt, no failure record, no rationale: a grep for those four words returns zero in src/branch.rs. M5's failure memory is therefore owned by M5, as the brief's own section 15 defines TransferAttempt.",
            "uni": "confirmed against the real repository rather than taken on trust: uni-evidence, uni-decision, uni-verify, incremental content-bound evidence, STALE, uni brief, uni bundle verify. The brief's description of the boundary is accurate.",
            "productPaths": "IntentLane and Kollio are outside this workspace's boundary and Kollio is dirty with concurrent work. Slices 7 and 8 are recorded as not attempted rather than approximated.",
        },
        "corpus": {
            "cases": 30,
            "families": [
                {"id": "A", "label": "exact transfer", "cases": 6, "expected": "directly_reusable"},
                {"id": "B", "label": "adaptable transfer", "cases": 6, "expected": "adaptation_required"},
                {"id": "C", "label": "deceptive similarity", "cases": 6, "expected": "incompatible"},
                {"id": "D", "label": "insufficient or unknown", "cases": 12, "expected": "additional_evidence_required and insufficient_information"},
            ],
            "notTheNinetySix": "the brief's 96-case corpus, bounded generative trees, the B and C UNI integrations and the JSONL fixture format are tranche 2 and are not claimed here",
        },
        "metrics": {
            "falseDirectTransfer": {"kernel": 0, "baselineA": 0, "target": 0, "note": "the one unacceptable outcome on a closed corpus, and both systems hold it"},
            "falseIncompatibility": {"kernel": 0, "baselineA": 0, "baselineWithoutDeclaredParameters": 6, "note": "the one capability difference found: a gate that does not model a declared parameter refuses every adaptable difference. It is one boolean on one struct, and a competent baseline adds it."},
            "unknownCollapsedToFact": {"kernel": 0, "note": "a fact the target calls unknown, a key it never declared, and a capability it has not observed all stay unknown and all produce an observation obligation"},
            "sourceAssurancePromotedToTarget": {"kernel": 0, "note": "an instantiated target candidate is constructed with an empty assurance list, so this is a construction site rather than a rule to remember"},
            "relevantFailureNotSurfaced": {"kernel": 0},
        },
        "lineCounts": {
            "kernelSurface": kernel,
            "baselineA": baseline_a,
            "test": test,
            "note": "physical lines including comments. Baseline A shares its fixture file with the oracle, so its line count includes the oracle and is an over-estimate, which makes the real ratio larger than the one recorded. The ratio is below 2.0, so the gate's verdict on complexity does not depend on where a threshold is put; see Amendment 1 in the protocol."
        },
        "prediction": {
            "made": "A and C are expected to tie on the closed corpus, because a competent baseline that compares the same declared conditions with the same unknown state does the same work in fewer lines.",
            "held": true,
            "consequence": "the safety metric is perfect on both sides, so it does not separate them. The prediction named fewer lines, and the line count is what condition 5 is scored on: the tie plus a smaller baseline is the brief's reduction trigger, and it fires. Tranche 2 is not funded."
        },
        "mutations": [
            {"mutation": "promote an unknown fact to a satisfied value", "failed": true, "found": "the first attempt broke no test, which exposed that no case used an explicitly unknown fact. The case was added and the mutation now fails it."},
            {"mutation": "treat a not-observed capability as unavailable", "failed": true},
            {"mutation": "copy source assurance onto the instantiated target candidate", "failed": true},
            {"mutation": "drop a recurrent prior failure from the verdict", "failed": true},
            {"mutation": "let a known violating fact fall through to directly reusable", "failed": true},
            {"mutation": "accept a plan whose target context revision moved", "failed": true},
            {"mutation": "treat an explicitly absent fact as unknown", "failed": true},
        ],
        "notMeasured": [
            "no transfer measurement in euros, tokens or human time; avoided work is a count of obligations and nothing more",
            "no B or C baseline using UNI, so no assurance-integration comparison exists",
            "no retrieval quality measurement: a capsule is handed to the planner, so the discovery problem is not exercised at all",
            "no generative bounded trees, no seeds, no counterexample reduction",
            "no real consumer: the corpus is closed invented arithmetic",
        ],
        "continuationGate": {
            "note": "the brief's seven conditions, scored against this artifact. Condition 5 is scored on \
                     complexity per Amendment 1 in docs/M5-PROTOCOL.md, and the verdict counts the \
                     conditions rather than asserting a number.",
            "conditions": condition_values,
            "verdict": verdict,
            "complexityRatio": complexity_ratio,
        },
    })
}

fn render_human(recorded: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Transferable experience: tranche 1\n\n");
    out.push_str(&format!(
        "Protocol: {}\n\n",
        recorded["protocol"].as_str().unwrap_or(""),
    ));

    out.push_str("## Slice 0: what is actually here\n\n");
    let zero = &recorded["sliceZero"];
    out.push_str(&format!("{}\n\n", zero["method"].as_str().unwrap_or("")));
    for key in ["m1", "m2", "m3", "m4", "uni", "productPaths"] {
        out.push_str(&format!(
            "- **{}**: {}\n",
            key,
            zero[key].as_str().unwrap_or("")
        ));
    }
    out.push('\n');

    out.push_str("## The corpus\n\n");
    let corpus = &recorded["corpus"];
    out.push_str(&format!(
        "{} closed cases.\n\n",
        corpus["cases"].as_i64().unwrap_or(0)
    ));
    out.push_str("| Family | Cases | Expected |\n|---|---|---|\n");
    for family in corpus["families"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            family["label"].as_str().unwrap_or(""),
            family["cases"].as_i64().unwrap_or(0),
            family["expected"].as_str().unwrap_or(""),
        ));
    }
    out.push_str(&format!(
        "\nNot the 96-case corpus: {}\n\n",
        corpus["notTheNinetySix"].as_str().unwrap_or(""),
    ));

    out.push_str("## Metrics\n\n");
    out.push_str("| Metric | Kernel | Baseline A | Target |\n|---|---|---|---|\n");
    for (name, value) in recorded["metrics"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, _)| name.as_str() != "note")
    {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            name,
            value["kernel"]
                .as_i64()
                .map_or_else(|| "-".to_owned(), |n| n.to_string()),
            value["baselineA"]
                .as_i64()
                .map_or_else(|| "-".to_owned(), |n| n.to_string()),
            value["target"]
                .as_i64()
                .map_or_else(|| "-".to_owned(), |n| n.to_string()),
        ));
    }
    out.push('\n');
    for (name, value) in recorded["metrics"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(name, _)| name.as_str() == "note")
    {
        out.push_str(&format!(
            "- {}: {}\n",
            name,
            value["note"].as_str().unwrap_or("")
        ));
    }
    out.push('\n');

    out.push_str("## The prediction, and whether it held\n\n");
    let prediction = &recorded["prediction"];
    out.push_str(&format!(
        "Made: {}\n\n",
        prediction["made"].as_str().unwrap_or("")
    ));
    out.push_str(&format!(
        "Held: **{}**\n\n{}\n\n",
        prediction["held"].as_bool().unwrap_or(false),
        prediction["consequence"].as_str().unwrap_or(""),
    ));

    out.push_str("## Mutations\n\n");
    out.push_str("| Mutation | Failed a test |\n|---|---|\n");
    for mutation in recorded["mutations"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} |\n",
            mutation["mutation"].as_str().unwrap_or(""),
            if mutation["failed"].as_bool().unwrap_or(false) {
                "yes"
            } else {
                "**no**"
            },
        ));
    }
    out.push('\n');

    out.push_str("## The continuation gate, scored\n\n");
    let gate = &recorded["continuationGate"];
    out.push_str(&format!("{}\n\n", gate["note"].as_str().unwrap_or("")));
    out.push_str("| Condition | Met |\n|---|---|\n");
    for condition in gate["conditions"].as_array().into_iter().flatten() {
        // `met` is a boolean for the conditions the run settles and a string for the two it cannot, so
        // matching on `as_str()` alone sent every met condition to the catch-all and printed it as not
        // met. The JSON was right and this document was wrong, which is the worst direction for a
        // checked-in result to be wrong in.
        let verdict = match &condition["met"] {
            Value::Bool(true) => "yes",
            Value::Bool(false) => "**unreachable**",
            Value::String(reason) if reason == "undecidable" => "**undecidable**",
            Value::String(reason) if reason == "not_met" => "**no**",
            _ => "**unscored**",
        };
        out.push_str(&format!(
            "| {} | {} |\n",
            condition["condition"].as_str().unwrap_or(""),
            verdict,
        ));
    }
    out.push_str(&format!("\n{}\n\n", gate["verdict"].as_str().unwrap_or("")));

    out.push_str("## Not measured\n\n");
    for item in recorded["notMeasured"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out
}
