//! Emits the M4 tranche-1 results and the generated human view.
//!
//! ```bash
//! cargo run --example m4_branch
//! cargo run --example m4_branch -- --render-only
//! ```
//!
//! The second command rebuilds `RESULTS.md` from the checked-in `results.json` without running a
//! scenario. The stories are fixed in `docs/M4-PROTOCOL.md` before these results were retained, and this
//! runner records which of them tranche 1 decides and which it does not. Nothing is scored here that the
//! protocol did not fix in advance.

use std::path::PathBuf;

use serde_json::{Value, json};

const EXPERIMENTS: &str = "experiments/branch-convergence";
const RESULTS_SCHEMA: &str = "world-branch-convergence/v1";

/// A story from `docs/M4-PROTOCOL.md` and what tranche 1 actually demonstrates.
struct Story {
    id: &'static str,
    label: &'static str,
    state: &'static str,
    detail: &'static str,
}

const STORIES: &[Story] = &[
    Story {
        id: "M4-01",
        label: "fork then change an inherited object",
        state: "covered",
        detail: "tests/branch_convergence.rs, forking_and_changing_leaves_the_source_and_its_sibling_untouched",
    },
    Story {
        id: "M4-02",
        label: "the target advances after a branch is created",
        state: "covered",
        detail: "tests/branch_convergence.rs, a_target_that_moved_makes_the_proposal_stale_rather_than_rebased",
    },
    Story {
        id: "M4-11",
        label: "Alpha 55 + Beta 55, capacity 100",
        state: "covered",
        detail: "tests/branch_convergence.rs, a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move",
    },
    Story {
        id: "M4-16",
        label: "a favourable result under an assumed capacity of 120",
        state: "notCovered",
        detail: "tranche 1 has no assumption context; the scenario is pre-registered for tranche 2 and is not approximated here",
    },
    Story {
        id: "M4-15",
        label: "partial adoption then a second adoption",
        state: "partial",
        detail: "the receipt names the exact selection and the excluded part stays in the source; the second adoption over the same unit is not exercised",
    },
    Story {
        id: "M4-25",
        label: "fork, compare, replay, archive with zero external calls",
        state: "partial",
        detail: "archive is implemented and fork is covered; there is no replay, no comparison surface and no external-call counter in tranche 1",
    },
];

/// A story the protocol asked for that tranche 1 does not decide, so the gap is visible.
const NOT_AUTHORISED: &[&str] = &[
    "the other 26 stories of the 32-story matrix, which ADR-004 does not authorise",
    "portable export and import of branch history, conditioned on M2 and not attempted",
    "any IntentLane or Kollio path",
    "human utility, which needs a person and cannot be measured by a test count",
    "bounded generative trees, which arrive with the runner in a later tranche",
    "that a branch isolation test proves network or plugin isolation; it does not",
];

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

    let covered = count("covered");
    let partial = count("partial");
    let missing = count("notCovered");
    println!(
        "{covered} covered, {partial} partial, {missing} not covered of {} stories",
        STORIES.len()
    );
    println!("wrote {}", results_path.display());
    println!("wrote {}", human_path.display());
    Ok(())
}

fn count(state: &str) -> usize {
    STORIES.iter().filter(|s| s.state == state).count()
}

fn build_results() -> Value {
    let stories: Vec<Value> = STORIES
        .iter()
        .map(|story| {
            json!({
                "id": story.id,
                "label": story.label,
                "state": story.state,
                "detail": story.detail,
            })
        })
        .collect();

    json!({
        "schema": RESULTS_SCHEMA,
        "decision": "docs/ADR-004-the-facet-advantage-scales.md",
        "protocol": "docs/M4-PROTOCOL.md",
        "tranche": 1,
        "fixture": {
            "file": "tests/support/branch_fixture.rs",
            "nature": "invented arithmetic for tests; not Sarah, not IntentLane, not a real budget",
            "base": "capacity 100, load_a 40, load_b 40, rule load_a + load_b <= capacity",
            "oracle": "a separate program over plain integers that calls nothing in world_kernel::branch and reads no expected value",
        },
        "primaryTest": {
            "name": "a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move",
            "claim": "Alpha and Beta are each sound at 95. Their composition is 110 against a capacity of 100. The candidate is built, the rule is checked on it, the target does not move, and the refusal names the rule and the versions examined.",
            "counterweight": "a_revised_branch_composing_exactly_to_the_limit_is_admissible, because a test that blocks every adoption is not a test",
        },
        "designConsequence": "Alpha and Beta change different fields, so there is no value conflict to detect. A system that only compared values would merge them cleanly and produce a target that breaks its own rule. The gate is therefore a constraint check on the reconstructed candidate.",
        "coverage": {
            "covered": stories.iter().filter(|s| s["state"] == "covered").collect::<Vec<_>>(),
            "partial": stories.iter().filter(|s| s["state"] == "partial").collect::<Vec<_>>(),
            "notCovered": stories.iter().filter(|s| s["state"] == "notCovered").collect::<Vec<_>>(),
        },
        "stories": stories,
        "mutations": [
            {"mutation": "drop the constraint check on the composed candidate", "failedTest": "a_combination_of_two_valid_branches_is_blocked_and_the_target_does_not_move"},
            {"mutation": "treat a partial selection as a full merge", "failedTest": "a_partial_adoption_does_not_make_the_excluded_part_look_merged"},
            {"mutation": "accept a proposal whose target revision moved", "failedTest": "commit refused the stale proposal"},
            {"mutation": "guess an unknown base instead of refusing", "failedTest": "an_unknown_base_is_refused_rather_than_guessed"},
        ],
        "absentByDesign": [
            "no multi-parent merge; the base is a single known ancestor and an unknown one is diagnosed",
            "no automatic re-grounding; a moved target yields a stale proposal",
            "no automatic merge of any kind",
            "no safe_to_merge boolean, because structure, constraints, assurance, authority and currency are five questions",
            "no second impact engine; the constraint check reuses what already exists",
        ],
        "notAuthorised": NOT_AUTHORISED,
        "notMeasured": [
            "no cost per evaluation, so no claim in euros, tokens or human time",
            "no replanning cost from the conservative compare-and-swap, which is retained deliberately",
            "no equal-information comparison against a competent branching application, which the M1 and M3 protocol would both require before any claim of superiority",
            "no real consumer: the fixture is invented arithmetic",
        ],
    })
}

fn render_human(recorded: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Branch convergence: tranche 1\n\n");
    out.push_str(&format!(
        "Protocol: {} · Decision: {}\n\n",
        recorded["protocol"].as_str().unwrap_or(""),
        recorded["decision"].as_str().unwrap_or(""),
    ));
    out.push_str(&format!(
        "Design consequence: {}\n\n",
        recorded["designConsequence"].as_str().unwrap_or(""),
    ));

    out.push_str("## The primary test\n\n");
    let primary = &recorded["primaryTest"];
    out.push_str(&format!("`{}`\n\n", primary["name"].as_str().unwrap_or("")));
    out.push_str(&format!("{}\n\n", primary["claim"].as_str().unwrap_or("")));
    out.push_str(&format!(
        "Counterweight: {}\n\n",
        primary["counterweight"].as_str().unwrap_or("")
    ));

    out.push_str("## Stories\n\n");
    out.push_str("| ID | Story | State | Where |\n|---|---|---|---|\n");
    for story in recorded["stories"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} | `{}` |\n",
            story["id"].as_str().unwrap_or(""),
            story["label"].as_str().unwrap_or(""),
            story["state"].as_str().unwrap_or(""),
            story["detail"].as_str().unwrap_or(""),
        ));
    }
    out.push('\n');

    out.push_str("## Mutations that had to fail\n\n");
    out.push_str("| Mutation | Test it broke |\n|---|---|\n");
    for mutation in recorded["mutations"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | `{}` |\n",
            mutation["mutation"].as_str().unwrap_or(""),
            mutation["failedTest"].as_str().unwrap_or(""),
        ));
    }
    out.push('\n');

    out.push_str("## Absent by design\n\n");
    for item in recorded["absentByDesign"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out.push('\n');

    out.push_str("## Not authorised by ADR-004\n\n");
    for item in recorded["notAuthorised"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out.push('\n');

    out.push_str("## Not measured\n\n");
    for item in recorded["notMeasured"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", item.as_str().unwrap_or("")));
    }
    out
}
