//! M3-22: crash then reopen and retry.
//!
//! The impact engine holds no durable store of its own, and the coverage table recorded that as the
//! reason this story was not covered. The store belongs to the application that runs a pass, so the
//! question the story asks is whether that application can lose a process and continue. This suite
//! answers it with a real store and a real crash: the child is this same test binary re-executed with
//! an env var, it writes inside an open transaction, and it leaves through `std::process::exit`, which
//! skips every destructor including SQLite's own cleanup. No error is returned to anyone and no
//! response reaches anyone. What a later process can read is the only evidence there is.
//!
//! The state a reopen needs is exactly `Revised`, which is why the stored document is the whole of it
//! and not a summary of it. A pass that finds nothing new still has records, and records are what the
//! next pass starts from; a document that carried only findings would let one hop be recovered and
//! not the sequence.
//!
//! Three interruption points, the same three `crash_symmetry.rs` measures for the admission layer:
//! before the write, after the write but before commit, and after commit but before the response.
//! The middle one is the interesting one, because the pages have reached the disk and the transaction
//! has not, so recovering means rolling back work that is physically present.
//!
//! The oracle runs inside `conditional_pass`, so every recovered pass below is also checked against
//! a full recompute of the same snapshot. Recovery that converged on a wrong answer would fail there
//! rather than only against the in-memory baseline.

mod support;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use support::m3_stories::{conditional_pass, conditional_records, conditional_world};
use world_kernel::impact::{EvaluationRecord, NodeId, Revised, Snapshot};

const CHILD_ROLE: &str = "WK_M322_CHILD";
const CHILD_POINT: &str = "WK_M322_POINT";
const CHILD_DATABASE: &str = "WK_M322_DATABASE";

/// The row a pass is committed under is the revision it started from, so a retry of the same pass
/// lands on the same row and an earlier pass can never be displaced by a later one.
const PASS_TWO: i64 = 2;

// ------------------------------------------------------------------ the worlds

fn world_one() -> Snapshot {
    conditional_world(1, "a", 10, 20)
}

fn world_two() -> Snapshot {
    conditional_world(2, "b", 10, 30)
}

fn world_three() -> Snapshot {
    conditional_world(3, "a", 11, 30)
}

fn world_four() -> Snapshot {
    conditional_world(4, "b", 12, 30)
}

// -------------------------------------------------------------- the documents

/// A pass as the store holds it: the whole engine outcome, not a digest of it.
fn document_of(revised: &Revised) -> Value {
    json!({
        "baseRevision": revised.base_revision,
        "targetRevision": revised.target_revision,
        "findings": revised.findings,
        "obligations": revised.obligations,
        "evaluations": revised.evaluations,
        "reused": revised.reused,
        "records": revised.records,
    })
}

fn revised_from(document: &Value) -> Revised {
    Revised {
        base_revision: document["baseRevision"]
            .as_u64()
            .expect("a stored base revision"),
        target_revision: document["targetRevision"]
            .as_u64()
            .expect("a stored target revision"),
        findings: serde_json::from_value(document["findings"].clone())
            .expect("stored findings round-trip"),
        obligations: serde_json::from_value(document["obligations"].clone())
            .expect("stored obligations round-trip"),
        evaluations: document["evaluations"]
            .as_u64()
            .expect("a stored evaluation count") as usize,
        reused: document["reused"].as_u64().expect(" a stored reuse count") as usize,
        records: records_of(document),
    }
}

fn records_of(document: &Value) -> BTreeMap<NodeId, EvaluationRecord> {
    serde_json::from_value(document["records"].clone()).expect("stored records round-trip")
}

// ------------------------------------------------------------------ the passes

/// The pass that completed before the crash, always in the parent and always first.
fn pass_one() -> Revised {
    let recorded = conditional_records(&world_one());
    conditional_pass(&world_one(), &world_two(), &recorded).0
}

fn pass_two_from(recorded: &BTreeMap<NodeId, EvaluationRecord>) -> Revised {
    conditional_pass(&world_two(), &world_three(), recorded).0
}

/// The answer a run that never lost a process produces, computed end to end in memory.
fn clean_pass_two() -> Revised {
    let recorded = conditional_records(&world_one());
    let first = conditional_pass(&world_one(), &world_two(), &recorded).0;
    pass_two_from(&first.records)
}

// --------------------------------------------------------------------- the store

fn open_store(database: &Path) -> Connection {
    let connection = Connection::open(database).expect("the store opens");
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS impact_passes (
                 sequence  INTEGER PRIMARY KEY,
                 document  TEXT NOT NULL
             );",
        )
        .expect("the store can be created");
    connection
}

fn commit(connection: &Connection, sequence: i64, document: &Value) {
    connection
        .execute(
            "INSERT OR REPLACE INTO impact_passes (sequence, document) VALUES (?1, ?2)",
            params![sequence, document.to_string()],
        )
        .expect("the pass is committed");
}

fn stored(connection: &Connection, sequence: i64) -> Option<Value> {
    connection
        .query_row(
            "SELECT document FROM impact_passes WHERE sequence = ?1",
            params![sequence],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|document| serde_json::from_str(&document).expect("a stored pass is valid JSON"))
}

fn latest(connection: &Connection) -> Option<Value> {
    connection
        .query_row(
            "SELECT document FROM impact_passes ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|document| serde_json::from_str(&document).expect("a stored pass is valid JSON"))
}

fn rows(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM impact_passes", [], |row| row.get(0))
        .expect("the store can be counted")
}

// ------------------------------------------------------------------ the child

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterruptionPoint {
    /// Nothing is written. The pass existed only in the memory of a process that is gone.
    BeforeWrite,
    /// The pages reached the disk inside an open transaction and the process is gone before commit.
    BeforeCommit,
    /// The pass is durable and the process is gone before anyone is told.
    AfterCommit,
}

impl InterruptionPoint {
    const ALL: [InterruptionPoint; 3] = [
        InterruptionPoint::BeforeWrite,
        InterruptionPoint::BeforeCommit,
        InterruptionPoint::AfterCommit,
    ];

    fn as_str(self) -> &'static str {
        match self {
            InterruptionPoint::BeforeWrite => "before-write",
            InterruptionPoint::BeforeCommit => "before-commit",
            InterruptionPoint::AfterCommit => "after-commit",
        }
    }

    fn exit_code(self) -> i32 {
        match self {
            InterruptionPoint::BeforeWrite => 17,
            InterruptionPoint::BeforeCommit => 18,
            InterruptionPoint::AfterCommit => 19,
        }
    }
}

/// Reopens the store, performs the second pass from what the first one left, and disappears at its
/// interruption point. The child never returns.
fn child_main() -> ! {
    let point = InterruptionPoint::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.as_str() == std::env::var(CHILD_POINT).unwrap_or_default())
        .expect("the child knows its interruption point");
    let database =
        PathBuf::from(std::env::var(CHILD_DATABASE).expect("the child knows its storage"));

    let connection = open_store(&database);
    let resumed = latest(&connection).expect("a pass survived the previous commit");
    let revised = pass_two_from(&records_of(&resumed));
    assert_eq!(
        revised.base_revision as i64, PASS_TWO,
        "the pass under test"
    );

    if point == InterruptionPoint::BeforeWrite {
        std::process::exit(point.exit_code());
    }

    connection
        .execute_batch("BEGIN IMMEDIATE")
        .expect("the child opens a transaction");
    commit(&connection, PASS_TWO, &document_of(&revised));

    if point == InterruptionPoint::BeforeCommit {
        // The write has reached the disk and the transaction has not. Nothing closes it: no
        // destructor runs and no error is returned to anyone.
        connection.cache_flush().expect("the pages reach the disk");
        std::process::exit(point.exit_code());
    }

    connection
        .execute_batch("COMMIT")
        .expect("the pass becomes durable");
    // The work is durable; the response is lost.
    std::process::exit(point.exit_code());
}

fn run_child(point: InterruptionPoint, database: &Path) -> std::process::Output {
    Command::new(std::env::current_exe().expect("the test binary is re-executable"))
        .env(CHILD_ROLE, "1")
        .env(CHILD_POINT, point.as_str())
        .env(CHILD_DATABASE, database)
        .arg("--exact")
        .arg("child_process_entry_point_is_a_no_op_when_not_re_executed")
        .output()
        .expect("the test can re-execute itself")
}

/// The parent half. When re-executed as the child, this test hands over and never returns.
#[test]
fn child_process_entry_point_is_a_no_op_when_not_re_executed() {
    if std::env::var(CHILD_ROLE).as_deref() == Ok("1") {
        child_main();
    }
}

// ------------------------------------------------------------- the parent's view

/// Commits the first pass, hands the store to a child that may not come back, and returns the state
/// the parent found on reopening.
///
/// The first pass is committed here in the parent on purpose. A crash story that also lost its
/// precondition would measure the wrong thing.
fn crash(database: &Path, point: InterruptionPoint) -> Connection {
    let connection = open_store(database);
    let first = pass_one();
    assert_eq!(
        first.base_revision, 1,
        "the first pass starts at revision one"
    );
    commit(
        &connection,
        first.base_revision as i64,
        &document_of(&first),
    );

    let output = run_child(point, database);
    assert_eq!(
        output.status.code(),
        Some(point.exit_code()),
        "the child left through its interruption point and not through a failure: {output:?}"
    );

    // Before the reopen, and therefore before SQLite has had the chance to recover, the evidence of
    // what the child actually did is still on disk. Opening the store would roll a hot journal back
    // and delete it, which would make the middle interruption point indistinguishable from the first.
    let journal = PathBuf::from(format!("{}-journal", database.display()));
    match point {
        InterruptionPoint::BeforeWrite => assert!(
            !journal.exists(),
            "nothing was written, so there is no journal to recover"
        ),
        InterruptionPoint::BeforeCommit => assert!(
            journal.exists(),
            "the interruption left a hot journal: the pages reached the disk inside an open \
             transaction, and it is SQLite's rollback that has to undo them"
        ),
        InterruptionPoint::AfterCommit => assert!(
            !journal.exists(),
            "a committed pass has no journal left behind, so the reopen is a plain read"
        ),
    }

    // The child's connection is gone with it. This is the reopen.
    open_store(database)
}

/// Reopens, resumes from whatever survived, and commits the second pass.
///
/// A pass already committed under this revision returns unchanged: an exact retry performs no new
/// mutation. The document is returned beside the pass so the caller can prove the stored bytes
/// still describe it.
fn reopen_and_retry(database: &Path) -> Revised {
    let connection = open_store(database);
    if let Some(document) = stored(&connection, PASS_TWO) {
        return revised_from(&document);
    }
    let resumed = latest(&connection).expect("the first pass survived its own commit");
    let revised = pass_two_from(&records_of(&resumed));
    commit(&connection, PASS_TWO, &document_of(&revised));
    revised
}

// ------------------------------------------------------------- the assertions

#[test]
fn a_crash_leaves_the_store_at_exactly_the_state_it_had_committed() {
    let expected = document_of(&pass_one());

    for point in [
        InterruptionPoint::BeforeWrite,
        InterruptionPoint::BeforeCommit,
    ] {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let database = directory.path().join("impact.db");
        let connection = crash(&database, point);

        assert_eq!(
            rows(&connection),
            1,
            "{:?} leaves one committed pass and no second one",
            point
        );
        assert_eq!(
            stored(&connection, PASS_TWO),
            None,
            "{:?} never publishes the pass it lost",
            point
        );
        let resumed = latest(&connection).expect("the first pass is still there");
        assert_eq!(
            resumed, expected,
            "{:?} does not disturb the state a reopen has to resume from",
            point
        );
        assert_eq!(
            revised_from(&resumed),
            pass_one(),
            "and it describes the same pass after a round trip through the store"
        );
    }
}

#[test]
fn a_retry_after_a_crash_converges_on_the_result_of_a_pass_that_never_crashed() {
    let expected = clean_pass_two();

    for point in InterruptionPoint::ALL {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let database = directory.path().join("impact.db");
        let connection = crash(&database, point);
        drop(connection);

        let recovered = reopen_and_retry(&database);
        assert_eq!(
            recovered, expected,
            "{:?} recovers to the answer a clean run produces, records included",
            point
        );

        let connection = open_store(&database);
        assert_eq!(
            rows(&connection),
            2,
            "{:?} commits the second pass exactly once",
            point
        );
        assert_eq!(
            stored(&connection, PASS_TWO).expect("the second pass is now committed"),
            document_of(&expected),
            "and the bytes it committed describe that pass"
        );
    }
}

#[test]
fn an_exact_retry_returns_the_stored_outcome_and_performs_no_new_mutation() {
    for point in InterruptionPoint::ALL {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let database = directory.path().join("impact.db");
        let connection = crash(&database, point);
        drop(connection);

        let first = reopen_and_retry(&database);
        let committed = {
            let connection = open_store(&database);
            stored(&connection, PASS_TWO).expect("the second pass is committed")
        };

        let second = reopen_and_retry(&database);
        assert_eq!(
            second, first,
            "{:?} a repeated retry does not recompute or diverge",
            point
        );

        let connection = open_store(&database);
        assert_eq!(rows(&connection), 2, "{:?} adds no row on a retry", point);
        assert_eq!(
            stored(&connection, PASS_TWO).expect("the second pass is still there"),
            committed,
            "{:?} a retry rewrites nothing",
            point
        );
    }
}

#[test]
fn the_state_a_reopen_needs_is_enough_to_carry_on_with_the_next_pass() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let database = directory.path().join("impact.db");
    let connection = crash(&database, InterruptionPoint::BeforeCommit);
    drop(connection);
    reopen_and_retry(&database);

    // Everything below starts from the store and from nothing else: no in-memory pass, no snapshot
    // carried across a process boundary.
    let connection = open_store(&database);
    let resumed = latest(&connection).expect("two passes are committed");
    let third = conditional_pass(&world_three(), &world_four(), &records_of(&resumed)).0;

    let recorded = conditional_records(&world_one());
    let first = conditional_pass(&world_one(), &world_two(), &recorded).0;
    let second = pass_two_from(&first.records);
    let expected = conditional_pass(&world_three(), &world_four(), &second.records).0;

    assert_eq!(
        third, expected,
        "the records left by two recovered passes start the third one in the same place"
    );
}
