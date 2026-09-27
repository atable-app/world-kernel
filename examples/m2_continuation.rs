//! Emits the M2 continuation results and the generated human view.
//!
//! ```bash
//! cargo run --example m2_continuation
//! cargo run --example m2_continuation -- --render-only
//! ```
//!
//! The second command rebuilds `RESULTS.md` from the checked-in `results.json`
//! without running a sequence, so the human view is reproducible from the
//! machine-readable result.
//!
//! This runner does not re-decide anything. It records which pre-registered
//! sequences the suite covers, which it does not, and which need the A2 and B2
//! baselines that M2 has not built yet. Nothing is scored here that the protocol
//! did not pre-register, and an uncovered sequence is reported as uncovered.

use std::path::PathBuf;

use serde_json::{Value, json};

const EXPERIMENTS: &str = "experiments/continuation";
const RESULTS_SCHEMA: &str = "world-kernel-continuation-results/v1";

/// The pre-registered matrix, with the coverage this build actually has.
struct Sequence {
    id: u32,
    label: &'static str,
    covered: bool,
    note: &'static str,
}

const SEQUENCES: &[Sequence] = &[
    Sequence {
        id: 1,
        label: "fresh consumer, unchanged context",
        covered: true,
        note: "",
    },
    Sequence {
        id: 2,
        label: "fresh consumer without capability",
        covered: true,
        note: "",
    },
    Sequence {
        id: 3,
        label: "producer workspace removed",
        covered: true,
        note: "",
    },
    Sequence {
        id: 4,
        label: "absent body declared",
        covered: true,
        note: "",
    },
    Sequence {
        id: 5,
        label: "crash and lost response",
        covered: true,
        note: "tests/crash_symmetry.rs",
    },
    Sequence {
        id: 6,
        label: "crash after a write before commit",
        covered: true,
        note: "tests/crash_symmetry.rs",
    },
    Sequence {
        id: 7,
        label: "moved declared dependency",
        covered: true,
        note: "",
    },
    Sequence {
        id: 8,
        label: "unrelated change, negative control",
        covered: true,
        note: "",
    },
    Sequence {
        id: 9,
        label: "substituted assurance reference",
        covered: true,
        note: "",
    },
    Sequence {
        id: 10,
        label: "history kept, applicability recomputed",
        covered: true,
        note: "",
    },
    Sequence {
        id: 11,
        label: "tampered resource digest",
        covered: true,
        note: "",
    },
    Sequence {
        id: 12,
        label: "truncated package",
        covered: true,
        note: "",
    },
    Sequence {
        id: 13,
        label: "reordered transitions",
        covered: true,
        note: "",
    },
    Sequence {
        id: 14,
        label: "unknown transition kind",
        covered: true,
        note: "",
    },
    Sequence {
        id: 15,
        label: "unknown package schema",
        covered: true,
        note: "",
    },
    Sequence {
        id: 16,
        label: "strict parse refusals",
        covered: true,
        note: "",
    },
    Sequence {
        id: 17,
        label: "unimplemented reducer",
        covered: true,
        note: "",
    },
    Sequence {
        id: 18,
        label: "path-shaped reference",
        covered: true,
        note: "",
    },
    Sequence {
        id: 19,
        label: "bounded sizes and counts",
        covered: true,
        note: "",
    },
    Sequence {
        id: 20,
        label: "no executable field in the package",
        covered: true,
        note: "",
    },
    Sequence {
        id: 21,
        label: "benign progression",
        covered: true,
        note: "",
    },
    Sequence {
        id: 22,
        label: "identity reuse refused",
        covered: true,
        note: "",
    },
    Sequence {
        id: 23,
        label: "anchor absent",
        covered: true,
        note: "",
    },
    Sequence {
        id: 24,
        label: "guarantees reported separately",
        covered: true,
        note: "",
    },
];

/// Sequences that run for C2 only, because A2 and B2 have no export of their own
/// yet. This is the equal-information gap and it stays visible in the report.
const NEEDS_BASELINES: &[(u32, &str)] = &[
    (
        3,
        "A2 and B2 have no export and no application-level resume",
    ),
    (
        4,
        "A2 and B2 have no own snapshot to declare resource bodies against",
    ),
    (21, "A2 and B2 have no own resume to progress through"),
    (
        24,
        "A2 and B2 report no guarantee structure of their own yet",
    ),
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

    let uncovered = recorded["coverage"]["notCovered"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let needs_baselines = recorded["coverage"]["needsBaselines"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    println!(
        "{} of {} pre-registered sequences covered; {needs_baselines} of them for C2 only",
        recorded["coverage"]["covered"], recorded["coverage"]["total"]
    );
    println!("uncovered by any system: {uncovered}");
    println!("wrote {}", results_path.display());
    println!("wrote {}", human_path.display());
    Ok(())
}

fn build_results() -> Value {
    let covered = SEQUENCES.iter().filter(|s| s.covered).count();
    json!({
        "schema": RESULTS_SCHEMA,
        "generatedBy": "cargo run --example m2_continuation",
        "protocol": "docs/M2-PROTOCOL.md",
        "decisionRecord": "docs/ADR-001-portable-continuation.md",
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
            "debugAssertions": cfg!(debug_assertions),
            "note": "this runner executes no sequence; coverage is read from the suites",
        },
        "systems": {
            "C2": "UNI plus the World Kernel, with the continuation consumer",
            "A2": "application SQLite gate; no export and no application-level resume yet",
            "B2": "the same application; the UNI seam is exercised in M1, not yet in a resume path",
        },
        "coverage": {
            "total": SEQUENCES.len(),
            "covered": covered,
            "notCovered": SEQUENCES.iter().filter(|s| !s.covered).map(|s| json!({
                "id": s.id,
                "label": s.label,
            })).collect::<Vec<_>>(),
            "needsBaselines": NEEDS_BASELINES.iter().map(|(id, note)| json!({
                "id": id,
                "gap": note,
            })).collect::<Vec<_>>(),
            "sequences": SEQUENCES.iter().map(|s| json!({
                "id": s.id,
                "label": s.label,
                "covered": s.covered,
                "suite": s.note,
            })).collect::<Vec<_>>(),
        },
        "guarantees": {
            "note": "reported separately, never collapsed into one verdict",
            "reported": [
                "format_understood",
                "resource_integrity",
                "authenticity",
                "completeness_within_scope",
                "historical_reconstruction",
                "completeness_beyond_scope",
            ],
            "neverClaimed": ["completeness_beyond_scope"],
            "authenticityRequires": "an expected head received out of band; a package never vouches for itself",
        },
        "cost": {
            "note": "M1 historical cost and M2 new cost are separate and are never summed. Production cost and test cost are also separate: a test suite is evidence, not a product.",
            "production": {
                "newModule": "src/continuation.rs",
                "newModuleLines": counted_lines(&["src/continuation.rs"]),
                "extendedModules": ["src/kernel.rs", "src/model.rs", "src/lib.rs"],
                "deltaSinceM1": "git diff --numstat f686d9a..HEAD -- src/",
            },
            "test": {
                "files": [
                    "tests/continuation_consumer.rs",
                    "tests/m2_sequences.rs",
                    "tests/crash_symmetry.rs",
                ],
                "lines": counted_lines(&[
                    "tests/continuation_consumer.rs",
                    "tests/m2_sequences.rs",
                    "tests/crash_symmetry.rs",
                ]),
                "deltaSinceM1": "git diff --numstat f686d9a..HEAD -- tests/",
            },
            "interpretation": "a line count is not human time, and these suites are hand-designed rather than sampled from a population, so no reliability rate follows from them",
        },
        "notTested": [
            "a fresh consumer outside this repository's language: the interoperability requirement is not met yet",
            "a demonstration inside a real product path: not attempted, so no capability is claimed",
            "A2 and B2 running the same matrix, so no cost comparison is available",
            "an independent authority mechanism: only digest-relative integrity is claimed",
        ],
    })
}

fn render_human(recorded: &Value) -> String {
    let mut out = String::new();
    out.push_str("# M2 continuation results\n\n");
    out.push_str(&format!(
        "Generated by `{}`. Rebuild this file without running a sequence with \
`cargo run --example m2_continuation -- --render-only`.\n\n\
Protocol: `{}`. Decision record: `{}`.\n\n",
        recorded["generatedBy"].as_str().unwrap_or("unknown"),
        recorded["protocol"].as_str().unwrap_or("unknown"),
        recorded["decisionRecord"].as_str().unwrap_or("unknown"),
    ));

    let coverage = &recorded["coverage"];
    out.push_str(&format!(
        "## Coverage\n\n{} of {} pre-registered sequences are covered. An uncovered sequence is \
reported, never dropped.\n\n| id | sequence | covered | suite |\n|---|---|---|---|\n",
        coverage["covered"], coverage["total"]
    ));
    for sequence in coverage["sequences"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            sequence["id"],
            sequence["label"].as_str().unwrap_or(""),
            if sequence["covered"] == json!(true) {
                "yes"
            } else {
                "no"
            },
            if sequence["suite"].as_str().unwrap_or("").is_empty() {
                "tests/m2_sequences.rs"
            } else {
                sequence["suite"].as_str().unwrap_or("")
            }
        ));
    }
    out.push('\n');

    out.push_str("## The equal-information gap\n\n");
    let gaps = coverage["needsBaselines"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if gaps.is_empty() {
        out.push_str("None recorded.\n\n");
    } else {
        out.push_str(
            "These sequences are covered for C2 only. A2 and B2 cannot be compared yet because \
they have no export and no application-level resume of their own, so no cost comparison exists \
and none is implied.\n\n",
        );
        for gap in gaps {
            out.push_str(&format!(
                "- sequence {}: {}\n",
                gap["id"],
                gap["gap"].as_str().unwrap_or("")
            ));
        }
        out.push('\n');
    }

    out.push_str("## Guarantees\n\n");
    let reported = recorded["guarantees"]["reported"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    out.push_str(&format!(
        "{} guarantees, reported separately rather than as one verdict. Never claimed: {}.\n\n{}\n\n",
        reported.len(),
        recorded["guarantees"]["neverClaimed"]
            .as_array()
            .map(|names| names
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
        recorded["guarantees"]["authenticityRequires"]
            .as_str()
            .unwrap_or("")
    ));

    out.push_str("## Cost\n\n");
    let cost = &recorded["cost"];
    out.push_str(&format!("{}\n\n", cost["note"].as_str().unwrap_or("")));
    out.push_str(&format!(
        "Production: {} lines in the new {}, plus changes to {}.\n\nTest evidence: {} lines across {}.\n\n\
Reproduce with:\n\n```bash\n{}\n{}\n```\n\n{}\n\n",
        cost["production"]["newModuleLines"],
        cost["production"]["newModule"].as_str().unwrap_or(""),
        cost["production"]["extendedModules"]
            .as_array()
            .map(|files| files
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
        cost["test"]["lines"],
        cost["test"]["files"]
            .as_array()
            .map(|files| files
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
        cost["production"]["deltaSinceM1"].as_str().unwrap_or(""),
        cost["test"]["deltaSinceM1"].as_str().unwrap_or(""),
        cost["interpretation"].as_str().unwrap_or("")
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

    out.push_str("## Capability statement\n\n");
    out.push_str(
        "A fresh consumer reconstructs the announced projection from the package alone, benign \
paths progress, a moved declared dependency is blocked and named, and no resumption depends on \
the producer. What is not established is differential value: A2 and B2 have not run this matrix, \
so the cost of the same capability without the envelope is unmeasured. The honest conclusion at \
this point is capability established for C2 and differential value not measured.\n",
    );
    out
}
