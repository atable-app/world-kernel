use serde::Deserialize;
use serde_json::Value;

use crate::ObjectRevision;

use super::{AdapterError, digest_json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentObservation {
    pub object: ObjectRevision,
    pub canonical_owner: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KollioDocumentHeader {
    document_id: String,
    semantic_revision: u64,
}

/// Observes a Kollio document without claiming ownership of its state.
pub fn observe_document(document: &Value) -> Result<DocumentObservation, AdapterError> {
    let header: KollioDocumentHeader = serde_json::from_value(document.clone())?;
    Ok(DocumentObservation {
        object: ObjectRevision {
            reference: format!("kollio:{}", header.document_id),
            revision: header.semantic_revision,
            digest: digest_json(document)?,
        },
        canonical_owner: "kollio",
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReassessmentFrontier {
    pub reassess: Vec<String>,
    pub unaffected: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KollioImpact {
    proposed_changes: Vec<ImpactedObject>,
    unaffected_refs: Vec<UnaffectedObject>,
    was_truncated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImpactedObject {
    #[serde(rename = "objectID")]
    object_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnaffectedObject {
    #[serde(rename = "objectID")]
    object_id: String,
}

/// Preserves Kollio's distinction between impact and verdict.
pub fn reassessment_frontier(impact: &Value) -> Result<ReassessmentFrontier, AdapterError> {
    let impact: KollioImpact = serde_json::from_value(impact.clone())?;
    Ok(ReassessmentFrontier {
        reassess: impact
            .proposed_changes
            .into_iter()
            .map(|object| object.object_id)
            .collect(),
        unaffected: impact
            .unaffected_refs
            .into_iter()
            .map(|object| object.object_id)
            .collect(),
        truncated: impact.was_truncated,
    })
}
