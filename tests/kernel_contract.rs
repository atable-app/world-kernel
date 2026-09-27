use std::collections::BTreeSet;

use tempfile::tempdir;
use world_kernel::{
    AcceptedAssessment, Candidate, Coverage, GroundedChange, Intent, Kernel, ObjectRevision, Patch,
    RejectionCode, StaticAuthority, SubmissionOutcome, WorldBootstrap,
};

fn authority(actor: &str, permission: &str) -> StaticAuthority {
    StaticAuthority::new([(actor.to_owned(), BTreeSet::from([permission.to_owned()]))])
}

fn valid_change() -> GroundedChange {
    GroundedChange {
        schema: "world-change/v0-experimental".into(),
        world: "world:test".into(),
        proposal_id: "change:1".into(),
        idempotency_key: "idem:1".into(),
        actor: "principal:worker-a".into(),
        base_revision: 0,
        intent: Intent {
            kind: "publishCandidate".into(),
            target: "artifact:demo".into(),
        },
        candidate: Candidate {
            reference: "artifact:demo".into(),
            digest: "sha256:candidate-a".into(),
        },
        reads: vec![ObjectRevision {
            reference: "requirement:demo".into(),
            revision: 1,
            digest: "sha256:requirement-v1".into(),
        }],
        coverage: Coverage {
            profile: "demo-closed-v1".into(),
            complete_for: BTreeSet::from(["publishCandidate".into()]),
            truncated: false,
        },
        assessments: vec![AcceptedAssessment {
            provider: "uni".into(),
            subject: "artifact:demo".into(),
            subject_digest: "sha256:candidate-a".into(),
            assessment_ref: "assessment:81".into(),
        }],
        patches: vec![Patch::PutObject {
            reference: "artifact:demo".into(),
            expected_revision: None,
            digest: "sha256:candidate-a".into(),
        }],
    }
}

#[test]
fn accepted_change_is_committed_once_and_replays_to_the_same_state() {
    let dir = tempdir().unwrap();
    let mut kernel = Kernel::create(
        dir.path().join("world.db"),
        WorldBootstrap {
            world: "world:test".into(),
            trusted_assurance_providers: BTreeSet::from(["uni".into()]),
            objects: vec![ObjectRevision {
                reference: "requirement:demo".into(),
                revision: 1,
                digest: "sha256:requirement-v1".into(),
            }],
        },
    )
    .unwrap();

    let outcome = kernel
        .submit(
            &valid_change(),
            &authority("principal:worker-a", "publishCandidate"),
        )
        .unwrap();

    let SubmissionOutcome::Committed(receipt) = outcome else {
        panic!("expected a committed change");
    };
    assert_eq!(receipt.world_revision, 1);
    assert_eq!(receipt.candidate_digest, "sha256:candidate-a");

    let snapshot = kernel.snapshot().unwrap();
    assert_eq!(snapshot.revision, 1);
    assert_eq!(
        snapshot.objects["artifact:demo"].digest,
        "sha256:candidate-a"
    );
    assert_eq!(kernel.replay().unwrap(), snapshot);
}

#[test]
fn the_serialized_envelope_uses_exactly_the_published_schema_field_names() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/schemas/world-change-v0.experimental.schema.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let envelope = serde_json::to_value(valid_change()).unwrap();
    let properties = schema["properties"].as_object().unwrap();
    let required = schema["required"].as_array().unwrap();

    let emitted = envelope.as_object().unwrap();
    for name in emitted.keys() {
        assert!(
            properties.contains_key(name),
            "envelope emits {name}, which the schema does not declare"
        );
    }
    for name in required {
        assert!(
            emitted.contains_key(name.as_str().unwrap()),
            "envelope omits the required field {name}"
        );
    }
    assert_eq!(
        properties.len(),
        emitted.len(),
        "schema and envelope disagree"
    );

    let patch_schema = &schema["properties"]["patches"]["items"];
    let patch_properties = patch_schema["properties"].as_object().unwrap();
    let patch = envelope["patches"][0].as_object().unwrap();
    for name in patch.keys() {
        assert!(
            patch_properties.contains_key(name),
            "patch emits {name}, which the schema does not declare"
        );
    }
    assert_eq!(
        patch_properties.len(),
        patch.len(),
        "schema and patch disagree"
    );
}

#[test]
fn an_unknown_field_in_a_proposal_is_rejected_rather_than_ignored() {
    let mut envelope = serde_json::to_value(valid_change()).unwrap();
    let object = envelope.as_object_mut().unwrap();
    object.insert("baseRevison".to_owned(), serde_json::json!(7));

    let error = serde_json::from_value::<GroundedChange>(envelope).unwrap_err();

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn an_unknown_field_in_a_patch_is_rejected_rather_than_ignored() {
    let mut envelope = serde_json::to_value(valid_change()).unwrap();
    envelope["patches"][0].as_object_mut().unwrap().insert(
        "expectedRevisionIgnored".to_owned(),
        serde_json::json!(null),
    );

    let error = serde_json::from_value::<GroundedChange>(envelope).unwrap_err();

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn repeating_the_same_idempotency_key_returns_the_original_receipt() {
    let dir = tempdir().unwrap();
    let mut kernel = Kernel::create(
        dir.path().join("world.db"),
        WorldBootstrap {
            world: "world:test".into(),
            trusted_assurance_providers: BTreeSet::from(["uni".into()]),
            objects: vec![ObjectRevision {
                reference: "requirement:demo".into(),
                revision: 1,
                digest: "sha256:requirement-v1".into(),
            }],
        },
    )
    .unwrap();
    let grants = authority("principal:worker-a", "publishCandidate");

    let first = kernel.submit(&valid_change(), &grants).unwrap();
    let second = kernel.submit(&valid_change(), &grants).unwrap();

    assert_eq!(second, first);
    assert_eq!(kernel.snapshot().unwrap().revision, 1);
}

#[test]
fn assessment_for_another_candidate_is_rejected_without_state_change() {
    let dir = tempdir().unwrap();
    let mut kernel = test_kernel(dir.path().join("world.db"));
    let mut change = valid_change();
    change.assessments[0].subject_digest = "sha256:somewhere-else".into();

    let outcome = kernel
        .submit(
            &change,
            &authority("principal:worker-a", "publishCandidate"),
        )
        .unwrap();

    assert_rejected(outcome, RejectionCode::AssessmentMismatch);
    assert_eq!(kernel.snapshot().unwrap().revision, 0);
}

#[test]
fn current_authority_is_checked_at_admission_time() {
    let dir = tempdir().unwrap();
    let mut kernel = test_kernel(dir.path().join("world.db"));

    let outcome = kernel
        .submit(&valid_change(), &StaticAuthority::default())
        .unwrap();

    assert_rejected(outcome, RejectionCode::Unauthorized);
    assert_eq!(kernel.snapshot().unwrap().revision, 0);
}

#[test]
fn stale_dependency_is_rejected_without_partial_patch() {
    let dir = tempdir().unwrap();
    let mut kernel = test_kernel(dir.path().join("world.db"));
    let mut change = valid_change();
    change.reads[0].revision = 2;
    change.patches.push(Patch::PutObject {
        reference: "artifact:side-effect".into(),
        expected_revision: None,
        digest: "sha256:must-not-appear".into(),
    });

    let outcome = kernel
        .submit(
            &change,
            &authority("principal:worker-a", "publishCandidate"),
        )
        .unwrap();

    assert_rejected(outcome, RejectionCode::StaleDependency);
    let snapshot = kernel.snapshot().unwrap();
    assert_eq!(snapshot.revision, 0);
    assert!(!snapshot.objects.contains_key("artifact:demo"));
    assert!(!snapshot.objects.contains_key("artifact:side-effect"));
}

#[test]
fn incomplete_context_view_cannot_authorize_a_change() {
    let dir = tempdir().unwrap();
    let mut kernel = test_kernel(dir.path().join("world.db"));
    let mut change = valid_change();
    change.coverage.truncated = true;

    let outcome = kernel
        .submit(
            &change,
            &authority("principal:worker-a", "publishCandidate"),
        )
        .unwrap();

    assert_rejected(outcome, RejectionCode::IncompleteView);
}

#[test]
fn a_producer_cannot_mint_assurance_from_an_untrusted_provider_name() {
    let dir = tempdir().unwrap();
    let mut kernel = test_kernel(dir.path().join("world.db"));
    let mut change = valid_change();
    change.assessments[0].provider = "self-declared-verifier".into();

    let outcome = kernel
        .submit(
            &change,
            &authority("principal:worker-a", "publishCandidate"),
        )
        .unwrap();

    assert_rejected(outcome, RejectionCode::UntrustedAssessmentProvider);
}

#[test]
fn an_idempotency_key_cannot_be_reused_for_another_change() {
    let dir = tempdir().unwrap();
    let mut kernel = test_kernel(dir.path().join("world.db"));
    let grants = authority("principal:worker-a", "publishCandidate");
    kernel.submit(&valid_change(), &grants).unwrap();
    let mut conflicting = valid_change();
    conflicting.proposal_id = "change:other".into();

    let outcome = kernel.submit(&conflicting, &grants).unwrap();

    assert_rejected(outcome, RejectionCode::IdempotencyConflict);
    assert_eq!(kernel.snapshot().unwrap().revision, 1);
}

#[test]
fn another_process_can_reopen_the_world_and_continue_from_the_same_history() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("world.db");
    let expected = {
        let mut kernel = test_kernel(&path);
        kernel
            .submit(
                &valid_change(),
                &authority("principal:worker-a", "publishCandidate"),
            )
            .unwrap();
        kernel.snapshot().unwrap()
    };

    let reopened = Kernel::open(&path, "world:test").unwrap();

    assert_eq!(reopened.snapshot().unwrap(), expected);
    assert_eq!(reopened.replay().unwrap(), expected);
}

#[test]
fn failure_at_the_last_durable_write_rolls_back_projection_and_event() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("world.db");
    let mut kernel = test_kernel(&path);
    let fault_connection = rusqlite::Connection::open(&path).unwrap();
    fault_connection
        .execute_batch(
            "CREATE TRIGGER interrupt_before_commit
             BEFORE INSERT ON submissions
             BEGIN
               SELECT RAISE(ABORT, 'simulated interruption before commit');
             END;",
        )
        .unwrap();

    let result = kernel.submit(
        &valid_change(),
        &authority("principal:worker-a", "publishCandidate"),
    );

    assert!(result.is_err());
    let snapshot = kernel.snapshot().unwrap();
    assert_eq!(snapshot.revision, 0);
    assert!(!snapshot.objects.contains_key("artifact:demo"));
    assert_eq!(kernel.replay().unwrap(), snapshot);
}

#[test]
fn lost_response_after_commit_returns_the_same_receipt_after_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("world.db");
    let grants = authority("principal:worker-a", "publishCandidate");
    let original = {
        let mut kernel = test_kernel(&path);
        kernel.submit(&valid_change(), &grants).unwrap()
    };

    let mut restarted = Kernel::open(&path, "world:test").unwrap();
    let recovered = restarted.submit(&valid_change(), &grants).unwrap();

    assert_eq!(recovered, original);
    assert_eq!(restarted.snapshot().unwrap().revision, 1);
}

fn test_kernel(path: impl AsRef<std::path::Path>) -> Kernel {
    Kernel::create(
        path,
        WorldBootstrap {
            world: "world:test".into(),
            trusted_assurance_providers: BTreeSet::from(["uni".into()]),
            objects: vec![ObjectRevision {
                reference: "requirement:demo".into(),
                revision: 1,
                digest: "sha256:requirement-v1".into(),
            }],
        },
    )
    .unwrap()
}

fn assert_rejected(outcome: SubmissionOutcome, code: RejectionCode) {
    let SubmissionOutcome::Rejected(rejection) = outcome else {
        panic!("expected rejection");
    };
    assert_eq!(rejection.code, code);
}
