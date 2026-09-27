use serde::Deserialize;
use serde_json::Value;

use crate::{AcceptedAssessment, Candidate};

use super::{AdapterError, digest_json};

#[derive(Debug, Deserialize)]
struct UniReport {
    decision: String,
}

/// Converts the stable output of `uni --json report` into a Kernel assessment.
///
/// The report does not identify the candidate artifact itself. The caller must
/// bind the report to a candidate collected in the same trusted operation. The
/// Kernel subsequently checks that exact reference and digest at admission.
pub fn assessment_from_report(
    report: &Value,
    exact_candidate: &Candidate,
) -> Result<AcceptedAssessment, AdapterError> {
    let parsed: UniReport = serde_json::from_value(report.clone())?;
    if parsed.decision != "Accepted" {
        return Err(AdapterError::UniNotAccepted(parsed.decision));
    }
    let report_digest = digest_json(report)?;
    Ok(AcceptedAssessment {
        provider: "uni".into(),
        subject: exact_candidate.reference.clone(),
        subject_digest: exact_candidate.digest.clone(),
        assessment_ref: format!("uni-report:{report_digest}"),
    })
}
