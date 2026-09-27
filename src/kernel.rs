use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{
    EXPERIMENTAL_SCHEMA, GroundedChange, KernelError, ObjectState, Patch, Receipt, Rejection,
    RejectionCode, SubmissionOutcome, WorldBootstrap, WorldSnapshot,
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
        let mut statement = self.connection.prepare(
            "SELECT event_type, payload FROM events WHERE world_id = ?1 ORDER BY sequence",
        )?;
        let events = statement
            .query_map([&self.world], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut snapshot: Option<WorldSnapshot> = None;
        for (event_type, payload) in events {
            match event_type.as_str() {
                "genesis" => {
                    let genesis: GenesisEvent = serde_json::from_str(&payload)?;
                    snapshot = Some(WorldSnapshot {
                        world: genesis.world,
                        revision: 0,
                        objects: genesis.objects,
                    });
                }
                "change_committed" => {
                    let event: ChangeCommittedEvent = serde_json::from_str(&payload)?;
                    let state = snapshot.as_mut().ok_or_else(|| {
                        KernelError::CorruptState("change appears before genesis".into())
                    })?;
                    state.revision += 1;
                    for patch in event.patches {
                        match patch {
                            Patch::PutObject {
                                reference,
                                expected_revision,
                                digest,
                            } => {
                                state.objects.insert(
                                    reference,
                                    ObjectState {
                                        revision: expected_revision
                                            .map_or(1, |revision| revision + 1),
                                        digest,
                                    },
                                );
                            }
                        }
                    }
                }
                other => {
                    return Err(KernelError::CorruptState(format!(
                        "unknown event type {other}"
                    )));
                }
            }
        }
        snapshot.ok_or(KernelError::WorldNotFound)
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
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn rejected(code: RejectionCode, detail: String) -> SubmissionOutcome {
    SubmissionOutcome::Rejected(Rejection { code, detail })
}
