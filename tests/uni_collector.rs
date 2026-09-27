use std::{collections::BTreeSet, fs, path::Path};

use serde_json::{Value, json};
use tempfile::tempdir;
use world_kernel::adapters::uni::{
    CollectRequest, ProcessUniRunner, UniCollector, UniCollectorError, UniCommandRunner,
    candidate_from_file,
};
use world_kernel::{
    Coverage, GroundedChange, Intent, Kernel, ObjectRevision, Patch, RejectionCode,
    StaticAuthority, SubmissionOutcome, WorldBootstrap,
};

struct MutatingRunner;

struct AcceptedRunner;

struct UncoveredRunner;

impl UniCommandRunner for AcceptedRunner {
    fn verify(&self, workspace: &Path, _contract: &Path) -> Result<Value, UniCollectorError> {
        let digest = candidate_from_file(workspace.join("candidate.bin"), "unused")?
            .digest
            .trim_start_matches("sha256:")
            .to_owned();
        Ok(json!({
            "decision": "Accepted",
            "evidence": [{
                "claim_id": "release",
                "state": "Valid",
                "artifact_files": {"candidate.bin": digest}
            }]
        }))
    }

    fn report(&self, _workspace: &Path) -> Result<Value, UniCollectorError> {
        Ok(json!({
            "intent": {"id": "release"},
            "decision": "Accepted",
            "reason": "all required claims verified",
            "summary": {"claims_total": 1, "claims_verified": 1},
            "claims": [{"claim_id": "release", "state": "Valid"}],
            "assurance": "A2"
        }))
    }
}

impl UniCommandRunner for UncoveredRunner {
    fn verify(&self, _workspace: &Path, _contract: &Path) -> Result<Value, UniCollectorError> {
        Ok(json!({
            "decision": "Accepted",
            "evidence": [{
                "claim_id": "release",
                "state": "Valid",
                "artifact_files": {"other.bin": "not-the-candidate"}
            }]
        }))
    }

    fn report(&self, _workspace: &Path) -> Result<Value, UniCollectorError> {
        AcceptedRunner.report(_workspace)
    }
}

impl UniCommandRunner for MutatingRunner {
    fn verify(&self, _workspace: &Path, _contract: &Path) -> Result<Value, UniCollectorError> {
        fs::write(_workspace.join("candidate.bin"), b"candidate-b")?;
        Ok(json!({"decision": "Accepted"}))
    }

    fn report(&self, _workspace: &Path) -> Result<Value, UniCollectorError> {
        Ok(json!({
            "intent": {"id": "release"},
            "decision": "Accepted",
            "reason": "all required claims verified",
            "summary": {"claims_total": 1, "claims_verified": 1},
            "claims": [{"claim_id": "release", "state": "Valid"}],
            "assurance": "A2"
        }))
    }
}

#[test]
fn candidate_changed_during_uni_verification_produces_no_assessment() {
    let directory = tempdir().unwrap();
    let candidate = directory.path().join("candidate.bin");
    let contract = directory.path().join("release.uni");
    fs::write(&candidate, b"candidate-a").unwrap();
    fs::write(&contract, b"UNI 0.1").unwrap();
    let collector = UniCollector::new(MutatingRunner);

    let error = collector
        .collect(&CollectRequest {
            workspace: directory.path().to_path_buf(),
            contract,
            candidate_path: candidate,
            candidate_ref: "artifact:release".into(),
        })
        .unwrap_err();

    assert_eq!(error.code(), "CANDIDATE_CHANGED_DURING_VERIFICATION");
}

#[test]
fn accepted_uni_decision_without_exact_candidate_evidence_is_rejected() {
    let directory = tempdir().unwrap();
    let candidate = directory.path().join("candidate.bin");
    let contract = directory.path().join("release.uni");
    fs::write(&candidate, b"candidate-a").unwrap();
    fs::write(&contract, b"UNI 0.1").unwrap();

    let error = UniCollector::new(UncoveredRunner)
        .collect(&CollectRequest {
            workspace: directory.path().to_path_buf(),
            contract,
            candidate_path: candidate,
            candidate_ref: "artifact:release".into(),
        })
        .unwrap_err();

    assert_eq!(error.code(), "CANDIDATE_NOT_COVERED_BY_UNI");
}

#[test]
fn process_runner_executes_verify_then_report_and_binds_the_exact_digest() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().unwrap();
    let candidate = directory.path().join("candidate.bin");
    let contract = directory.path().join("release.uni");
    let fake_uni = directory.path().join("uni");
    fs::write(&candidate, b"abc").unwrap();
    fs::write(&contract, b"UNI 0.1").unwrap();
    fs::write(
        &fake_uni,
        r#"#!/bin/sh
if [ "$1" = "--json" ] && [ "$2" = "verify" ]; then
  printf '%s\n' '{"decision":"Accepted","evidence":[{"claim_id":"release","state":"Valid","artifact_files":{"candidate.bin":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}}]}'
  exit 0
fi
if [ "$1" = "--json" ] && [ "$2" = "bundle" ] && [ "$3" = "export" ]; then
  printf '%s\n' \
    '{"kind":"header","body":{"version":"uni-bundle-0.1"}}' \
    '{"kind":"evidence","body":{"claim_id":"release","state":"Valid","artifact_files":{"candidate.bin":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}}}' \
    > "$6"
  printf '%s\n' '{"bundle":"test","version":"uni-bundle-0.1","records":1}'
  exit 0
fi
if [ "$1" = "--json" ] && [ "$2" = "bundle" ] && [ "$3" = "verify" ]; then
  printf '%s\n' '{"ok":true,"records":1}'
  exit 0
fi
if [ "$1" = "--json" ] && [ "$2" = "report" ]; then
  printf '%s\n' '{"intent":{"id":"release"},"decision":"Accepted","reason":"ok","summary":{"claims_total":1,"claims_verified":1},"claims":[{"claim_id":"release","state":"Valid"}],"assurance":"A2"}'
  exit 0
fi
printf '%s\n' 'unexpected arguments' >&2
exit 64
"#,
    )
    .unwrap();
    fs::set_permissions(&fake_uni, fs::Permissions::from_mode(0o755)).unwrap();
    let collector = UniCollector::new(ProcessUniRunner::new(&fake_uni));

    let collected = collector
        .collect(&CollectRequest {
            workspace: directory.path().to_path_buf(),
            contract,
            candidate_path: candidate,
            candidate_ref: "artifact:release".into(),
        })
        .unwrap();

    assert_eq!(
        collected.candidate.digest,
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        collected.assessment.subject_digest,
        collected.candidate.digest
    );
    assert!(
        collected
            .assessment
            .assessment_ref
            .starts_with("uni-bound:")
    );
}

#[test]
#[ignore = "requires UNI_BIN pointing to a built UNI CLI"]
fn real_uni_cli_binds_valid_evidence_to_the_candidate() {
    let uni_binary = std::env::var_os("UNI_BIN").expect("UNI_BIN must point to the UNI binary");
    let directory = tempdir().unwrap();
    let dot_uni = directory.path().join(".uni");
    fs::create_dir_all(dot_uni.join("evidence")).unwrap();
    fs::write(
        dot_uni.join("config.toml"),
        r#"[verifiers."candidate.ready"]
run = "true"
files = ["candidate.bin"]
"#,
    )
    .unwrap();
    let candidate = directory.path().join("candidate.bin");
    let contract = directory.path().join("release.uni");
    fs::write(&candidate, b"release-candidate").unwrap();
    fs::write(
        &contract,
        r#"VERSION 0.1
DOMAIN software
INTENT release
GOAL
  Publish the verified candidate.
CLAIM candidate-ready REQUIRED
  ENSURE the candidate is ready
VERIFY candidate-ready
  USING candidate.ready
ACCEPT WHEN
  required_claims == VERIFIED
  AND critical_failures == 0
"#,
    )
    .unwrap();

    let collector = UniCollector::new(ProcessUniRunner::new(uni_binary));
    let request = CollectRequest {
        workspace: directory.path().to_path_buf(),
        contract,
        candidate_path: candidate,
        candidate_ref: "artifact:release".into(),
    };
    let collected = collector.collect(&request).unwrap();
    let repeated = collector.collect(&request).unwrap();

    assert_eq!(collected.assessment.provider, "uni");
    assert_eq!(
        collected.assessment.subject_digest,
        collected.candidate.digest
    );
    assert!(
        collected
            .assessment
            .assessment_ref
            .starts_with("uni-bound:")
    );
    assert_eq!(
        repeated.assessment.assessment_ref,
        collected.assessment.assessment_ref
    );
}

#[test]
fn candidate_swapped_after_assurance_is_rejected_at_admission() {
    let directory = tempdir().unwrap();
    let candidate_path = directory.path().join("candidate.bin");
    let contract = directory.path().join("release.uni");
    fs::write(&candidate_path, b"candidate-a").unwrap();
    fs::write(&contract, b"UNI 0.1").unwrap();
    let collected = UniCollector::new(AcceptedRunner)
        .collect(&CollectRequest {
            workspace: directory.path().to_path_buf(),
            contract,
            candidate_path: candidate_path.clone(),
            candidate_ref: "artifact:release".into(),
        })
        .unwrap();

    fs::write(&candidate_path, b"candidate-b").unwrap();
    let current_candidate = candidate_from_file(&candidate_path, "artifact:release").unwrap();
    assert_ne!(current_candidate.digest, collected.candidate.digest);

    let mut kernel = Kernel::create(
        directory.path().join("world.db"),
        WorldBootstrap {
            world: "world:release".into(),
            trusted_assurance_providers: BTreeSet::from(["uni".into()]),
            objects: vec![ObjectRevision {
                reference: "requirement:release".into(),
                revision: 1,
                digest: "sha256:requirement-v1".into(),
            }],
        },
    )
    .unwrap();
    let change = GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: "world:release".into(),
        proposal_id: "change:release".into(),
        idempotency_key: "release:1".into(),
        actor: "principal:worker-a".into(),
        base_revision: 0,
        intent: Intent {
            kind: "publishCandidate".into(),
            target: current_candidate.reference.clone(),
        },
        candidate: current_candidate.clone(),
        reads: vec![ObjectRevision {
            reference: "requirement:release".into(),
            revision: 1,
            digest: "sha256:requirement-v1".into(),
        }],
        coverage: Coverage {
            profile: "release-closed-v1".into(),
            complete_for: BTreeSet::from(["publishCandidate".into()]),
            truncated: false,
        },
        assessments: vec![collected.assessment],
        patches: vec![Patch::PutObject {
            reference: current_candidate.reference,
            expected_revision: None,
            digest: current_candidate.digest,
        }],
    };
    let authority = StaticAuthority::new([(
        "principal:worker-a".into(),
        BTreeSet::from(["publishCandidate".into()]),
    )]);

    let outcome = kernel.submit(&change, &authority).unwrap();

    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("expected the swapped candidate to be rejected");
    };
    assert_eq!(rejection.code, RejectionCode::AssessmentMismatch);
    assert_eq!(kernel.snapshot().unwrap().revision, 0);
}
