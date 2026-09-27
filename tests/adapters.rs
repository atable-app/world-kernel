use serde_json::json;
use world_kernel::{
    Candidate,
    adapters::{kollio, uni},
};

#[test]
fn uni_report_becomes_an_assessment_only_for_an_explicit_exact_subject() {
    let report = json!({
        "intent": {"id": "release-demo"},
        "decision": "Accepted",
        "reason": "all required claims verified",
        "summary": {"claims_total": 2, "claims_verified": 2},
        "claims": [
            {"claim_id": "builds", "state": "Valid"},
            {"claim_id": "tests", "state": "Valid"}
        ],
        "assurance": "A2",
        "independent_actor": false,
        "identity_assurance": "SELF-DECLARED"
    });
    let candidate = Candidate {
        reference: "artifact:demo".into(),
        digest: "sha256:candidate-a".into(),
    };

    let assessment = uni::assessment_from_report(&report, &candidate).unwrap();

    assert_eq!(assessment.provider, "uni");
    assert_eq!(assessment.subject, "artifact:demo");
    assert_eq!(assessment.subject_digest, "sha256:candidate-a");
    assert!(assessment.assessment_ref.starts_with("uni-report:sha256:"));
}

#[test]
fn uni_non_accepted_report_never_becomes_an_accepted_assessment() {
    let report = json!({
        "intent": {"id": "release-demo"},
        "decision": "EvidenceRequired",
        "reason": "one claim is missing",
        "summary": {"claims_total": 1, "claims_verified": 0},
        "claims": [{"claim_id": "tests", "state": "Stale"}],
        "assurance": "A0"
    });

    let error = uni::assessment_from_report(
        &report,
        &Candidate {
            reference: "artifact:demo".into(),
            digest: "sha256:candidate-a".into(),
        },
    )
    .unwrap_err();

    assert_eq!(error.code(), "UNI_NOT_ACCEPTED");
}

#[test]
fn kollio_document_is_observed_without_becoming_kernel_owned() {
    let document = json!({
        "schemaVersion": 7,
        "documentId": "decision:launch",
        "revision": 12,
        "semanticRevision": 5,
        "content": {},
        "relationships": {},
        "decisions": {},
        "presentation": {}
    });

    let observation = kollio::observe_document(&document).unwrap();

    assert_eq!(observation.object.reference, "kollio:decision:launch");
    assert_eq!(observation.object.revision, 5);
    assert!(observation.object.digest.starts_with("sha256:"));
    assert_eq!(observation.canonical_owner, "kollio");
}

#[test]
fn kollio_impact_remains_a_reassessment_frontier_not_a_verdict() {
    let impact = json!({
        "id": "impact:1",
        "trigger": {
            "kind": "sourceRevisionSuperseded",
            "sourceID": "source:pricing",
            "revisionID": "source-revision:2",
            "reason": "supplier changed the deadline"
        },
        "readSet": {
            "sourceID": "source:pricing",
            "revisionIDs": ["source-revision:1"],
            "citationIDs": ["citation:1"],
            "visitedObjectIDs": ["object:plan"],
            "relationshipIDs": []
        },
        "proposedChanges": [{
            "objectID": "object:plan",
            "reason": "evidenceMoved",
            "path": ["object:plan"]
        }],
        "unaffectedRefs": [{
            "objectID": "object:other",
            "why": "citationStillCurrent"
        }],
        "wasTruncated": false,
        "createdAt": "2026-09-27T00:00:00Z"
    });

    let frontier = kollio::reassessment_frontier(&impact).unwrap();

    assert_eq!(frontier.reassess, vec!["object:plan"]);
    assert_eq!(frontier.unaffected, vec!["object:other"]);
    assert!(!frontier.truncated);
}
