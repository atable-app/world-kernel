//! System A: a solid application-specific SQLite admission gate.
//!
//! It receives the same information as the portable envelope and must reach the
//! same decisions. It deliberately shares no Kernel implementation code: its own
//! request type, its own tables, its own transaction order, its own authority
//! check. A weaker A would make the benchmark meaningless, so every check the
//! Kernel performs is implemented here in application terms.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use world_kernel::ObjectRevision;

use super::admission_case::BenchmarkCode;
use super::assurance::AssuranceOutcome;

/// The application's own request vocabulary. Nothing here is the portable
/// envelope; the application is free to name things its own way.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PublishRequest {
    pub request_version: String,
    pub scope: String,
    pub request_key: String,
    pub principal: String,
    pub operation: String,
    pub target: String,
    pub subject: String,
    pub subject_digest: String,
    pub observed_revision: u64,
    pub declared_reads: Vec<DeclaredRead>,
    pub declared_view: DeclaredView,
    pub verifier_claims: Vec<VerifierClaim>,
    pub write_intents: Vec<WriteIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeclaredRead {
    pub subject: String,
    pub revision: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeclaredView {
    pub profile: String,
    pub covers: BTreeSet<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct VerifierClaim {
    pub verifier: String,
    pub subject: String,
    pub subject_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WriteIntent {
    pub subject: String,
    pub expect_revision: Option<u64>,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    Applied { revision: u64 },
    Refused(BenchmarkCode),
}

#[derive(Debug)]
pub enum GateError {
    Storage(rusqlite::Error),
    Interrupted(String),
    Serialization(serde_json::Error),
}

impl From<rusqlite::Error> for GateError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<serde_json::Error> for GateError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

/// The application's own authority list.
pub struct AppAuthority {
    grants: BTreeMap<String, BTreeSet<String>>,
}

impl AppAuthority {
    pub fn new(grants: BTreeMap<String, BTreeSet<String>>) -> Self {
        Self { grants }
    }

    fn permits(&self, principal: &str, operation: &str) -> bool {
        self.grants
            .get(principal)
            .is_some_and(|operations| operations.contains(operation))
    }
}

pub const REQUEST_VERSION: &str = "publish-request/2";

pub struct BaselineA {
    connection: Connection,
    scope: String,
    trusted_verifiers: BTreeSet<String>,
}

impl BaselineA {
    pub fn create(
        path: impl AsRef<Path>,
        scope: &str,
        trusted_verifiers: BTreeSet<String>,
        objects: &[ObjectRevision],
    ) -> Result<Self, GateError> {
        let mut connection = Connection::open(path)?;
        Self::prepare(&connection)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO app_scopes(scope_id, revision, trusted_verifiers) VALUES (?1, 0, ?2)",
            params![scope, serde_json::to_string(&trusted_verifiers)?],
        )?;
        for object in objects {
            transaction.execute(
                "INSERT INTO app_objects(scope_id, subject, revision, digest) VALUES (?1, ?2, ?3, ?4)",
                params![scope, object.reference, object.revision, object.digest],
            )?;
        }
        transaction.execute(
            "INSERT INTO app_events(scope_id, revision, kind, body) VALUES (?1, 0, 'opened', '{}')",
            params![scope],
        )?;
        transaction.commit()?;
        Ok(Self {
            connection,
            scope: scope.to_owned(),
            trusted_verifiers,
        })
    }

    /// Reopens an existing scope, as a restarted process would.
    pub fn reopen(path: impl AsRef<Path>, scope: &str) -> Result<Self, GateError> {
        let connection = Connection::open(path)?;
        Self::prepare(&connection)?;
        let trusted: Option<String> = connection
            .query_row(
                "SELECT trusted_verifiers FROM app_scopes WHERE scope_id = ?1",
                [scope],
                |row| row.get(0),
            )
            .optional()?;
        let trusted_verifiers = trusted
            .and_then(|json| serde_json::from_str(&json).ok())
            .ok_or_else(|| GateError::Interrupted("scope is not open".into()))?;
        Ok(Self {
            connection,
            scope: scope.to_owned(),
            trusted_verifiers,
        })
    }

    pub fn revision(&self) -> Result<u64, GateError> {
        Ok(self.connection.query_row(
            "SELECT revision FROM app_scopes WHERE scope_id = ?1",
            [&self.scope],
            |row| row.get(0),
        )?)
    }

    /// The application's admission gate, in its own order.
    pub fn apply(
        &mut self,
        request: &PublishRequest,
        assurance: &AssuranceOutcome,
        authority: &AppAuthority,
    ) -> Result<GateOutcome, GateError> {
        if request.request_version != REQUEST_VERSION {
            return Ok(GateOutcome::Refused(BenchmarkCode::UnsupportedSchema));
        }
        if request.scope != self.scope {
            return Ok(GateOutcome::Refused(BenchmarkCode::WrongWorld));
        }
        if let Some(code) = assurance.refusal_code() {
            return Ok(GateOutcome::Refused(code_from_assurance(code)));
        }
        let request_digest = digest_of(&serde_json::to_string(request)?);
        let prior: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT request_digest, outcome FROM app_requests WHERE scope_id = ?1 AND request_key = ?2",
                params![self.scope, request.request_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((stored, outcome)) = prior {
            if stored == request_digest {
                let applied: AppliedReceipt = serde_json::from_str(&outcome)?;
                return Ok(GateOutcome::Applied {
                    revision: applied.revision,
                });
            }
            return Ok(GateOutcome::Refused(BenchmarkCode::IdempotencyConflict));
        }
        if !authority.permits(&request.principal, &request.operation) {
            return Ok(GateOutcome::Refused(BenchmarkCode::Unauthorized));
        }
        if request.declared_view.truncated
            || !request.declared_view.covers.contains(&request.operation)
        {
            return Ok(GateOutcome::Refused(BenchmarkCode::IncompleteView));
        }
        let exact: Vec<&VerifierClaim> = request
            .verifier_claims
            .iter()
            .filter(|claim| {
                claim.subject == request.subject && claim.subject_digest == request.subject_digest
            })
            .collect();
        if exact.is_empty() {
            return Ok(GateOutcome::Refused(BenchmarkCode::AssessmentMismatch));
        }
        // A competent gate also requires the claim to cover the bytes the
        // verifier actually verified, not only the digest the request declares.
        if let AssuranceOutcome::Bound {
            subject,
            subject_digest,
            ..
        } = assurance
            && !exact
                .iter()
                .any(|claim| claim.subject == *subject && claim.subject_digest == *subject_digest)
        {
            return Ok(GateOutcome::Refused(BenchmarkCode::AssessmentMismatch));
        }
        if !exact
            .iter()
            .any(|claim| self.trusted_verifiers.contains(&claim.verifier))
        {
            return Ok(GateOutcome::Refused(
                BenchmarkCode::UntrustedAssessmentProvider,
            ));
        }
        let publishes_subject = request.write_intents.iter().any(|write| {
            write.subject == request.subject && write.digest == request.subject_digest
        });
        if !publishes_subject || request.target != request.subject {
            return Ok(GateOutcome::Refused(BenchmarkCode::CandidateMismatch));
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: u64 = transaction.query_row(
            "SELECT revision FROM app_scopes WHERE scope_id = ?1",
            [&self.scope],
            |row| row.get(0),
        )?;
        if current != request.observed_revision {
            return Ok(GateOutcome::Refused(BenchmarkCode::StaleWorld));
        }
        for read in &request.declared_reads {
            let found: Option<(u64, String)> = transaction
                .query_row(
                    "SELECT revision, digest FROM app_objects WHERE scope_id = ?1 AND subject = ?2",
                    params![self.scope, read.subject],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if found != Some((read.revision, read.digest.clone())) {
                return Ok(GateOutcome::Refused(BenchmarkCode::StaleDependency));
            }
        }
        for write in &request.write_intents {
            let found: Option<u64> = transaction
                .query_row(
                    "SELECT revision FROM app_objects WHERE scope_id = ?1 AND subject = ?2",
                    params![self.scope, write.subject],
                    |row| row.get(0),
                )
                .optional()?;
            if found != write.expect_revision {
                return Ok(GateOutcome::Refused(BenchmarkCode::PatchConflict));
            }
        }

        let next = current + 1;
        for write in &request.write_intents {
            let next_object_revision = write.expect_revision.map_or(1, |revision| revision + 1);
            transaction.execute(
                "INSERT INTO app_objects(scope_id, subject, revision, digest) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(scope_id, subject) DO UPDATE SET revision = excluded.revision, digest = excluded.digest",
                params![self.scope, write.subject, next_object_revision, write.digest],
            )?;
        }
        transaction.execute(
            "UPDATE app_scopes SET revision = ?2 WHERE scope_id = ?1",
            params![self.scope, next],
        )?;
        transaction.execute(
            "INSERT INTO app_events(scope_id, revision, kind, body) VALUES (?1, ?2, 'published', ?3)",
            params![
                self.scope,
                next,
                serde_json::to_string(&serde_json::json!({
                    "principal": request.principal,
                    "subject": request.subject,
                    "digest": request.subject_digest,
                }))?
            ],
        )?;
        let receipt = AppliedReceipt {
            revision: next,
            subject_digest: request.subject_digest.clone(),
        };
        transaction.execute(
            "INSERT INTO app_requests(scope_id, request_key, request_digest, outcome) VALUES (?1, ?2, ?3, ?4)",
            params![
                self.scope,
                request.request_key,
                request_digest,
                serde_json::to_string(&receipt)?
            ],
        )?;
        transaction.commit()?;
        Ok(GateOutcome::Applied { revision: next })
    }

    fn prepare(connection: &Connection) -> Result<(), GateError> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_scopes (
               scope_id TEXT PRIMARY KEY,
               revision INTEGER NOT NULL,
               trusted_verifiers TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS app_objects (
               scope_id TEXT NOT NULL,
               subject TEXT NOT NULL,
               revision INTEGER NOT NULL,
               digest TEXT NOT NULL,
               PRIMARY KEY(scope_id, subject)
             );
             CREATE TABLE IF NOT EXISTS app_events (
               sequence INTEGER PRIMARY KEY AUTOINCREMENT,
               scope_id TEXT NOT NULL,
               revision INTEGER NOT NULL,
               kind TEXT NOT NULL,
               body TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS app_requests (
               scope_id TEXT NOT NULL,
               request_key TEXT NOT NULL,
               request_digest TEXT NOT NULL,
               outcome TEXT NOT NULL,
               PRIMARY KEY(scope_id, request_key)
             );",
        )?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AppliedReceipt {
    revision: u64,
    subject_digest: String,
}

pub fn code_from_assurance(code: &str) -> BenchmarkCode {
    match code {
        "UNI_NOT_ACCEPTED" => BenchmarkCode::UniNotAccepted,
        "CANDIDATE_NOT_COVERED_BY_UNI" => BenchmarkCode::CandidateNotCoveredByUni,
        "CANDIDATE_CHANGED_DURING_VERIFICATION" => {
            BenchmarkCode::CandidateChangedDuringVerification
        }
        "UNI_COMMAND_FAILED" => BenchmarkCode::UniCommandFailed,
        "UNI_BUNDLE_INVALID" => BenchmarkCode::UniBundleInvalid,
        "UNI_BUNDLE_UNSUPPORTED" => BenchmarkCode::UniBundleUnsupported,
        other => panic!("no benchmark code for the assurance refusal {other}"),
    }
}

fn digest_of(bytes: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes.as_bytes()))
}
