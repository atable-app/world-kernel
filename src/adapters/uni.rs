use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{AcceptedAssessment, Candidate};

use super::{AdapterError, digest_json};

pub trait UniCommandRunner {
    /// Runs assurance and returns a normalized envelope of evidence already
    /// validated through UNI's versioned bundle transport.
    fn verify(&self, workspace: &Path, contract: &Path) -> Result<Value, UniCollectorError>;

    /// Returns the byte-stable report for the decision just produced.
    fn report(&self, workspace: &Path) -> Result<Value, UniCollectorError>;
}

#[derive(Debug, Clone)]
pub struct ProcessUniRunner {
    binary: PathBuf,
}

impl ProcessUniRunner {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }

    fn run_json(
        &self,
        workspace: &Path,
        operation: &str,
        trailing_argument: Option<&Path>,
    ) -> Result<Value, UniCollectorError> {
        let mut command = Command::new(&self.binary);
        command.current_dir(workspace).arg("--json").arg(operation);
        if let Some(argument) = trailing_argument {
            command.arg(argument);
        }
        let output = command.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(UniCollectorError::CommandFailed(format!(
                "{operation} exited with {}: {stderr}",
                output.status
            )));
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    fn run_bundle_command(
        &self,
        workspace: &Path,
        arguments: &[&std::ffi::OsStr],
        operation: &str,
    ) -> Result<Value, UniCollectorError> {
        let output = Command::new(&self.binary)
            .current_dir(workspace)
            .arg("--json")
            .args(arguments)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(UniCollectorError::CommandFailed(format!(
                "{operation} exited with {}: {stderr}",
                output.status
            )));
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }
}

impl UniCommandRunner for ProcessUniRunner {
    fn verify(&self, workspace: &Path, contract: &Path) -> Result<Value, UniCollectorError> {
        self.run_json(workspace, "verify", Some(contract))?;

        let temporary = tempfile::tempdir()?;
        let bundle_path = temporary.path().join("uni-bundle.jsonl");
        self.run_bundle_command(
            workspace,
            &[
                std::ffi::OsStr::new("bundle"),
                std::ffi::OsStr::new("export"),
                contract.as_os_str(),
                std::ffi::OsStr::new("--out"),
                bundle_path.as_os_str(),
            ],
            "bundle export",
        )?;
        let bundle_check = self.run_bundle_command(
            workspace,
            &[
                std::ffi::OsStr::new("bundle"),
                std::ffi::OsStr::new("verify"),
                bundle_path.as_os_str(),
            ],
            "bundle verify",
        )?;
        if bundle_check.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(UniCollectorError::BundleInvalid);
        }

        normalize_verified_bundle(&fs::read_to_string(bundle_path)?)
    }

    fn report(&self, workspace: &Path) -> Result<Value, UniCollectorError> {
        self.run_json(workspace, "report", None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectRequest {
    pub workspace: PathBuf,
    pub contract: PathBuf,
    pub candidate_path: PathBuf,
    pub candidate_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedAssessment {
    pub candidate: Candidate,
    pub assessment: AcceptedAssessment,
}

pub struct UniCollector<R> {
    runner: R,
}

impl<R> UniCollector<R>
where
    R: UniCommandRunner,
{
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    pub fn collect(
        &self,
        request: &CollectRequest,
    ) -> Result<CollectedAssessment, UniCollectorError> {
        let workspace = request.workspace.canonicalize()?;
        let candidate_path = request.candidate_path.canonicalize()?;
        let contract = request.contract.canonicalize()?;
        if !candidate_path.starts_with(&workspace) || !contract.starts_with(&workspace) {
            return Err(UniCollectorError::OutsideWorkspace);
        }

        let relative_candidate = candidate_path
            .strip_prefix(&workspace)
            .map_err(|_| UniCollectorError::OutsideWorkspace)?
            .to_string_lossy()
            .replace('\\', "/");
        let before = digest_file(&candidate_path)?;
        let verification = self.runner.verify(&workspace, &contract)?;
        let report = self.runner.report(&workspace)?;
        let after = digest_file(&candidate_path)?;
        if before != after {
            return Err(UniCollectorError::CandidateChanged { before, after });
        }
        require_candidate_evidence(&verification, &relative_candidate, &before)?;

        let candidate = Candidate {
            reference: request.candidate_ref.clone(),
            digest: before,
        };
        let mut assessment = assessment_from_report(&report, &candidate)?;
        let bound_record = serde_json::json!({
            "candidate": candidate,
            "report": report,
            "verification": verification,
        });
        assessment.assessment_ref = format!("uni-bound:{}", digest_json(&bound_record)?);
        Ok(CollectedAssessment {
            candidate,
            assessment,
        })
    }
}

#[derive(Debug, Error)]
pub enum UniCollectorError {
    #[error("candidate or contract is outside the declared workspace")]
    OutsideWorkspace,
    #[error("candidate changed during verification: {before} -> {after}")]
    CandidateChanged { before: String, after: String },
    #[error("valid UNI evidence does not cover candidate {path} at {digest}")]
    CandidateNotCovered { path: String, digest: String },
    #[error("UNI command failed: {0}")]
    CommandFailed(String),
    #[error("UNI did not validate its exported evidence bundle")]
    BundleInvalid,
    #[error("unsupported UNI evidence bundle version: {0}")]
    UnsupportedBundle(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON from UNI: {0}")]
    Json(#[from] serde_json::Error),
    #[error("adapter error: {0}")]
    Adapter(#[from] AdapterError),
}

impl UniCollectorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::OutsideWorkspace => "OUTSIDE_WORKSPACE",
            Self::CandidateChanged { .. } => "CANDIDATE_CHANGED_DURING_VERIFICATION",
            Self::CandidateNotCovered { .. } => "CANDIDATE_NOT_COVERED_BY_UNI",
            Self::CommandFailed(_) => "UNI_COMMAND_FAILED",
            Self::BundleInvalid => "UNI_BUNDLE_INVALID",
            Self::UnsupportedBundle(_) => "UNI_BUNDLE_UNSUPPORTED",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "UNI_INVALID_JSON",
            Self::Adapter(error) => error.code(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct UniVerification {
    #[serde(default)]
    evidence: Vec<UniEvidence>,
}

#[derive(Debug, Deserialize)]
struct UniEvidence {
    state: String,
    #[serde(default)]
    artifact_files: BTreeMap<String, String>,
}

fn require_candidate_evidence(
    verification: &Value,
    relative_candidate: &str,
    candidate_digest: &str,
) -> Result<(), UniCollectorError> {
    let parsed: UniVerification = serde_json::from_value(verification.clone())?;
    let raw_digest = candidate_digest
        .strip_prefix("sha256:")
        .unwrap_or(candidate_digest);
    let covered = parsed.evidence.iter().any(|evidence| {
        evidence.state == "Valid"
            && evidence
                .artifact_files
                .get(relative_candidate)
                .map(|digest| digest.strip_prefix("sha256:").unwrap_or(digest) == raw_digest)
                .unwrap_or(false)
    });
    if covered {
        return Ok(());
    }
    Err(UniCollectorError::CandidateNotCovered {
        path: relative_candidate.to_owned(),
        digest: candidate_digest.to_owned(),
    })
}

fn normalize_verified_bundle(bundle: &str) -> Result<Value, UniCollectorError> {
    let mut lines = bundle.lines().filter(|line| !line.trim().is_empty());
    let header: Value =
        serde_json::from_str(lines.next().ok_or(UniCollectorError::BundleInvalid)?)?;
    let version = header
        .pointer("/body/version")
        .and_then(Value::as_str)
        .ok_or(UniCollectorError::BundleInvalid)?;
    if version != "uni-bundle-0.1" {
        return Err(UniCollectorError::UnsupportedBundle(version.to_owned()));
    }

    let mut evidence = Vec::new();
    for line in lines {
        let record: Value = serde_json::from_str(line)?;
        if record.get("kind").and_then(Value::as_str) == Some("evidence") {
            evidence.push(
                record
                    .get("body")
                    .cloned()
                    .ok_or(UniCollectorError::BundleInvalid)?,
            );
        }
    }
    Ok(serde_json::json!({
        "bundleVersion": version,
        "evidence": evidence,
    }))
}

fn digest_file(path: &Path) -> Result<String, std::io::Error> {
    let bytes = fs::read(path)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Reads the candidate that is about to be proposed to the Kernel.
///
/// Calling this after collection makes a substitution visible as a different
/// candidate. The Kernel then compares it with the collected assessment.
pub fn candidate_from_file(
    path: impl AsRef<Path>,
    reference: impl Into<String>,
) -> Result<Candidate, UniCollectorError> {
    Ok(Candidate {
        reference: reference.into(),
        digest: digest_file(path.as_ref())?,
    })
}

#[derive(Debug, Deserialize)]
struct UniReport {
    decision: String,
}

/// Low-level conversion of `uni --json report` into a Kernel assessment.
///
/// The report does not identify the candidate artifact itself. The caller must
/// already have established exact evidence binding. Normal integrations should
/// use `UniCollector`, which verifies that binding through a UNI bundle. The
/// Kernel subsequently checks the exact reference and digest at admission.
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
