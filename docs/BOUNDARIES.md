# State ownership and seams

## Verified starting state

The implementation began from read-only inspection on 2026-09-27.

| Project | Revision read | Working tree | Authority retained by the project |
|---|---|---|---|
| UNI | `c1d4c1f6ea4caf3b359c0ff17fdabd533a56f3a4` | Source clean, unrelated `.DS_Store` files untracked | Contracts, evidence, policies and assurance decisions |
| Kollio | `536e7553a4eb6bc2fd3ec0d54102cf4c98ca4897` | Dirty with concurrent user work, left untouched | Document, commands, decisions and impact semantics |
| World Lab | `5fea88c` | Read only | Public deterministic simulation |
| IntentLane | Not integrated | Not modified | Capability contracts and Apple integration evidence |

The fourth product named `Template` in the research brief was not identifiable from the local project
registry. No adapter or product semantics were invented for it.

## Ownership

| State | Canonical owner | Kernel representation |
|---|---|---|
| Native World objects | World Kernel | Versioned projection and replayable events |
| UNI assurance | UNI | `AcceptedAssessment` reference bound to an exact candidate |
| Kollio document | Kollio | Versioned observation with `canonical_owner = kollio` |
| Kollio impact | Kollio | Reassessment frontier, not a verdict |
| External resources | Their originating system | Reference, digest and observed revision only |

## Public seams

`Kernel::submit(change, authority)` is the mutation seam. The caller must know the change envelope and
provide a current authority source. Validation, optimistic concurrency, trusted-provider checks,
idempotency, transaction ordering and receipt creation remain inside the module.

`Kernel::snapshot()` and `Kernel::replay()` are the read seams. Replay consumes recorded events only.
It never calls adapters, models or external systems.

Kollio adapters are pure translation seams. The UNI collector invokes only UNI's public CLI: `verify`,
the versioned `bundle export` / `bundle verify` transport, and the byte-stable JSON `report`. It never
reads UNI's private cache directly or writes product-owned files itself. UNI remains responsible for
its own evidence and decision state.

## Trust model

The host process and Kernel database are trusted in this first mono-authority profile. Producers are
not trusted to mint authority. A World stores the names of assurance providers its owner trusts.
Merely placing an `AcceptedAssessment` in a proposal does not make an unknown provider acceptable.

The stable UNI report contains the decision but not the candidate digest. `UniCollector` closes the
ordinary substitution gap by hashing the file before and after verification, verifying UNI's exported
bundle through UNI itself, and requiring a valid bundle evidence record whose workspace-relative path
and SHA-256 equal the candidate. The resulting assessment reference hashes the normalized evidence,
stable report and candidate together.

The remaining filesystem race is an A-B-A mutation during the verifier command: without an immutable
snapshot or operating-system lock, the candidate can theoretically change and return to its original
bytes between the two Kernel hashes. The mono-authority profile therefore still trusts the host process
and the configured UNI binary. Also, `AcceptedAssessment` is a data type, not a signature: untrusted
code must not run inside the host and mint provider name `uni` directly.

## Invariant coverage

| Invariant | Current evidence |
|---|---|
| WK-01 exact subject | UNI bundle evidence path and digest must match the candidate; admission rechecks the assessment subject and digest |
| WK-02 current authority | `AuthoritySource` checked on every new submission |
| WK-03 current context | World revision and positive object reads checked |
| WK-04 no half commit | Projection and event share one SQLite transaction |
| WK-05 impact is not truth | Kollio adapter returns only a reassessment frontier |
| WK-06 replay has no effects | Replay reads only recorded SQLite events |
| WK-07 uncertain effects | Out of scope because external effects are not implemented |
| WK-08 no self-authorization | Trusted assurance providers belong to World bootstrap state |
| WK-09 visible view limits | Truncated or incomplete coverage is rejected |
| WK-10 product autonomy | Adapters are read-only and products remain independently usable |
| WK-11 versioned semantics | Unknown change schema is rejected |
| WK-12 isolated scopes | Proposal World must equal the opened World |

WK-03 remains partial because negative-query dependencies are not represented. WK-02 does not yet
provide a transactionally versioned external authority snapshot.

UNI interface claims above follow its checked-in `README.md`, where `report` is byte-stable and bundles
are the transport surface, plus `docs/evidence.md`, which defines `uni-bundle-0.1` integrity and
cross-check semantics. The collector has a live contract test against the local UNI CLI in addition to
its fake-process seam tests.
