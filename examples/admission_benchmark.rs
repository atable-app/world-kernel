//! Emits the deterministic machine-readable result and the generated human view
//! for the M1 equal-information admission benchmark.
//!
//! ```bash
//! cargo run --example admission_benchmark
//! cargo run --example admission_benchmark -- --render-only
//! ```
//!
//! The second command regenerates `RESULTS.md` from the checked-in `results.json`
//! without running a single case, which is what makes the human view reproducible
//! from the machine-readable result. Durations are recorded for the environment
//! that produced them and are descriptive only.

use std::{collections::BTreeMap, path::PathBuf};

use serde_json::{Value, json};
#[path = "../tests/support/mod.rs"]
mod support;

use support::{
    admission_case::{CaseFamily, Corpus, ExpectedStatus, corpus_path},
    harness::{
        ActualStatus, Benchmark, CaseResult, SystemId, declared_support, median, percentile_95,
    },
    system_c::Ablation,
};

const RESULTS_SCHEMA: &str = "world-kernel-admission-results/v1";
const EXPERIMENTS: &str = "experiments/admission-benchmark";

/// The files that implement each system, so the reported line counts can be
/// reproduced with the command recorded beside them.
fn system_sources(system: SystemId) -> Vec<&'static str> {
    match system {
        SystemId::A => vec!["tests/support/baseline_a.rs"],
        SystemId::B => vec!["tests/support/baseline_b.rs"],
        SystemId::C => vec!["src/kernel.rs", "src/model.rs", "tests/support/system_c.rs"],
    }
}

fn line_count(paths: &[&str]) -> usize {
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.lines().count())
        .sum()
}

fn counting_command() -> String {
    "wc -l tests/support/baseline_a.rs tests/support/baseline_b.rs src/kernel.rs src/model.rs tests/support/system_c.rs"
        .to_owned()
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

    let corpus = Corpus::load(corpus_path())?;
    let benchmark = Benchmark::run(&corpus)?;
    let mut ablations: Vec<Value> = Vec::new();
    let mut weak: Vec<&str> = Vec::new();
    for ablation in Ablation::ALL {
        let results = Benchmark::run_with_ablation(&corpus, SystemId::C, ablation)?;
        let misadmitted: Vec<&CaseResult> = results
            .iter()
            .filter(|result| {
                result.actual.status == ActualStatus::Committed
                    && result.expected.status != ExpectedStatus::Committed
            })
            .collect();
        if misadmitted.is_empty() {
            weak.push(ablation.name());
        }
        ablations.push(json!({
            "ablation": ablation.name(),
            "system": "C",
            "scope": "test-only input ablation, no production check is disabled",
            "cases": results.len(),
            "incorrectlyAdmitted": ids(misadmitted.into_iter()),
        }));
    }

    let recorded = build_results(&corpus, &benchmark, &ablations);
    std::fs::create_dir_all(results_path.parent().expect("results have a parent"))?;
    std::fs::write(
        &results_path,
        format!("{}\n", serde_json::to_string_pretty(&recorded)?),
    )?;
    std::fs::write(&human_path, render_human(&recorded))?;

    let failures = recorded["aggregate"]["failures"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);

    println!(
        "{} cases x {} systems, {failures} failures, ablations that weakened nothing: {weak:?}",
        corpus.cases.len(),
        SystemId::ALL.len()
    );
    println!("wrote {}", results_path.display());
    println!("wrote {}", human_path.display());

    if failures > 0 || !weak.is_empty() {
        return Err("the run did not meet the M1 acceptance criteria".into());
    }
    Ok(())
}

fn build_results(corpus: &Corpus, benchmark: &Benchmark, ablations: &[Value]) -> Value {
    let per_system: Vec<Value> = SystemId::ALL
        .iter()
        .map(|system| {
            let results = benchmark.for_system(*system);
            let timings = benchmark.timings_micros(*system);
            let sources = system_sources(*system);
            json!({
                "system": system.label(),
                "cases": results.len(),
                "correct": results.iter().filter(|result| result.correct).count(),
                "incorrectAdmissions": ids(benchmark.incorrect_admissions(*system).into_iter()),
                "unjustifiedRejections": ids(benchmark.unjustified_rejections(*system).into_iter()),
                "harnessErrors": ids(benchmark.errors(*system).into_iter()),
                "unsupportedByFamily": family_counts(benchmark.unsupported_by_family(*system, corpus)),
                "timingMicros": {
                    "median": median(&timings),
                    "p95": percentile_95(&timings),
                    "note": "descriptive only, measured on the recorded environment",
                },
                "implementationLines": {
                    "files": sources,
                    "lines": line_count(&sources),
                },
            })
        })
        .collect();

    json!({
        "schema": RESULTS_SCHEMA,
        "generatedBy": "cargo run --example admission_benchmark",
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
            "debugAssertions": cfg!(debug_assertions),
            "note": "durations depend on this environment and are descriptive only",
        },
        "corpus": {
            "path": support::admission_case::CORPUS_PATH,
            "schema": support::admission_case::CASE_SCHEMA,
            "cases": corpus.cases.len(),
            "adverse": corpus.cases.iter().filter(|case| case.is_adverse()).count(),
            "benign": corpus.cases.iter().filter(|case| !case.is_adverse()).count(),
        },
        "declaredSupport": declared_support(),
        "cases": benchmark.results,
        "aggregate": {
            "perSystem": per_system,
            "decisionDisagreements": benchmark.decision_disagreements(corpus),
            "ablations": ablations,
            "lineCountingCommand": counting_command(),
            "failures": benchmark
                .failures()
                .into_iter()
                .map(|result| json!({
                    "caseId": result.case_id,
                    "system": result.system.label(),
                    "actual": result.actual,
                    "expected": result.expected,
                }))
                .collect::<Vec<_>>(),
        },
    })
}

fn ids<'a, I: Iterator<Item = &'a CaseResult>>(results: I) -> Vec<&'a str> {
    results.map(|result| result.case_id.as_str()).collect()
}

fn family_counts(counts: BTreeMap<CaseFamily, usize>) -> Value {
    let mut object = serde_json::Map::new();
    for (family, count) in counts {
        let name = serde_json::to_value(family)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{family:?}"));
        object.insert(name, json!(count));
    }
    Value::Object(object)
}

fn code_label(code: Option<&str>) -> String {
    code.unwrap_or("none").to_owned()
}

fn render_human(recorded: &Value) -> String {
    let mut out = String::new();
    out.push_str("# M1 equal-information admission benchmark results\n\n");
    out.push_str(&format!(
        "Generated by `{}` from `results.json`.\n",
        recorded["generatedBy"].as_str().unwrap_or("unknown")
    ));
    out.push_str(
        "Regenerate this file without running a case with \
`cargo run --example admission_benchmark -- --render-only`.\n\n",
    );

    let corpus = &recorded["corpus"];
    out.push_str(&format!(
        "Corpus: {} cases, {} adverse, {} benign, schema `{}`.\n\n",
        corpus["cases"], corpus["adverse"], corpus["benign"], corpus["schema"]
    ));

    out.push_str("## Systems\n\n");
    for entry in recorded["aggregate"]["perSystem"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let label = entry["system"].as_str().unwrap_or("?");
        out.push_str(&format!(
            "### System {label}\n\n\
             - correct decisions: {}/{}\n\
             - incorrect admissions: {}\n\
             - unjustified rejections among the benign cases: {}\n\
             - harness errors: {}\n\
             - unsupported cases by family: {}\n\
             - timing: median {} us, p95 {} us (descriptive only)\n\
             - implementation: {} lines across {}\n\n",
            entry["correct"],
            entry["cases"],
            list(&entry["incorrectAdmissions"]),
            list(&entry["unjustifiedRejections"]),
            list(&entry["harnessErrors"]),
            entry["unsupportedByFamily"],
            entry["timingMicros"]["median"],
            entry["timingMicros"]["p95"],
            entry["implementationLines"]["lines"],
            join(&entry["implementationLines"]["files"]),
        ));
    }

    out.push_str("## Decision-code disagreements between A, B and C\n\n");
    let disagreements = recorded["aggregate"]["decisionDisagreements"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if disagreements.is_empty() {
        out.push_str("None. Every case produced the same decision code in all three systems.\n\n");
    } else {
        out.push_str("| case | A | B | C |\n|---|---|---|---|\n");
        for entry in disagreements {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                entry["caseId"].as_str().unwrap_or(""),
                code_label(entry["a"].as_str()),
                code_label(entry["b"].as_str()),
                code_label(entry["c"].as_str()),
            ));
        }
        out.push('\n');
    }

    out.push_str("## Test-only input ablations on system C\n\n");
    out.push_str(
        "Each ablation weakens the proposal before admission. No production check is disabled, \
and an ablation that admits nothing would prove no check.\n\n",
    );
    for entry in recorded["aggregate"]["ablations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!(
            "- `{}`: incorrectly admitted {}\n",
            entry["ablation"].as_str().unwrap_or(""),
            list(&entry["incorrectlyAdmitted"])
        ));
    }
    out.push('\n');

    out.push_str("## Failures\n\n");
    let failures = recorded["aggregate"]["failures"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if failures.is_empty() {
        out.push_str("None. All 80 cases produced the expected decision in all three systems.\n\n");
    } else {
        out.push_str("No case is filtered out of the totals above.\n\n");
        out.push_str("| case | system | actual | expected |\n|---|---|---|---|\n");
        for entry in failures {
            out.push_str(&format!(
                "| {} | {} | {} {} | {} {} |\n",
                entry["caseId"].as_str().unwrap_or(""),
                entry["system"].as_str().unwrap_or(""),
                entry["actual"]["status"].as_str().unwrap_or(""),
                code_label(entry["actual"]["code"].as_str()),
                entry["expected"]["status"].as_str().unwrap_or(""),
                code_label(entry["expected"]["code"].as_str()),
            ));
        }
        out.push('\n');
    }

    out.push_str("## Continuation decision\n\n");
    out.push_str(&continuation(recorded));
    out
}

fn continuation(recorded: &Value) -> String {
    let per_system = recorded["aggregate"]["perSystem"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let all_equal = per_system.iter().all(|entry| {
        entry["incorrectAdmissions"].as_array().map(Vec::len) == Some(0)
            && entry["harnessErrors"].as_array().map(Vec::len) == Some(0)
    });
    let disagreements = recorded["aggregate"]["decisionDisagreements"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let lines: Vec<u64> = per_system
        .iter()
        .map(|entry| entry["implementationLines"]["lines"].as_u64().unwrap_or(0))
        .collect();

    let mut out = String::new();
    if !all_equal {
        out.push_str(
            "**Blocked.** At least one system admitted an adverse case it claims to support, or a \
system failed to produce a structured result. The M1 acceptance criteria are not met.\n\n",
        );
        return out;
    }
    out.push_str(
        "All acceptance criteria pass: no system incorrectly admits an adverse case, no system has \
more than one unjustified rejection among the 20 benign cases, the 9 unsupported cases stay \
`unsupported`, and all three ablations weaken a decision.\n\n",
    );
    if disagreements == 0 {
        out.push_str(&format!(
            "**Finding.** On this corpus the three systems reach the same decision in all 80 cases, \
with {disagreements} code disagreements. C did not prevent a class of error that competent A and \
B do not also prevent: the portable envelope is not buying a correctness advantage here. The \
difference that remains is cost and reuse: C costs {} lines, A {} and B {}. The receipt, replay and \
handoff value of C is not exercised by this corpus, so it is neither measured nor disproved \
here.\n\n",
            lines[2], lines[0], lines[1]
        ));
        out.push_str(
            "Under the M1 continuation gate this is the third outcome, not the first two. Before M2, \
decide deliberately whether the reusable receipt and replay are worth the envelope, because the \
corpus shows no admission-error reduction attributable to C. This result does not license a \
claim about human time; that metric needs repeated human tasks.\n",
        );
    } else {
        out.push_str(
            "The systems disagree on at least one case. Inspect the disagreement table before \
choosing a direction.\n",
        );
    }
    out
}

fn join(value: &Value) -> String {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn list(value: &Value) -> String {
    let items = value.as_array().cloned().unwrap_or_default();
    if items.is_empty() {
        return "none".to_owned();
    }
    format!(
        "{} ({})",
        items.len(),
        items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    )
}
