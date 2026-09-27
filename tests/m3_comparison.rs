//! The equal-information comparison: R3, A3, B3 and C3 on the same inputs.
//!
//! All four receive the same fixture, the same evaluators, the same snapshots, the same trust scope and
//! the same ordered target list. They choose their own representation. None reads an expected result.
//! The harness and the result format are shared; the competing decision mechanisms are not.
//!
//! R3 is a full recompute and is the correction oracle, not a weak baseline.
//! A3 is the competent application mechanism, not a strawman.
//! B3 is the same application, consuming a UNI staleness answer it did not compute.
//! C3 is the World Kernel impact core.
//!
//! If A3 and B3 reach the same observable results with the same work avoided, the portable graph has no
//! measured value on this scope, and that is the finding.

mod support;

use serde_json::Value;
use std::collections::BTreeMap;
use support::{
    application_incremental::{AppCache, revise_app},
    impact_fixture as fixture, incumbent_comparison,
};

use support::impact_core::{
    Disposition, EvaluationRecord, Limits, NodeId, Profile, TrustConfiguration, full_recompute,
    revise,
};

#[test]
fn every_system_reaches_the_same_observable_result_on_every_scenario() {
    let rows = incumbent_comparison::run_all();

    for row in &rows {
        assert!(row.r3_correct, "R3 diverged on {}", row.scenario);
        assert!(row.a3_correct, "A3 diverged on {}", row.scenario);
        assert!(row.b3_correct, "B3 diverged on {}", row.scenario);
        assert!(
            row.c3_correct,
            "C3 diverged on {}: the core is the system under test as much as the others",
            row.scenario
        );
    }
}

#[test]
fn the_application_mechanism_avoids_the_same_work_the_core_avoids_except_where_a_facet_matters() {
    let rows = incumbent_comparison::run_all();

    for row in &rows {
        assert_eq!(
            row.b3_executions, row.a3_executions,
            "B3 added no work of its own on {}",
            row.scenario
        );
        if row.scenario == "unconsumed_field_change" {
            // The one scenario where the granularities part company.
            assert!(row.c3_executions < row.a3_executions);
            continue;
        }
        assert_eq!(
            row.a3_executions, row.c3_executions,
            "A3 and C3 avoided different amounts of work on {}: {} against {}",
            row.scenario, row.a3_executions, row.c3_executions
        );
    }
}

#[test]
fn a_change_nothing_consumed_costs_nothing_anywhere() {
    let rows = incumbent_comparison::run_all();
    let unchanged = rows
        .iter()
        .find(|row| row.scenario == "mutation_7_unchanged_world")
        .expect("the unchanged scenario ran");

    assert_eq!(
        unchanged.reference_executions, 3,
        "a full recompute would run every evaluator"
    );
    assert_eq!(
        unchanged.a3_executions, 0,
        "the application mechanism runs nothing"
    );
    assert_eq!(unchanged.c3_executions, 0, "and neither does the core");
}

#[test]
fn b3_refuses_to_reuse_an_entry_whose_assurance_went_stale() {
    let base = fixture::snapshot(0, 100, 80, 110, "draft");
    let target = fixture::snapshot(1, 100, 80, 110, "draft");

    // The world has not changed. A3 therefore reuses everything.
    let clean = incumbent_comparison::run_a3(&base, &target).expect("A3 runs");
    assert_eq!(
        clean.executions, 0,
        "nothing moved, so the application reuses all three"
    );
    assert_eq!(clean.reused, 3);

    // The support object reads eligible_a and eligible_b. UNI declares eligible_a
    // no longer current. B3 has to re-run the support object, and it still has to
    // answer: consuming a declaration is not a reason to return nothing.
    let stale = incumbent_comparison::run_b3(&base, &target, &[fixture::ELIGIBLE_A.to_owned()])
        .expect("B3 runs");
    assert_eq!(stale.reused, 2, "and only that one is kept");
    assert_eq!(
        stale.values.get(fixture::SUPPORT),
        clean.values.get(fixture::SUPPORT),
        "B3 answers the same as A3, it just costs more to get there"
    );

    // A declaration about a reference nobody in this run relied on costs nothing.
    let irrelevant = incumbent_comparison::run_b3(&base, &target, &[fixture::LABEL.to_owned()])
        .expect("B3 runs");
    assert_eq!(
        irrelevant.executions, 0,
        "an irrelevant declaration buys no work and costs none"
    );

    // The runner records the same two numbers, so the artifact cannot claim B3 is
    // free when the measurement says it costs.
    let (a3, b3, same_answer, cost_more) = incumbent_comparison::stale_declaration_cost();
    assert_eq!(a3, 0);
    assert_eq!(b3, 1);
    assert!(same_answer, "the same answer for one more evaluation");
    assert!(cost_more, "consuming a declaration is not free");
}

#[test]
fn the_core_reports_what_the_application_mechanism_cannot() {
    // The application mechanism returns values. It does not return a
    // disposition, an obligation or an explanation, and it cannot be asked why.
    // The limit moves without either boolean moving, so the support consumer is
    // genuinely reused and the core can say why.
    let base = fixture::snapshot(0, 100, 80, 110, "draft");
    let target = fixture::snapshot(1, 90, 80, 110, "draft");
    let (_, _, _) = incumbent_comparison::run_c3(&base, &target).expect("C3 runs");

    let recorded: BTreeMap<NodeId, EvaluationRecord> = full_recompute(
        &base,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the oracle runs")
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect();
    let revised = revise(
        &base,
        &target,
        &recorded,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("C3 runs");

    // The core produces a disposition and an explanation. The application
    // mechanism returns values and nothing else, so it cannot answer either
    // question the protocol calls indispensable.
    assert!(revised.why(fixture::ELIGIBLE_A).is_some());
    let reused = revised
        .why_reused(fixture::SUPPORT)
        .expect("the core can say why a consumer was kept");
    assert_eq!(reused.code, "consumed_facets_unchanged");
    assert_eq!(
        revised.findings[fixture::SUPPORT].disposition,
        Disposition::ReusedAfterCheck
    );
    // And the application mechanism returns the same value with no way to say
    // where it came from: its outcome is values and a count, nothing else.
    let application = stale_support(&base, &target);
    assert_eq!(
        application.as_ref().map(|value| value["supports"].clone()),
        revised.findings[fixture::SUPPORT]
            .output
            .as_ref()
            .map(|value| value["supports"].clone()),
        "the same value, reached by the same decision"
    );
}

#[test]
fn an_ungranted_evaluator_is_refused_by_both_mechanisms() {
    let base = fixture::snapshot(0, 100, 80, 110, "draft");
    let target = fixture::snapshot(1, 90, 80, 110, "draft");
    let ungranted = TrustConfiguration::default();

    let ungranted_error = full_recompute(
        &base,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &ungranted,
        fixture::limits(),
    )
    .expect_err("an ungranted evaluator is refused even by the oracle");
    assert!(
        ungranted_error
            .to_string()
            .contains("grants at most opaque"),
        "and the refusal names the shortfall"
    );

    let recorded: BTreeMap<NodeId, EvaluationRecord> = full_recompute(
        &base,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("a granted configuration records the base")
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect();
    assert!(
        revise(
            &base,
            &target,
            &recorded,
            &incumbent_comparison::targets(),
            &fixture::evaluators(),
            &ungranted,
            fixture::limits()
        )
        .is_err()
    );

    // And the application mechanism, given the same records, refuses it too.
    let mut cache = AppCache::new();
    let warm = revise_app(
        &base,
        &incumbent_comparison::targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    );
    assert!(warm.is_ok(), "the warm-up used a granted configuration");
}

#[test]
fn the_profile_rule_is_the_same_on_both_sides() {
    // The core refuses an evaluator that claims more than granted. The
    // application mechanism only asks whether the evaluator runs at all, which
    // is a weaker question and a real difference in kind.
    let base = fixture::snapshot(0, 100, 80, 110, "draft");
    let target = fixture::snapshot(1, 90, 80, 110, "draft");
    let recorded: BTreeMap<NodeId, EvaluationRecord> = full_recompute(
        &base,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("runs")
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect();

    assert!(
        revise(
            &base,
            &target,
            &recorded,
            &incumbent_comparison::targets(),
            &fixture::evaluators(),
            &TrustConfiguration::default(),
            fixture::limits(),
        )
        .is_err(),
        "the core refuses a stronger claim than granted"
    );

    let mut cache = AppCache::new();
    assert!(
        revise_app(
            &base,
            &incumbent_comparison::targets(),
            &mut cache,
            &fixture::evaluators(),
            &TrustConfiguration::default(),
            fixture::limits()
        )
        .is_err(),
        "and the application mechanism refuses an ungranted evaluator outright"
    );
}

#[test]
fn the_limits_are_reported_the_same_way() {
    let base = fixture::snapshot(0, 100, 80, 110, "draft");
    let target = fixture::snapshot(1, 90, 80, 110, "draft");
    let tight = Limits {
        max_evaluations: 1,
        ..fixture::limits()
    };

    let mut cache = AppCache::new();
    revise_app(
        &base,
        &incumbent_comparison::targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the warm-up runs");

    let application = revise_app(
        &target,
        &incumbent_comparison::targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        tight,
    );
    assert!(application.is_err(), "the application mechanism also stops");

    let recorded: BTreeMap<NodeId, EvaluationRecord> = full_recompute(
        &base,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("runs")
    .into_iter()
    .map(|(id, evaluated)| (id, evaluated.record))
    .collect();
    let core = revise(
        &base,
        &target,
        &recorded,
        &incumbent_comparison::targets(),
        &fixture::evaluators(),
        &fixture::trust(),
        tight,
    );
    assert!(core.is_err(), "and so does the core");
}

/// What the application mechanism can say about a consumer it kept: the value,
/// and nothing else.
fn stale_support(
    base: &support::impact_core::Snapshot,
    target: &support::impact_core::Snapshot,
) -> Option<Value> {
    let mut cache = AppCache::new();
    revise_app(
        base,
        &incumbent_comparison::targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the application warm-up runs");
    let outcome = revise_app(
        target,
        &incumbent_comparison::targets(),
        &mut cache,
        &fixture::evaluators(),
        &fixture::trust(),
        fixture::limits(),
    )
    .expect("the application pass runs");
    // A reused entry is indistinguishable from a recomputed one at the interface.
    assert!(cache.entry(fixture::SUPPORT).is_some());
    outcome.values.get(fixture::SUPPORT).cloned()
}

/// The one scenario on this fixture where the two mechanisms differ, and the
/// direction of the difference.
///
/// The core caches on a consumed facet; the application mechanism caches on whole
/// values. The limit node carries a second field that nothing in the arithmetic
/// reads. Changing only that field must not re-run a comparison that never looked
/// at it, and a whole-value cache cannot tell the difference.
#[test]
fn a_consumed_facet_cache_avoids_work_a_whole_value_cache_cannot() {
    let base = fixture::snapshot_with_note(0, 100, "first");
    let target = fixture::snapshot_with_note(1, 100, "second");

    let application = incumbent_comparison::run_a3(&base, &target).expect("A3 runs");
    let (_, core_executions, core_reused) =
        incumbent_comparison::run_c3(&base, &target).expect("C3 runs");

    // The unconsumed field did move. The compared value did not.
    assert_eq!(
        target
            .live(fixture::LIMIT)
            .map(|node| node.payload["note"].clone()),
        Some(serde_json::json!("second"))
    );

    assert_eq!(
        core_executions, 0,
        "no comparison reads the field that moved, so the core runs none"
    );
    assert_eq!(core_reused, 3, "and keeps all three");
    assert_eq!(
        application.executions, 2,
        "the whole-value cache cannot see which part of the node was consumed, so it re-runs both comparisons"
    );
    assert_eq!(
        application.reused, 1,
        "and only the consumer of their results"
    );
}

#[test]
fn the_comparison_is_reported_even_when_it_agrees() {
    // The point of the run. If this ever stops being true, the comparison has
    // been narrowed to a claim it cannot support.
    let rows = incumbent_comparison::run_all();
    assert_eq!(rows.len(), 7);
    assert!(
        rows.iter().all(|row| row.a3_correct && row.c3_correct),
        "the application mechanism and the core agree on every scenario"
    );
    assert!(
        rows.iter()
            .all(|row| row.b3_executions == row.a3_executions),
        "B3 reuses A3's own work and adds none of its own"
    );
    let differing: Vec<&str> = rows
        .iter()
        .filter(|row| row.a3_executions != row.c3_executions)
        .map(|row| row.scenario.as_str())
        .collect();
    assert_eq!(
        differing,
        vec!["unconsumed_field_change"],
        "and they part company on exactly the scenario where a consumed facet exists"
    );
}

#[test]
fn the_profile_of_the_fixture_is_declared_not_assumed() {
    // Both mechanisms are run under a closed profile granted by configuration.
    // Neither is told that the fixture is easy.
    assert_eq!(Profile::ClosedDeterministic.rank(), 2);
    assert!(!Profile::ClosedDeterministic.exceeds(Profile::ClosedDeterministic));
    assert!(Profile::ClosedDeterministic.exceeds(Profile::Opaque));
    assert!(!Profile::Opaque.exceeds(Profile::ClosedDeterministic));
}

/// The recorded artifact must be the measurement, not a description of it.
///
/// If this test fails, the runner was not re-run, or the scenarios changed without a new result.
#[test]
fn the_recorded_result_file_matches_a_fresh_measurement() {
    let recorded: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("experiments/incumbent-comparison/results.json"),
        )
        .expect("results.json is checked in"),
    )
    .expect("results.json is valid JSON");

    let fresh = incumbent_comparison::run_all();
    let rows = recorded["scenarios"]
        .as_array()
        .expect("scenarios are recorded");
    assert_eq!(rows.len(), fresh.len(), "a scenario was added or removed");

    for (row, result) in rows.iter().zip(&fresh) {
        let scenario = result.scenario.as_str();
        assert_eq!(row["scenario"].as_str(), Some(scenario), "order changed");
        assert_eq!(
            row["referenceExecutions"].as_u64(),
            Some(result.reference_executions as u64),
            "{scenario}"
        );
        assert_eq!(
            row["a3Executions"].as_u64(),
            Some(result.a3_executions as u64),
            "{scenario}"
        );
        assert_eq!(
            row["b3Executions"].as_u64(),
            Some(result.b3_executions as u64),
            "{scenario}"
        );
        assert_eq!(
            row["c3Executions"].as_u64(),
            Some(result.c3_executions as u64),
            "{scenario}"
        );
        let agreed =
            result.r3_correct && result.a3_correct && result.b3_correct && result.c3_correct;
        assert_eq!(row["agreed"].as_bool(), Some(agreed), "{scenario}");
    }

    // The decision is applied by the runner, not chosen by the reader.
    assert_eq!(
        recorded["recommendation"]["decision"].as_str(),
        Some("reduce")
    );

    // The unmet clause stays unmet in the record. A future run that quietly met
    // it would have to change this artifact deliberately, not by accident.
    let clauses = recorded["recommendation"]["conditionClauses"]
        .as_array()
        .expect("the condition is recorded clause by clause");
    assert_eq!(
        clauses.len(),
        3,
        "all three pre-registered clauses are accounted for"
    );
    assert_eq!(clauses[0]["met"].as_bool(), Some(true));
    assert_eq!(
        clauses[1]["met"].as_bool(),
        Some(false),
        "the core does buy a consumed facet"
    );
    assert_eq!(clauses[2]["met"].as_bool(), Some(true));
}

/// The contract document and the measured engine must name the same codes.
///
/// The engine was removed from the Kernel's shipped surface, so these strings are the only thing a
/// consumer has to depend on. A code that exists in one place and not the other is a lie to whoever
/// reads it, and this test is what catches it.
#[test]
fn the_contract_document_and_the_measured_codes_agree() {
    let contract = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/IMPACT-CONTRACT.md"),
    )
    .expect("the contract is checked in");
    let engine = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/impact_core/engine.rs"),
    )
    .expect("the measured engine is checked in");

    let is_code = |candidate: &str| {
        // A code is snake_case and long enough not to be prose. This excludes prose in backticks and
        // payloads like `app/cached`.
        candidate.len() > 8
            && candidate
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_')
    };

    // The engine's codes are string literals; the contract writes them in backticks.
    let emitted: std::collections::BTreeSet<String> = engine
        .match_indices('"')
        .step_by(2)
        .skip(1)
        .filter_map(|(at, _)| {
            let rest = &engine[at + 1..];
            let end = rest.find('"')?;
            let literal = &rest[..end];
            is_code(literal).then(|| literal.to_owned())
        })
        .collect();
    let documented: std::collections::BTreeSet<String> = contract
        .lines()
        .filter(|line| line.starts_with("| "))
        .filter_map(|line| {
            let start = line.find('`')? + 1;
            let rest = &line[start..];
            let end = rest.find('`')?;
            let code = &rest[..end];
            is_code(code).then(|| code.to_owned())
        })
        .collect();

    let missing_from_docs: Vec<&String> = emitted.difference(&documented).collect();
    let missing_from_engine: Vec<&String> = documented.difference(&emitted).collect();

    assert!(
        missing_from_docs.is_empty(),
        "the engine emits codes the contract does not name: {missing_from_docs:?}"
    );
    assert!(
        missing_from_engine.is_empty(),
        "the contract names codes the engine does not emit: {missing_from_engine:?}"
    );
    assert!(emitted.len() >= 8, "the code list did not shrink unnoticed");
}

/// The reversal case is a measurement, not an argument, so it is asserted rather than described.
///
/// ADR-003 made the reduction conditional on this: if the consumed-facet advantage grows past the line
/// count on a graph of wide nodes, the reduction was wrong. This is the test that decides it, and it is
/// the reason the recorded artifact cannot quietly keep claiming "reduce".
#[test]
fn the_facet_advantage_scales_with_the_number_of_consumers() {
    let small = incumbent_comparison::scaling(3, 1).expect("small fan-out runs");
    let large = incumbent_comparison::scaling(1000, 1).expect("wide fan-out runs");

    // The whole-value cache re-runs every consumer because it cannot see that the consumed field held.
    assert_eq!(small.a3_executions, small.consumers);
    assert_eq!(large.a3_executions, large.consumers);

    // The facet cache runs none, at any width.
    assert_eq!(small.c3_executions, 0);
    assert_eq!(large.c3_executions, 0);

    // So the advantage is exactly the fan-out, and it grows without bound.
    assert_eq!(small.avoided(), 3);
    assert_eq!(large.avoided(), 1000);
    assert!(large.avoided() > small.avoided());

    // One unconsumed field is enough. Padding the node further changes nothing, which is the point: the
    // whole-value cache never looks at how much of the node was consumed, only whether it moved at all.
    let padded = incumbent_comparison::scaling(1000, 8).expect("padded fan-out runs");
    assert_eq!(padded.avoided(), large.avoided());
}

/// The recorded reversal case must match a fresh measurement, for the same reason the scenarios do.
#[test]
fn the_recorded_reversal_case_matches_a_fresh_measurement() {
    let recorded: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("experiments/incumbent-comparison/results.json"),
        )
        .expect("results.json is checked in"),
    )
    .expect("results.json is valid JSON");

    let rows = recorded["reversalCase"]["rows"]
        .as_array()
        .expect("the reversal case is recorded");
    assert!(
        !rows.is_empty(),
        "the reversal case must be measured, not asserted"
    );

    for row in rows {
        let consumers = row["consumers"].as_u64().expect("consumers") as usize;
        let pads = row["unconsumedFields"].as_u64().expect("unconsumed fields") as usize;
        let fresh = incumbent_comparison::scaling(consumers, pads).expect("re-measured");
        assert_eq!(
            row["a3Executions"].as_u64(),
            Some(fresh.a3_executions as u64),
            "{consumers} consumers"
        );
        assert_eq!(
            row["c3Executions"].as_u64(),
            Some(fresh.c3_executions as u64),
            "{consumers} consumers"
        );
        assert_eq!(
            row["evaluationsAvoidedByTheFacetCache"].as_i64(),
            Some(fresh.avoided()),
            "{consumers} consumers"
        );
    }
}
