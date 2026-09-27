//! The corpus is a versioned measurement instrument. Criterion 2 requires it to
//! fail loudly on a missing field, an unknown enum value or a duplicate id, so
//! these tests feed the parser deliberate damage rather than trusting the
//! checked-in file to be well formed.

// Only the corpus model is needed here, so this test target does not pull in the
// three benchmark systems.
#[path = "support/admission_case.rs"]
mod admission_case;

use admission_case::{
    AdmissionCase, BenchmarkCode, CaseFamily, Corpus, CorpusError, ExpectedStatus,
    unsafe_candidate_path,
};

const CANDIDATE_DIGEST: &str =
    "sha256:9d7ae0dfd1e2a1e0e6db4b0a0b1b1f3b7f0d3a1a2c4e5f60718293a4b5c6d7e8";

fn admission_case_literal() -> String {
    format!(
        r#"{{"schema":"world-kernel-admission-case/v1","id":"candidate.substitution.001","family":"candidate_exactness","expected":{{"status":"rejected","code":"ASSESSMENT_MISMATCH"}},"bootstrap":{{"world":"world:bench","trustedAssuranceProviders":["uni"],"objects":[{{"ref":"requirement:demo","revision":1,"digest":"sha256:requirement-v1"}}]}},"prelude":[],"proposal":{{"schema":"world-change/v0-experimental","world":"world:bench","proposalId":"change:1","idempotencyKey":"idem:1","actor":"principal:worker-a","baseRevision":0,"intent":{{"kind":"publishCandidate","target":"artifact:demo"}},"candidate":{{"ref":"artifact:demo","digest":"{CANDIDATE_DIGEST}"}},"reads":[{{"ref":"requirement:demo","revision":1,"digest":"sha256:requirement-v1"}}],"coverage":{{"profile":"closed-v1","completeFor":["publishCandidate"],"truncated":false}},"assessments":[{{"provider":"uni","subject":"artifact:demo","subjectDigest":"sha256:somewhere-else","assessmentRef":"uni-bound:fixture:1"}}],"patches":[{{"op":"putObject","ref":"artifact:demo","expectedRevision":null,"digest":"{CANDIDATE_DIGEST}"}}]}},"current":{{"grants":{{"principal:worker-a":["publishCandidate"]}},"candidatePath":"candidate.bin","candidateContents":"candidate-a","candidateContentsAfterAssurance":null}},"assurance":{{"mode":"accepted","provider":"uni","evidencePath":"release.uni"}},"fault":null,"notes":"exact subject binding"}}"#
    )
}

fn parse_case(text: &str) -> Result<AdmissionCase, CorpusError> {
    admission_case::parse_case(text.trim(), 1)
}

fn assert_rejected_for(detail: &str, text: &str) {
    let error = parse_case(text).unwrap_err();
    assert!(
        error.to_string().contains(detail),
        "expected {detail:?} in {error}"
    );
}

fn assert_contract_rejects(detail: &str, text: &str) {
    let errors = Corpus::parse(text).unwrap().validate();
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains(detail)),
        "expected {detail:?} in {errors:#?}"
    );
}

#[test]
fn a_well_formed_case_deserializes_into_named_types() {
    let case = parse_case(&admission_case_literal()).unwrap();

    assert_eq!(case.schema, "world-kernel-admission-case/v1");
    assert_eq!(case.family, CaseFamily::CandidateExactness);
    assert_eq!(case.expected.status, ExpectedStatus::Rejected);
    assert_eq!(case.expected.code, Some(BenchmarkCode::AssessmentMismatch));
    assert_eq!(case.bootstrap.world, "world:bench");
    assert_eq!(case.current.candidate_path, "candidate.bin");
    assert_eq!(case.current.candidate_contents, "candidate-a");
    assert_eq!(case.current.candidate_contents_after_assurance, None);
    assert_eq!(case.proposal.intent.kind, "publishCandidate");
    assert_eq!(case.proposal.reads[0].reference, "requirement:demo");
    assert_eq!(case.assurance.mode, admission_case::AssuranceMode::Accepted);
    assert_eq!(case.fault, None);
}

#[test]
fn an_unknown_top_level_field_is_rejected() {
    assert_rejected_for(
        "unknown field",
        &admission_case_literal().replace(
            r#""notes":"exact subject binding""#,
            r#""notes":"exact subject binding","hint":"ignore me""#,
        ),
    );
}

#[test]
fn a_missing_required_field_is_rejected() {
    assert_rejected_for(
        "missing the required field fault",
        &admission_case_literal().replace(r#""fault":null,"#, ""),
    );
}

#[test]
fn a_missing_required_text_field_is_rejected() {
    assert_rejected_for(
        "missing the required field notes",
        &admission_case_literal().replace(r#","notes":"exact subject binding""#, ""),
    );
}

#[test]
fn an_unknown_family_is_rejected() {
    assert_rejected_for(
        "unknown variant",
        &admission_case_literal().replace(
            r#""family":"candidate_exactness""#,
            r#""family":"candidate_similarity""#,
        ),
    );
}

#[test]
fn an_unknown_assurance_mode_is_rejected() {
    assert_rejected_for(
        "unknown variant",
        &admission_case_literal().replace(r#""mode":"accepted""#, r#""mode":"maybe""#),
    );
}

#[test]
fn an_unknown_fault_point_is_rejected() {
    assert_rejected_for(
        "unknown variant",
        &admission_case_literal().replace(r#""fault":null"#, r#""fault":"afterTheLedgerFlush""#),
    );
}

#[test]
fn an_unknown_benchmark_code_is_rejected() {
    assert_rejected_for(
        "unknown variant",
        &admission_case_literal().replace(
            r#""code":"ASSESSMENT_MISMATCH""#,
            r#""code":"ASSESSMENT_PROBABLY_FINE""#,
        ),
    );
}

#[test]
fn an_unknown_expected_status_is_rejected() {
    assert_rejected_for(
        "unknown variant",
        &admission_case_literal().replace(r#""status":"rejected""#, r#""status":"confirmed""#),
    );
}

#[test]
fn a_case_with_the_wrong_envelope_schema_is_rejected() {
    assert_contract_rejects(
        "declares schema",
        &format!(
            "{}\n",
            admission_case_literal().replace(
                r#""schema":"world-kernel-admission-case/v1""#,
                r#""schema":"world-kernel-admission-case/v2""#,
            )
        ),
    );
}

#[test]
fn a_malformed_json_line_is_rejected() {
    let text = format!("{}\n{{ not json\n", admission_case_literal());

    let error = Corpus::parse(&text).unwrap_err();

    assert!(error.to_string().contains("line 2"), "{error}");
}

#[test]
fn duplicate_ids_are_rejected() {
    let first = admission_case_literal();
    let other_family = first.replace(
        r#""family":"candidate_exactness""#,
        r#""family":"authority""#,
    );
    let text = format!("{first}\n{other_family}\n");

    let errors = Corpus::parse(&text).unwrap().validate();
    let duplicates: Vec<&CorpusError> = errors
        .iter()
        .filter(|error| matches!(error, CorpusError::DuplicateId { .. }))
        .collect();

    assert_eq!(duplicates.len(), 1, "{errors:#?}");
    let CorpusError::DuplicateId { first, second, .. } = duplicates[0] else {
        unreachable!()
    };
    assert_eq!(*first, CaseFamily::CandidateExactness);
    assert_eq!(*second, CaseFamily::Authority);
}

#[test]
fn the_checked_in_corpus_satisfies_its_own_contract() {
    let corpus = Corpus::load_checked().expect("tests/fixtures/admission-corpus.jsonl is valid");
    let families: std::collections::BTreeSet<CaseFamily> =
        corpus.cases.iter().map(|case| case.family).collect();

    assert_eq!(corpus.cases.len(), 80);
    assert_eq!(
        corpus.cases.iter().filter(|case| case.is_adverse()).count(),
        60
    );
    assert_eq!(
        corpus
            .cases
            .iter()
            .filter(|case| !case.is_adverse())
            .count(),
        20
    );
    assert_eq!(families.len(), 14, "the contract lists fourteen families");
}

#[test]
fn every_committed_case_declares_its_current_authority_and_candidate() {
    let corpus = Corpus::load_checked().unwrap();

    for case in corpus.cases.iter().filter(|case| !case.is_adverse()) {
        assert!(
            case.current
                .grants
                .get(&case.proposal.actor)
                .is_some_and(|permissions| permissions.contains(&case.proposal.intent.kind)),
            "{} commits without a current grant",
            case.id
        );
        assert!(
            case.proposal.candidate.digest == case.expected_digest(),
            "{} does not publish the bytes the workspace holds",
            case.id
        );
    }
}

#[test]
fn no_committed_case_declares_an_unimplemented_capability() {
    let corpus = Corpus::load_checked().unwrap();

    for case in &corpus.cases {
        if case.expected.status == ExpectedStatus::Committed {
            assert_eq!(
                case.required_capability(),
                None,
                "{} claims a commit on an unimplemented capability",
                case.id
            );
        }
    }
}

#[test]
fn every_family_declares_its_own_implementation_status_honestly() {
    use admission_case::UnsupportedCapability;
    let corpus = Corpus::load_checked().unwrap();

    for case in &corpus.cases {
        let capability = case.required_capability();
        match case.family {
            CaseFamily::NegativeQueryDependency | CaseFamily::GraphTermination => assert!(
                matches!(capability, Some(UnsupportedCapability::CoverageProfile(_))),
                "{} must declare a future view protocol",
                case.id
            ),
            CaseFamily::ExternalEffectUncertainty => assert!(
                matches!(
                    (capability, case.expected.status),
                    (None, ExpectedStatus::Committed)
                        | (Some(UnsupportedCapability::IntentKind(_)), _)
                ),
                "{} mixes a supported publish with an unimplemented operation class",
                case.id
            ),
            _ => assert_eq!(capability, None, "{} declares a capability", case.id),
        }
    }
}

#[test]
fn every_kernel_rejection_code_has_a_matching_benchmark_code() {
    let codes = [
        world_kernel::RejectionCode::UnsupportedSchema,
        world_kernel::RejectionCode::WrongWorld,
        world_kernel::RejectionCode::StaleWorld,
        world_kernel::RejectionCode::Unauthorized,
        world_kernel::RejectionCode::IncompleteView,
        world_kernel::RejectionCode::StaleDependency,
        world_kernel::RejectionCode::AssessmentMismatch,
        world_kernel::RejectionCode::UntrustedAssessmentProvider,
        world_kernel::RejectionCode::CandidateMismatch,
        world_kernel::RejectionCode::PatchConflict,
        world_kernel::RejectionCode::IdempotencyConflict,
    ];

    for code in codes {
        let kernel_name = serde_json::to_value(&code).unwrap();
        let parsed: BenchmarkCode = serde_json::from_value(kernel_name).unwrap();
        assert_eq!(parsed, BenchmarkCode::from(&code));

        let benchmark_name = serde_json::to_value(parsed).unwrap();
        let benchmark_name = benchmark_name.as_str().unwrap();
        assert_eq!(parsed.as_str(), benchmark_name);
    }
}

#[test]
fn unsafe_candidate_paths_are_rejected() {
    for path in [
        "",
        "/etc/passwd",
        "../outside.bin",
        "a/../../b.bin",
        "./x.bin",
        "C:/x.bin",
    ] {
        assert!(
            unsafe_candidate_path(path).is_some(),
            "{path} should be rejected"
        );
    }
    for path in ["candidate.bin", "src/candidate.bin", "a/b/c.bin"] {
        assert_eq!(unsafe_candidate_path(path), None, "{path} should pass");
    }
}
