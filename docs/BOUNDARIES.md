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

Adapters are pure translation seams. They do not reach into UNI or Kollio private storage and they do
not write back into either product.

## Trust model

The host process and Kernel database are trusted in this first mono-authority profile. Producers are
not trusted to mint authority. A World stores the names of assurance providers its owner trusts.
Merely placing an `AcceptedAssessment` in a proposal does not make an unknown provider acceptable.

The remaining weak point is the exact-subject binding around `uni --json report`: the current stable
report contains the decision but not the candidate digest. A trusted collector must compute and bind
that digest in the same operation. A later UNI export should carry this binding directly.

## Invariant coverage

| Invariant | Current evidence |
|---|---|
| WK-01 exact subject | Assessment reference and digest must match the candidate |
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

