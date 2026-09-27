# M2 continuation experiment

The recorded run of the experiment defined in
[`docs/M2-PROTOCOL.md`](../../docs/M2-PROTOCOL.md) and decided in
[`docs/ADR-001-portable-continuation.md`](../../docs/ADR-001-portable-continuation.md).

## Question

Can a fresh consumer reconstruct a work state and its justifications from exported data, then
determine what can be resumed in its own current context, without depending on the producer's
session, private database or paths?

## What exists

| Piece | Where |
|---|---|
| The continuation package format and the consumer | `src/continuation.rs` |
| The export and the historical import on the Kernel | `src/kernel.rs` |
| The first consumption test, red before the implementation | `tests/continuation_consumer.rs` |
| The pre-registered sequence matrix, executed | `tests/m2_sequences.rs` |
| Crash symmetry for A2, B2 and C2 in real subprocesses | `tests/crash_symmetry.rs` |
| The recorded result and its generated human view | `results.json`, `RESULTS.md` |

## Commands

```bash
cargo run --example m2_continuation                    # record
cargo run --example m2_continuation -- --render-only   # RESULTS.md from results.json
cargo test --test continuation_consumer                # the consumption contract
cargo test --test m2_sequences                         # the sequence matrix
cargo test --test crash_symmetry                       # interruption symmetry
```

## What the package carries, and what it refuses to claim

The Kernel stores references, revisions and digests. It never stored object bytes. The package
therefore reconstructs a projection and its history, and declares every resource body `absent`. A hash
without accessible content does not reconstruct an artifact, and a body claim the consumer cannot check
is refused rather than believed.

A package carries no trust configuration. The expected head arrives out of band and is compared against
a digest the package cannot influence, so a package can never vouch for itself. A chain of digests alone
would not detect a coherent rewrite or a truncation below an announced head, which is why the anchor
exists and why authenticity is reported as unestablished without one.

Guarantees are six separate results, not one verdict: `format_understood`, `resource_integrity`,
`authenticity`, `completeness_within_scope`, `historical_reconstruction` and
`completeness_beyond_scope`. The last is never claimed. Completeness is relative to an announced
manifest, a head and a scope; a hidden uninstrumented source cannot be detected by a package.

The recorded declarations travel in the package so a consumer never has to guess its limits.

## What the reconstruction does not do

It is not an admission. `import_history` writes recorded transitions and maintains the projection with
the same reduction `Kernel::replay` uses, so a rebuilt projection and a replayed one cannot drift, and
the suite asserts `snapshot == replay` on the destination. It consults no authority, no assurance and no
current context, because none of those were required for the transitions to have been recorded. It
writes only to the destination the caller chose, and never touches the live World or an application.

It cannot bootstrap its own authority. The trusted provider set of the destination is the consumer's own
configuration, passed explicitly, and `import_history` leaves it untouched. Reading the past needs no
trust; admitting anything new does. That is a separate, explicit act, and the suite proves the refusal
names the missing trust rather than something else.

## Interruption

Three observable points are exercised for A2, B2 and C2 alike, in a subprocess the test itself creates:
before the transaction, after a write but before commit, and after commit but before the response.

A trigger-raised SQLite error and the disappearance of a process are not the same experience, and both
are recorded as such. The pre-commit point is produced by arming a temporary trigger on the last durable
write and then leaving through `std::process::exit` from inside the failed submit, so no destructor runs
and no error reaches anyone. The parent observes only what a later process can read. On retry, the suite
requires no duplicate transition, intact state, and a stable response; a reused identity with new content
is a conflict rather than a retry.

The injected trigger is durable in the database, so recovery means it has ended. The parent drops it
before retrying, which is what the word recovery means here.

A recorded gap, not hidden: C2 has a replay seam and A2 and B2 do not yet have one, because an
application-level replay is part of the A2 baseline that M2 has not built.

## What UNI is reused for, not reinvented

The local UNI at `c1d4c1f`, binary `uni 0.9.4`, already provides `uni bundle export` and `bundle verify`
with per-record digests and cross-checks, the seven-dimension evidence invalidation model, the append-only
`.uni/events.jsonl` journal, and `uni brief`. The package references those artifacts and versions; it does
not re-implement claim evaluation, evidence validity or policy. Assurance references travel as historical
assertions by their issuer, and verification here is reference reading, not re-running the producer's
verifiers. UNI's own statement holds in this design: importing a bundle never injects proofs, because
transport is not authority.

## Equal-information state

Sequences 3, 4, 21 and 24 are covered for C2 only. A2 and B2 have no export and no application-level
resume of their own, so the same matrix has not been run against them and no cost comparison exists. The
report states this rather than narrowing the claim.

## What is not established

- No consumer outside this repository's language has read the format. The interoperability requirement
  in the protocol is not met yet.
- No demonstration inside a real product path was attempted, so no capability is claimed there. An
  AppIntentsTesting observation would have required a real host app, its test target, signature and
  metadata, and actually observed Apple identifiers.
- No independent authority mechanism was tested. Only integrity relative to a digest received over a
  trusted channel is claimed.
- The suites are hand-designed. They are not independent observations of a population, so no reliability
  rate follows from them, and no line count is converted into human time.

## Conclusion recorded

Capability established for C2; differential value not measured. The capability is real: a fresh consumer
reconstructs the announced state from the package alone, benign paths progress, a moved declared
dependency is blocked and named, and no resumption depends on the producer. What remains unproven is
whether the envelope is the cheapest way to obtain it, which is the question M2 exists to answer and
which requires A2 and B2 to run the same matrix.
