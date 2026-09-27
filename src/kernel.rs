use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::continuation::{
    AssuranceTransport, BodyStatus, ContinuationPackage, Head, Origin, RecordedAdmission,
    RecordedTransition, ResourceRecord,
};
use crate::model::{
    EXPERIMENTAL_SCHEMA, GroundedChange, KernelError, ObjectState, Patch, Receipt, Rejection,
    RejectionCode, SubmissionOutcome, WorldBootstrap, WorldSnapshot, digest_of_bytes,
};

pub trait AuthoritySource {
    fn is_allowed(&self, world: &str, actor: &str, permission: &str) -> bool;
}

#[derive(Debug, Clone, Default)]
pub struct StaticAuthority {
    grants: BTreeMap<String, BTreeSet<String>>,
}

impl StaticAuthority {
    pub fn new(grants: impl IntoIterator<Item = (String, BTreeSet<String>)>) -> Self {
        Self {
            grants: grants.into_iter().collect(),
        }
    }
}

impl AuthoritySource for StaticAuthority {
    fn is_allowed(&self, _world: &str, actor: &str, permission: &str) -> bool {
        self.grants
            .get(actor)
            .is_some_and(|permissions| permissions.contains(permission))
    }
}

pub struct Kernel {
    connection: Connection,
    world: String,
    trusted_assurance_providers: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenesisEvent {
    world: String,
    objects: BTreeMap<String, ObjectState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangeCommittedEvent {
    proposal_id: String,
    candidate_digest: String,
    patches: Vec<Patch>,
}

impl Kernel {
    pub fn open(path: impl AsRef<Path>, world: impl Into<String>) -> Result<Self, KernelError> {
        let connection = Connection::open(path)?;
        Self::prepare_schema(&connection)?;
        let world = world.into();
        let trusted_json: Option<String> = connection
            .query_row(
                "SELECT trusted_assurance_providers FROM worlds WHERE world_id = ?1",
                [&world],
                |row| row.get(0),
            )
            .optional()?;
        let trusted_assurance_providers = trusted_json
            .map(|json| serde_json::from_str(&json))
            .transpose()?
            .ok_or(KernelError::WorldNotFound)?;
        Ok(Self {
            connection,
            world,
            trusted_assurance_providers,
        })
    }

    pub fn create(path: impl AsRef<Path>, bootstrap: WorldBootstrap) -> Result<Self, KernelError> {
        let mut connection = Connection::open(path)?;
        Self::prepare_schema(&connection)?;

        let exists = connection
            .query_row(
                "SELECT 1 FROM worlds WHERE world_id = ?1",
                [&bootstrap.world],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if exists {
            return Err(KernelError::WorldAlreadyExists(bootstrap.world));
        }

        let trusted_assurance_providers = bootstrap.trusted_assurance_providers;
        let objects: BTreeMap<String, ObjectState> = bootstrap
            .objects
            .into_iter()
            .map(|object| {
                (
                    object.reference,
                    ObjectState {
                        revision: object.revision,
                        digest: object.digest,
                    },
                )
            })
            .collect();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO worlds(world_id, revision, trusted_assurance_providers) VALUES (?1, 0, ?2)",
            params![
                bootstrap.world,
                serde_json::to_string(&trusted_assurance_providers)?
            ],
        )?;
        for (reference, state) in &objects {
            transaction.execute(
                "INSERT INTO objects(world_id, object_ref, revision, digest) VALUES (?1, ?2, ?3, ?4)",
                params![bootstrap.world, reference, state.revision, state.digest],
            )?;
        }
        let genesis = GenesisEvent {
            world: bootstrap.world.clone(),
            objects,
        };
        transaction.execute(
            "INSERT INTO events(world_id, world_revision, event_type, payload) VALUES (?1, 0, 'genesis', ?2)",
            params![bootstrap.world, serde_json::to_string(&genesis)?],
        )?;
        transaction.commit()?;

        Ok(Self {
            connection,
            world: bootstrap.world,
            trusted_assurance_providers,
        })
    }

    pub fn submit(
        &mut self,
        change: &GroundedChange,
        authority: &dyn AuthoritySource,
    ) -> Result<SubmissionOutcome, KernelError> {
        if change.schema != EXPERIMENTAL_SCHEMA {
            return Ok(rejected(
                RejectionCode::UnsupportedSchema,
                format!("unsupported schema {}", change.schema),
            ));
        }
        if change.world != self.world {
            return Ok(rejected(
                RejectionCode::WrongWorld,
                format!(
                    "proposal targets {}, kernel owns {}",
                    change.world, self.world
                ),
            ));
        }
        let proposal_digest = hex_digest(&serde_json::to_vec(change)?);
        let prior: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT proposal_digest, outcome FROM submissions WHERE world_id = ?1 AND idempotency_key = ?2",
                params![self.world, change.idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((stored_digest, stored_outcome)) = prior {
            if stored_digest == proposal_digest {
                return Ok(serde_json::from_str(&stored_outcome)?);
            }
            return Ok(rejected(
                RejectionCode::IdempotencyConflict,
                format!(
                    "idempotency key {} was already used for another proposal",
                    change.idempotency_key
                ),
            ));
        }
        if !authority.is_allowed(&self.world, &change.actor, &change.intent.kind) {
            return Ok(rejected(
                RejectionCode::Unauthorized,
                format!("{} may not {}", change.actor, change.intent.kind),
            ));
        }
        if change.coverage.truncated || !change.coverage.complete_for.contains(&change.intent.kind)
        {
            return Ok(rejected(
                RejectionCode::IncompleteView,
                format!("coverage does not include {}", change.intent.kind),
            ));
        }
        let exact_assessments: Vec<_> = change
            .assessments
            .iter()
            .filter(|assessment| {
                assessment.subject == change.candidate.reference
                    && assessment.subject_digest == change.candidate.digest
            })
            .collect();
        if exact_assessments.is_empty() {
            return Ok(rejected(
                RejectionCode::AssessmentMismatch,
                "no accepted assessment covers the exact candidate".into(),
            ));
        }
        if !exact_assessments.iter().any(|assessment| {
            self.trusted_assurance_providers
                .contains(&assessment.provider)
        }) {
            return Ok(rejected(
                RejectionCode::UntrustedAssessmentProvider,
                "the exact candidate is covered only by untrusted assurance providers".into(),
            ));
        }
        let candidate_is_published = change.patches.iter().any(|patch| match patch {
            Patch::PutObject {
                reference, digest, ..
            } => reference == &change.candidate.reference && digest == &change.candidate.digest,
        });
        if !candidate_is_published || change.intent.target != change.candidate.reference {
            return Ok(rejected(
                RejectionCode::CandidateMismatch,
                "the patch and intent do not publish the assessed candidate".into(),
            ));
        }

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_revision: u64 = transaction.query_row(
            "SELECT revision FROM worlds WHERE world_id = ?1",
            [&self.world],
            |row| row.get(0),
        )?;
        if current_revision != change.base_revision {
            return Ok(rejected(
                RejectionCode::StaleWorld,
                format!(
                    "expected world revision {}, current revision is {}",
                    change.base_revision, current_revision
                ),
            ));
        }

        for read in &change.reads {
            let current: Option<(u64, String)> = transaction
                .query_row(
                    "SELECT revision, digest FROM objects WHERE world_id = ?1 AND object_ref = ?2",
                    params![self.world, read.reference],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if current != Some((read.revision, read.digest.clone())) {
                return Ok(rejected(
                    RejectionCode::StaleDependency,
                    format!("dependency {} no longer matches", read.reference),
                ));
            }
        }

        for patch in &change.patches {
            match patch {
                Patch::PutObject {
                    reference,
                    expected_revision,
                    ..
                } => {
                    let current: Option<u64> = transaction
                        .query_row(
                            "SELECT revision FROM objects WHERE world_id = ?1 AND object_ref = ?2",
                            params![self.world, reference],
                            |row| row.get(0),
                        )
                        .optional()?;
                    if current != *expected_revision {
                        return Ok(rejected(
                            RejectionCode::PatchConflict,
                            format!("object {reference} has revision {current:?}"),
                        ));
                    }
                }
            }
        }

        let next_world_revision = current_revision + 1;
        for patch in &change.patches {
            match patch {
                Patch::PutObject {
                    reference,
                    expected_revision,
                    digest,
                } => {
                    let next_object_revision = expected_revision.map_or(1, |revision| revision + 1);
                    transaction.execute(
                        "INSERT INTO objects(world_id, object_ref, revision, digest) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(world_id, object_ref) DO UPDATE SET revision = excluded.revision, digest = excluded.digest",
                        params![self.world, reference, next_object_revision, digest],
                    )?;
                }
            }
        }
        transaction.execute(
            "UPDATE worlds SET revision = ?2 WHERE world_id = ?1",
            params![self.world, next_world_revision],
        )?;
        let event = ChangeCommittedEvent {
            proposal_id: change.proposal_id.clone(),
            candidate_digest: change.candidate.digest.clone(),
            patches: change.patches.clone(),
        };
        transaction.execute(
            "INSERT INTO events(world_id, world_revision, event_type, payload) VALUES (?1, ?2, 'change_committed', ?3)",
            params![self.world, next_world_revision, serde_json::to_string(&event)?],
        )?;
        let event_sequence = transaction.last_insert_rowid() as u64;
        let outcome = SubmissionOutcome::Committed(Receipt {
            proposal_id: change.proposal_id.clone(),
            world_revision: next_world_revision,
            event_sequence,
            candidate_digest: change.candidate.digest.clone(),
        });
        transaction.execute(
            "INSERT INTO submissions(world_id, idempotency_key, proposal_digest, outcome) VALUES (?1, ?2, ?3, ?4)",
            params![
                self.world,
                change.idempotency_key,
                proposal_digest,
                serde_json::to_string(&outcome)?
            ],
        )?;
        // The declared change travels with its outcome, in the same transaction,
        // so a continuation can answer what was admitted, under which rules and on
        // which evidence. A recorded event alone cannot.
        transaction.execute(
            "INSERT INTO admissions(world_id, proposal_id, idempotency_key, change_json, outcome_json, event_sequence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                self.world,
                change.proposal_id,
                change.idempotency_key,
                serde_json::to_string(change)?,
                serde_json::to_string(&outcome)?,
                event_sequence as i64,
            ],
        )?;
        transaction.commit()?;

        Ok(outcome)
    }

    pub fn snapshot(&self) -> Result<WorldSnapshot, KernelError> {
        let revision = self
            .connection
            .query_row(
                "SELECT revision FROM worlds WHERE world_id = ?1",
                [&self.world],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(KernelError::WorldNotFound)?;
        let mut statement = self.connection.prepare(
            "SELECT object_ref, revision, digest FROM objects WHERE world_id = ?1 ORDER BY object_ref",
        )?;
        let objects = statement
            .query_map([&self.world], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ObjectState {
                        revision: row.get(1)?,
                        digest: row.get(2)?,
                    },
                ))
            })?
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(WorldSnapshot {
            world: self.world.clone(),
            revision,
            objects,
        })
    }

    pub fn replay(&self) -> Result<WorldSnapshot, KernelError> {
        let transitions = self.recorded_history()?;
        reduce_transitions(&transitions, &self.world).map_err(KernelError::CorruptState)
    }

    /// The recorded history, decoded into typed transitions.
    ///
    /// An unknown event type is a corrupt-state error, never an invitation to
    /// guess at semantics.
    fn recorded_history(&self) -> Result<Vec<RecordedTransition>, KernelError> {
        let mut statement = self.connection.prepare(
            "SELECT event_type, payload FROM events WHERE world_id = ?1 ORDER BY sequence",
        )?;
        let events = statement
            .query_map([&self.world], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        decode_history(&events)
    }

    /// Writes a continuation package describing this World as recorded.
    ///
    /// The package is derived from the event log and the recorded admissions, so
    /// two exports of an unchanged World are byte-identical. It carries no trust
    /// configuration and no secret: a consumer receives its expected head over a
    /// separate channel.
    pub fn export_continuation(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<ContinuationPackage, KernelError> {
        let path = path.as_ref();
        let snapshot = self.snapshot()?;
        let transitions = self.recorded_history()?;

        let mut admissions_statement = self.connection.prepare(
            "SELECT change_json, outcome_json FROM admissions WHERE world_id = ?1 ORDER BY proposal_id",
        )?;
        let admissions = admissions_statement
            .query_map([&self.world], |row| {
                let change: GroundedChange = serde_json::from_str(&row.get::<_, String>(0)?)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                let outcome: SubmissionOutcome = serde_json::from_str(&row.get::<_, String>(1)?)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(RecordedAdmission {
                    proposal_id: change.proposal_id.clone(),
                    idempotency_key: change.idempotency_key.clone(),
                    actor: change.actor.clone(),
                    base_revision: change.base_revision,
                    intent: change.intent.clone(),
                    candidate: change.candidate.clone(),
                    reads: change.reads.clone(),
                    coverage: change.coverage.clone(),
                    assessments: change.assessments.clone(),
                    patches: change.patches.clone(),
                    outcome,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut references: Vec<String> = admissions
            .iter()
            .flat_map(|admission| {
                admission
                    .assessments
                    .iter()
                    .map(|assessment| assessment.assessment_ref.clone())
            })
            .collect();
        references.sort();
        references.dedup();

        let resources = snapshot
            .objects
            .iter()
            .map(|(reference, state)| ResourceRecord {
                reference: reference.clone(),
                revision: state.revision,
                digest: state.digest.clone(),
                // The Kernel never stored object bytes, so a body is always
                // declared absent rather than implied present.
                body_status: BodyStatus::Absent,
            })
            .collect::<Vec<_>>();

        let mut package = ContinuationPackage {
            schema: crate::continuation::PACKAGE_SCHEMA.to_owned(),
            reducer: crate::continuation::RECORDED_HISTORY_REDUCER.to_owned(),
            world: self.world.clone(),
            origin: Origin {
                kind: "genesis".to_owned(),
                world_revision: 0,
                transition_count: transitions.len(),
            },
            head: Head {
                world_revision: snapshot.revision,
                event_sequence: transitions.len() as u64,
                head_digest: String::new(),
            },
            transitions,
            admissions,
            resources,
            assurance: AssuranceTransport {
                kind: "uni-bundle-0.1".to_owned(),
                verification: "reference-only".to_owned(),
                bytes_available: false,
                references,
            },
            declarations: vec![
                "no trust configuration travels in this package; the expected head arrives out of band".to_owned(),
                "object bodies are not included; a digest without accessible content does not reconstruct an artifact".to_owned(),
                "assurance references are historical assertions by their issuer and are not re-verified here".to_owned(),
            ],
        };
        package.head.head_digest = crate::continuation::head_digest(&package);

        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_vec(&package)?)?;
        Ok(package)
    }

    /// Writes recorded transitions into this World, in order, in one transaction.
    ///
    /// This is historical reconstruction, not admission: no authority, no assurance
    /// and no current context is consulted, because none of them were required for
    /// the transitions to have been recorded. The projection is maintained by the
    /// same reduction `replay` uses, so the two cannot drift, and the World's own
    /// trusted provider set is left exactly as its owner configured it.
    pub fn import_history(
        &mut self,
        transitions: &[RecordedTransition],
    ) -> Result<(), KernelError> {
        let projected =
            reduce_transitions(transitions, &self.world).map_err(KernelError::CorruptState)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // The trusted provider set belongs to whoever opened this World. A
        // reconstruction advances its history and projection and touches nothing
        // else: overwriting the trust configuration here would let a package
        // influence who is believed.
        transaction.execute(
            "UPDATE worlds SET revision = ?2 WHERE world_id = ?1",
            params![self.world, projected.revision],
        )?;
        transaction.execute("DELETE FROM events WHERE world_id = ?1", [&self.world])?;
        transaction.execute("DELETE FROM objects WHERE world_id = ?1", [&self.world])?;
        for (index, transition) in transitions.iter().enumerate() {
            let (event_type, payload, revision) = match transition {
                RecordedTransition::Genesis { world, objects } => (
                    "genesis",
                    serde_json::to_string(&GenesisEvent {
                        world: world.clone(),
                        objects: objects.clone(),
                    })?,
                    0u64,
                ),
                RecordedTransition::ChangeCommitted {
                    proposal_id,
                    candidate_digest,
                    patches,
                } => (
                    "change_committed",
                    serde_json::to_string(&ChangeCommittedEvent {
                        proposal_id: proposal_id.clone(),
                        candidate_digest: candidate_digest.clone(),
                        patches: patches.clone(),
                    })?,
                    (index + 1) as u64,
                ),
            };
            transaction.execute(
                "INSERT INTO events(world_id, world_revision, event_type, payload) VALUES (?1, ?2, ?3, ?4)",
                params![self.world, revision, event_type, payload],
            )?;
        }
        for (reference, state) in &projected.objects {
            transaction.execute(
                "INSERT INTO objects(world_id, object_ref, revision, digest) VALUES (?1, ?2, ?3, ?4)",
                params![self.world, reference, state.revision, state.digest],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn prepare_schema(connection: &Connection) -> Result<(), KernelError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS worlds (
               world_id TEXT PRIMARY KEY,
               revision INTEGER NOT NULL,
               trusted_assurance_providers TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS objects (
               world_id TEXT NOT NULL,
               object_ref TEXT NOT NULL,
               revision INTEGER NOT NULL,
               digest TEXT NOT NULL,
               PRIMARY KEY(world_id, object_ref),
               FOREIGN KEY(world_id) REFERENCES worlds(world_id)
             );
             CREATE TABLE IF NOT EXISTS events (
               sequence INTEGER PRIMARY KEY AUTOINCREMENT,
               world_id TEXT NOT NULL,
               world_revision INTEGER NOT NULL,
               event_type TEXT NOT NULL,
               payload TEXT NOT NULL,
               FOREIGN KEY(world_id) REFERENCES worlds(world_id)
             );
             CREATE TABLE IF NOT EXISTS admissions (
               world_id TEXT NOT NULL,
               proposal_id TEXT NOT NULL,
               idempotency_key TEXT NOT NULL,
               change_json TEXT NOT NULL,
               outcome_json TEXT NOT NULL,
               event_sequence INTEGER NOT NULL,
               PRIMARY KEY(world_id, proposal_id),
               FOREIGN KEY(world_id) REFERENCES worlds(world_id)
             );
             CREATE TABLE IF NOT EXISTS submissions (
               world_id TEXT NOT NULL,
               idempotency_key TEXT NOT NULL,
               proposal_digest TEXT NOT NULL,
               outcome TEXT NOT NULL,
               PRIMARY KEY(world_id, idempotency_key),
               FOREIGN KEY(world_id) REFERENCES worlds(world_id)
             );",
        )?;
        Ok(())
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    digest_of_bytes(bytes)
}

/// Decodes recorded event rows into typed transitions.
///
/// An unknown event type is a corrupt-state error. It is never an invitation to
/// guess at semantics, and it is never skipped.
fn decode_history(events: &[(String, String)]) -> Result<Vec<RecordedTransition>, KernelError> {
    let mut transitions = Vec::with_capacity(events.len());
    for (event_type, payload) in events {
        match event_type.as_str() {
            "genesis" => {
                let genesis: GenesisEvent = serde_json::from_str(payload)?;
                transitions.push(RecordedTransition::Genesis {
                    world: genesis.world,
                    objects: genesis.objects,
                });
            }
            "change_committed" => {
                let event: ChangeCommittedEvent = serde_json::from_str(payload)?;
                transitions.push(RecordedTransition::ChangeCommitted {
                    proposal_id: event.proposal_id,
                    candidate_digest: event.candidate_digest,
                    patches: event.patches,
                });
            }
            other => {
                return Err(KernelError::CorruptState(format!(
                    "unknown event type {other}"
                )));
            }
        }
    }
    Ok(transitions)
}

/// The single reduction used by both `replay` and a continuation reconstruction,
/// so a rebuilt projection and a replayed one cannot drift apart.
fn reduce_transitions(
    transitions: &[RecordedTransition],
    world: &str,
) -> Result<WorldSnapshot, String> {
    crate::continuation::reduce(transitions, world).map_err(|error| error.to_string())
}

fn rejected(code: RejectionCode, detail: String) -> SubmissionOutcome {
    SubmissionOutcome::Rejected(Rejection { code, detail })
}
