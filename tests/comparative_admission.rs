//! Criterion 3: A, B and C consume the same deserialized case object.
//! Criterion 4: every system produces a structured result for all 80 cases, and
//! a panic or a skipped case fails the run.
//! Criterion 5: no system incorrectly admits an adverse case it claims to
//! support.
//! Criterion 6: no system has more than one unjustified rejection among the 20
//! benign cases.
//! Criterion 7: known unsupported cases stay `unsupported`.
//! Criterion 8: each of the three test-only input ablations incorrectly admits
//! at least one matching adverse case.

mod support;

use serde_json::json;
use support::{
    admission_case::{BenchmarkCode, CaseFamily, Corpus, ExpectedStatus, UnsupportedCapability},
    harness::{
        ActualStatus, Benchmark, SystemId, expected_assurance_code, median, percentile_95, run_case,
    },
    system_c::Ablation,
};

fn corpus() -> Corpus {
    Corpus::load_checked().expect("the checked-in corpus is valid")
}

#[test]
fn every_system_produces_a_structured_result_for_every_case() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).expect("no case panics or is skipped");

    assert_eq!(benchmark.results.len(), 80 * 3);
    for system in SystemId::ALL {
        assert_eq!(benchmark.for_system(system).len(), 80);
        assert!(
            benchmark.errors(system).is_empty(),
            "system {} reported a harness error: {:?}",
            system.label(),
            benchmark.errors(system)
        );
    }
}

#[test]
fn no_system_incorrectly_admits_an_adverse_case_it_supports() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).unwrap();

    for system in SystemId::ALL {
        let admissions = benchmark.incorrect_admissions(system);
        assert!(
            admissions.is_empty(),
            "system {} admitted {admissions:#?}",
            system.label()
        );
    }
}

#[test]
fn no_system_has_more_than_one_unjustified_rejection() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).unwrap();

    for system in SystemId::ALL {
        let rejections = benchmark.unjustified_rejections(system);
        assert!(
            rejections.len() <= 1,
            "system {} unjustifiably rejected {rejections:#?}",
            system.label()
        );
    }
}

#[test]
fn known_unsupported_cases_are_never_rewritten_as_successes() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).unwrap();
    let unsupported: Vec<&str> = corpus
        .cases
        .iter()
        .filter(|case| case.expected.status == ExpectedStatus::Unsupported)
        .map(|case| case.id.as_str())
        .collect();

    assert_eq!(unsupported.len(), 9);
    for system in SystemId::ALL {
        for result in benchmark.for_system(system) {
            if !unsupported.contains(&result.case_id.as_str()) {
                continue;
            }
            assert_eq!(
                result.actual.status,
                ActualStatus::Unsupported,
                "system {} reported {:?} for unsupported case {}",
                system.label(),
                result.actual,
                result.case_id
            );
        }
    }
}

#[test]
fn an_adverse_case_in_a_supported_family_is_never_committed() {
    let corpus = corpus();
    let mut checked = 0;

    for case in corpus
        .cases
        .iter()
        .filter(|case| case.expected.status == ExpectedStatus::Rejected)
    {
        checked += 1;
        for system in SystemId::ALL {
            let result = run_case(system, case, None).unwrap();
            assert_ne!(
                result.actual.status,
                ActualStatus::Committed,
                "system {} admitted adverse case {}",
                system.label(),
                case.id
            );
        }
    }

    // 60 adverse = 45 rejected + 9 unsupported + 6 that commit only after a
    // clean retry from an injected interruption.
    assert_eq!(checked, 45, "the corpus composition changed");
}

#[test]
fn each_input_ablation_incorrectly_admits_a_matching_adverse_case() {
    let corpus = corpus();
    let expectations = [
        (Ablation::ForgeAssessment, CaseFamily::CandidateExactness),
        (
            Ablation::DropChangedReads,
            CaseFamily::WorldObjectConcurrency,
        ),
        (
            Ablation::CompleteTruncatedCoverage,
            CaseFamily::CoverageLimits,
        ),
    ];

    for (ablation, family) in expectations {
        let results = Benchmark::run_with_ablation(&corpus, SystemId::C, ablation).unwrap();
        let misadmitted: Vec<&str> = results
            .iter()
            .filter(|result| {
                result.actual.status == ActualStatus::Committed
                    && result.expected.status.ne(&ExpectedStatus::Committed)
            })
            .map(|result| result.case_id.as_str())
            .collect();
        assert!(
            !misadmitted.is_empty(),
            "ablation {} admitted nothing, so it proves no check",
            ablation.name()
        );
        let matching: Vec<&&str> = misadmitted
            .iter()
            .filter(|case_id| {
                corpus
                    .cases
                    .iter()
                    .any(|case| case.id == **case_id && case.family == family)
            })
            .collect();
        assert!(
            !matching.is_empty(),
            "ablation {} only admitted cases outside {family:?}: {misadmitted:?}",
            ablation.name()
        );
    }
}

#[test]
fn the_unablated_kernel_still_rejects_what_every_ablation_admits() {
    let corpus = corpus();
    let baseline = Benchmark::run(&corpus).unwrap();

    for ablation in Ablation::ALL {
        let ablated = Benchmark::run_with_ablation(&corpus, SystemId::C, ablation).unwrap();
        let weakened = ablated
            .iter()
            .filter(|result| {
                result.actual.status == ActualStatus::Committed
                    && result.expected.status != ExpectedStatus::Committed
            })
            .count();
        let unweakened = baseline
            .for_system(SystemId::C)
            .iter()
            .filter(|result| {
                result.actual.status == ActualStatus::Committed
                    && result.expected.status != ExpectedStatus::Committed
            })
            .count();
        assert_eq!(unweakened, 0, "the Kernel must admit no adverse case");
        assert!(
            weakened > 0,
            "ablation {} weakened nothing",
            ablation.name()
        );
    }
}

#[test]
fn the_three_systems_agree_on_every_case_they_all_support() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).unwrap();
    let disagreements = benchmark.decision_disagreements(&corpus);

    assert!(
        disagreements.is_empty(),
        "the systems reached different decisions: {disagreements:#?}"
    );
}

#[test]
fn the_assurance_fixture_mode_always_produces_its_declared_code() {
    let corpus = corpus();

    for case in &corpus.cases {
        let declared = expected_assurance_code(case);
        let expected = case.expected.code;
        let expected_is_assurance = expected.is_some_and(|code| {
            matches!(
                code,
                BenchmarkCode::UniNotAccepted
                    | BenchmarkCode::CandidateNotCoveredByUni
                    | BenchmarkCode::CandidateChangedDuringVerification
                    | BenchmarkCode::UniCommandFailed
                    | BenchmarkCode::UniBundleInvalid
                    | BenchmarkCode::UniBundleUnsupported
            )
        });
        if expected_is_assurance {
            assert_eq!(
                expected.map(|code| code.as_str()),
                declared,
                "case {} expects an assurance refusal the fixture does not produce",
                case.id
            );
        } else {
            assert!(
                expected == Some(BenchmarkCode::Unsupported) || declared.is_none(),
                "case {} declares the failing mode {} but expects {:?}",
                case.id,
                declared.unwrap_or(""),
                expected
            );
        }
    }
}

#[test]
fn the_aggregate_accounts_for_every_case_and_every_failure() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).unwrap();

    assert!(
        benchmark.failures().is_empty(),
        "{:#?}",
        benchmark.failures()
    );
    for system in SystemId::ALL {
        let results = benchmark.for_system(system);
        assert_eq!(results.len(), 80);

        // Every result lands in exactly one reported bucket, so no failure can
        // be filtered out of the totals.
        let bucket = |status| {
            results
                .iter()
                .filter(|result| result.actual.status == status)
                .count()
        };
        assert_eq!(
            bucket(ActualStatus::Error),
            0,
            "system {} errored",
            system.label()
        );
        assert_eq!(
            bucket(ActualStatus::Committed)
                + bucket(ActualStatus::Rejected)
                + bucket(ActualStatus::Unsupported)
                + bucket(ActualStatus::Error),
            results.len(),
            "system {} has results outside every reported bucket",
            system.label()
        );

        let unsupported = benchmark.unsupported_by_family(system, &corpus);
        assert_eq!(unsupported.len(), 3, "{unsupported:?}");
        assert_eq!(
            unsupported.values().sum::<usize>(),
            9,
            "nine cases are unsupported and none may be hidden"
        );

        let timings = benchmark.timings_micros(system);
        assert_eq!(timings.len(), 80);
        assert!(median(&timings) <= percentile_95(&timings));
    }
}

#[test]
fn the_declared_support_surface_is_printed_not_hidden() {
    let declared = support::harness::declared_support();

    assert_eq!(declared["systems"], json!(["A", "B", "C"]));
    assert_eq!(
        declared["supportedIntentKinds"],
        json!(["publishCandidate"])
    );
    assert_eq!(
        declared["unsupportedIntentKinds"],
        json!(["dispatchExternalEffect"])
    );
}

#[test]
fn an_accepted_case_embeds_the_assessment_its_own_assurance_produced() {
    let corpus = corpus();
    let mut checked = 0;

    for case in &corpus.cases {
        if case.assurance.mode != support::admission_case::AssuranceMode::Accepted {
            continue;
        }
        // A deliberately untrusted provider, a mismatched claim or a candidate
        // mismatch is the condition under test, not a corpus inconsistency.
        if case.expected.code.is_some_and(|code| {
            matches!(
                code,
                BenchmarkCode::AssessmentMismatch
                    | BenchmarkCode::CandidateMismatch
                    | BenchmarkCode::UntrustedAssessmentProvider
            )
        }) {
            continue;
        }
        let collected = collect_with_seam(case);
        let Some(collected) = collected else {
            continue;
        };
        let embedded = case
            .proposal
            .assessments
            .iter()
            .find(|assessment| assessment.provider == collected.assessment.provider)
            .expect("the producer embeds the assessment its assurance produced");
        assert_eq!(
            embedded.subject, collected.assessment.subject,
            "{}",
            case.id
        );
        assert_eq!(
            embedded.subject_digest, collected.assessment.subject_digest,
            "{}",
            case.id
        );
        checked += 1;
    }

    assert!(checked > 40, "only {checked} accepted cases were checked");
}

fn collect_with_seam(
    case: &support::admission_case::AdmissionCase,
) -> Option<support::assurance::CollectedForTest> {
    support::assurance::collect_for_test(case)
}

#[test]
fn a_benign_case_in_a_sibling_world_does_not_disturb_the_measured_scope() {
    let corpus = corpus();
    let benchmark = Benchmark::run(&corpus).unwrap();

    for result in benchmark.for_system(SystemId::C) {
        if result.case_id.starts_with("scope.") {
            assert!(result.correct, "{result:#?}");
        }
    }
}

#[test]
fn the_corpus_declares_only_capabilities_no_system_claims() {
    let corpus = corpus();

    for case in &corpus.cases {
        match case.required_capability() {
            None => assert_ne!(
                case.expected.status,
                ExpectedStatus::Unsupported,
                "{} is unsupported without a declaration",
                case.id
            ),
            Some(UnsupportedCapability::CoverageProfile(profile)) => {
                assert!(
                    profile.contains("v1"),
                    "{profile} is not a versioned profile"
                )
            }
            Some(UnsupportedCapability::IntentKind(kind)) => {
                assert!(!kind.is_empty())
            }
        }
    }
}
