//! Emits the M3 results and the generated human view.
//!
//! ```bash
//! cargo run --example m3_revision
//! cargo run --example m3_revision -- --render-only
//! ```
//!
//! The second command rebuilds `RESULTS.md` from the checked-in `results.json`
//! without running a scenario. This runner executes no scenario itself: it
//! records which pre-registered stories the suites cover, which they do not, and
//! what was not measured. Nothing is scored here that the protocol did not fix in
//! advance.

use std::path::PathBuf;

use serde_json::{Value, json};

const EXPERIMENTS: &str = "experiments/incremental";
const RESULTS_SCHEMA: &str = "world-impact-results/v1";

/// A pre-registered story and what this build actually demonstrates.
struct Story {
    id: &'static str,
    label: &'static str,
    state: &'static str,
    detail: &'static str,
}

const STORIES: &[Story] = &[
    Story {
        id: "M3-01",
        label: "identical query on an identical snapshot",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-02",
        label: "change to an object nothing consumed",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-03",
        label: "a direct dependency changed",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-04",
        label: "a chain of three derivations",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-05",
        label: "input changed, output identical",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-06",
        label: "diamond with two affected parents",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-07",
        label: "a presentation-only projection",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-08",
        label: "same content, assurance turned stale",
        state: "partial",
        detail: "assurance references are carried, not re-simulated",
    },
    Story {
        id: "M3-09",
        label: "current authority withdrawn",
        state: "partial",
        detail: "authority is reported per finding, no real revocation",
    },
    Story {
        id: "M3-10",
        label: "evaluator or comparator version changed",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-11",
        label: "conditional read switching from A to B",
        state: "not_covered",
        detail: "no conditional evaluator in the fixture",
    },
    Story {
        id: "M3-12",
        label: "a collection gains a relevant member",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-13",
        label: "an observed absence becomes a value",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-14",
        label: "a read entry is deleted",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-15",
        label: "an observation expires on explicit time",
        state: "not_covered",
        detail: "no time-based evaluator, no documented trigger",
    },
    Story {
        id: "M3-16",
        label: "a value returns to an earlier one",
        state: "covered",
        detail: "tests/incremental_revision.rs",
    },
    Story {
        id: "M3-17",
        label: "support replaced, wording identical",
        state: "partial",
        detail: "declared support honoured, same-wording swap not distinguished",
    },
    Story {
        id: "M3-18",
        label: "a new contradiction in a collection",
        state: "not_covered",
        detail: "an obligation only, no arbitration implemented",
    },
    Story {
        id: "M3-19",
        label: "partial or opaque declared reads",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-20",
        label: "executable cycle versus narrative cycle",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-21",
        label: "a concurrent change before publication",
        state: "covered",
        detail: "tests/m3_confirmation.rs",
    },
    Story {
        id: "M3-22",
        label: "crash then reopen and retry",
        state: "not_covered",
        detail: "the engine holds no durable store of its own yet",
    },
    Story {
        id: "M3-23",
        label: "export, import, local change",
        state: "partial",
        detail: "the M2 path is exercised, the M3 pass over an imported snapshot is not",
    },
    Story {
        id: "M3-24",
        label: "a second domain through an adapter",
        state: "not_covered",
        detail: "the core has not frozen",
    },
];

fn counted_lines(paths: &[&str]) -> usize {
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.lines().count())
        .sum()
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

    let recorded = build_results();
    std::fs::create_dir_all(results_path.parent().expect("results have a parent"))?;
    std::fs::write(
        &results_path,
        format!("{}\n", serde_json::to_string_pretty(&recorded)?),
    )?;
    std::fs::write(&human_path, render_human(&recorded))?;

    let covered = recorded["coverage"]["covered"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let partial = recorded["coverage"]["partial"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let missing = recorded["coverage"]["notCovered"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    println!("{covered} covered, {partial} partial, {missing} not covered of 24 stories");
    println!("wrote {}", results_path.display());
    println!("wrote {}", human_path.display());
    Ok(())
}

fn build_results() -> Value {
    let count = |state: &str| {
        STORIES
            .iter()
            .filter(|story| story.state == state)
            .map(|story| json!({ "id": story.id, "label": story.label, "detail": story.detail }))
            .collect::<Vec<_>>()
    };
    json!({
        "schema": RESULTS_SCHEMA,
        "generatedBy": "cargo run --example m3_revision",
        "protocol": "docs/M3-PROTOCOL.md",
        "decisionRecord": "docs/ADR-002-revisable-work.md",
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
            "debugAssertions": cfg!(debug_assertions),
            "note": "this runner executes no scenario; coverage is read from the suites",
        },
        "fixture": {
            "kind": "closed arithmetic, invented for tests",
            "notARealScenario": "not Sarah, not IntentLane, not a real budget, not a user decision",
            "values": {"limit": 100, "cost_a": 80, "cost_b": 110, "label": "draft"},
        },
        "hypotheses": {
            "H3-Correction": "tested under a stated closed profile; the oracle is a fixed-point recompute that receives no record",
            "H3-Selectivite": "observed: evaluation is avoided where consumed facets hold, and the avoided count is reported",
            "H3-Transfert": "not measured: the core has not frozen and no second adapter exists",
            "H3-Utilite": "not measured: this requires a consented observation, and automated tests do not establish it",
        },
        "coverage": {
            "total": STORIES.len(),
            "covered": count("covered"),
            "partial": count("partial"),
            "notCovered": count("not_covered"),
            "stories": STORIES.iter().map(|story| json!({
                "id": story.id,
                "label": story.label,
                "state": story.state,
                "detail": story.detail,
            })).collect::<Vec<_>>(),
        },
        "model": {
            "note": "five separate dimensions, never collapsed into one validity flag",
            "dimensions": ["history", "currency", "business_verdict", "authority", "coverage"],
            "dispositions": [
                "unchanged_in_scope", "reused_after_check", "recomputed_same", "recomputed_changed",
                "needs_evidence", "needs_human_review", "blocked", "unsupported", "unknown",
            ],
            "profileRule": "an evaluator may claim a weaker profile than granted, never a stronger one; a recompute never promotes a record's coverage",
            "interface": ["why(node)", "why_reused(node)"],
        },
        "cost": {
            "note": "M1 and M2 costs are historical and are not summed with M3",
            "production": {
                "files": ["src/impact/model.rs", "src/impact/engine.rs", "src/impact/mod.rs"],
                "lines": counted_lines(&["src/impact/model.rs", "src/impact/engine.rs", "src/impact/mod.rs"]),
                "delta": "git diff --numstat bc29a22..HEAD -- src/",
            },
            "test": {
                "files": ["tests/incremental_revision.rs", "tests/m3_confirmation.rs", "tests/support/impact_fixture.rs"],
                "lines": counted_lines(&["tests/incremental_revision.rs", "tests/m3_confirmation.rs", "tests/support/impact_fixture.rs"]),
                "delta": "git diff --numstat bc29a22..HEAD -- tests/",
            },
            "interpretation": "a line count is not human time, and a hand-designed suite is not a sample of a population",
        },
        "notTested": [
            "H3-Transfert: no second domain, so no transfer cost and no claim of a useful abstraction",
            "H3-Utilite: no consented observation, so human utility is not measured",
            "no R3, A3, B3 comparison: the equal-information comparison in the mandate is not run",
            "no IntentLane path: not inspected, so no capability is claimed there",
            "no Kollio or Sarah scenario: the core has not frozen",
            "no durable store of the engine's own, so crash and reopen is not covered",
            "no time-triggered expiry, no conditional branch switching, no contradiction arbitration",
        ],
        "continuation": {
            "note": "recorded per the mandate, and it is a recommendation rather than a decision",
            "statement": "capability demonstrated for C on a closed profile with an invented fixture; differential value not measured",
            "towardM4": "not justified yet: the core has not frozen and a second domain has not been attempted",
        },
    })
}

fn render_human(recorded: &Value) -> String {
    let mut out = String::new();
    out.push_str("# M3 incremental revision results\n\n");
    out.push_str(&format!(
        "Generated by `{}`. Rebuild this file without running a scenario with \
`cargo run --example m3_revision -- --render-only`.\n\nProtocol: `{}`. Decision record: `{}`.\n\n",
        recorded["generatedBy"].as_str().unwrap_or("unknown"),
        recorded["protocol"].as_str().unwrap_or("unknown"),
        recorded["decisionRecord"].as_str().unwrap_or("unknown"),
    ));

    let coverage = &recorded["coverage"];
    let count = |key: &str| coverage[key].as_array().map(Vec::len).unwrap_or(0);
    out.push_str(&format!(
        "## Coverage\n\n{} of {} pre-registered stories are covered, {} partial, {} not covered. A story that is not \
covered is listed, never dropped.\n\n| id | story | state | detail |\n|---|---|---|---|\n",
        count("covered"),
        coverage["total"],
        count("partial"),
        count("notCovered"),
    ));
    for story in coverage["stories"].as_array().cloned().unwrap_or_default() {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            story["id"],
            story["label"].as_str().unwrap_or(""),
            story["state"].as_str().unwrap_or(""),
            story["detail"].as_str().unwrap_or("")
        ));
    }
    out.push('\n');

    out.push_str("## Hypotheses\n\n| hypothesis | state |\n|---|---|\n");
    for (id, state) in recorded["hypotheses"]
        .as_object()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!("| {} | {} |\n", id, state.as_str().unwrap_or("")));
    }
    out.push('\n');

    out.push_str("## The model\n\n");
    out.push_str(&format!(
        "{}\n\nDimensions kept separate: {}.\n\nDispositions: {}.\n\n{}\n\nInterface: {}.\n\n",
        recorded["model"]["note"].as_str().unwrap_or(""),
        join(&recorded["model"]["dimensions"]),
        join(&recorded["model"]["dispositions"]),
        recorded["model"]["profileRule"].as_str().unwrap_or(""),
        join(&recorded["model"]["interface"]),
    ));

    out.push_str("## Cost\n\n");
    let cost = &recorded["cost"];
    out.push_str(&format!("{}\n\n", cost["note"].as_str().unwrap_or("")));
    out.push_str(&format!(
        "Production: {} lines across {}.\n\nTest evidence: {} lines across {}.\n\nReproduce with:\n\n```bash\n{}\n{}\n```\n\n{}\n\n",
        cost["production"]["lines"],
        join(&cost["production"]["files"]),
        cost["test"]["lines"],
        join(&cost["test"]["files"]),
        cost["production"]["delta"].as_str().unwrap_or(""),
        cost["test"]["delta"].as_str().unwrap_or(""),
        cost["interpretation"].as_str().unwrap_or(""),
    ));

    out.push_str("## Not tested\n\n");
    for item in recorded["notTested"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out.push('\n');

    out.push_str("## Continuation\n\n");
    out.push_str(&format!(
        "{}\n\n{}\n\n{}\n",
        recorded["continuation"]["note"].as_str().unwrap_or(""),
        recorded["continuation"]["statement"].as_str().unwrap_or(""),
        recorded["continuation"]["towardM4"].as_str().unwrap_or(""),
    ));
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
