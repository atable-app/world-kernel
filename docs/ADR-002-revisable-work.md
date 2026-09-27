# ADR 002: M3 makes recorded work revisable, and keeps human judgement human

Date: 2026-09-27
Status: accepted for one bounded experiment
Supersedes: nothing
Relates to: [ADR-001](ADR-001-portable-continuation.md), [SPEC.md](SPEC.md), [M3-PROTOCOL.md](M3-PROTOCOL.md)

## Audit of the real state, recorded rather than assumed

The mandate stated that no report attesting M2's completion had been provided. That is true of the
conversation and it is not true of this workspace. Tranche 0 requires the real state, so it was
inspected rather than inferred in either direction.

| Item | Observed at `bc29a22` | Consequence for M3 |
|---|---|---|
| M1 benchmark | Implemented, 80 cases, 56 tests, recorded at `5003e90` | Its negative result stands and is not re-run to make C win |
| M2 portable continuation | Implemented at `6d129bd`: `src/continuation.rs` 597 lines, `Kernel::export_continuation`, `Kernel::import_history`, an `admissions` table | M3 can be built on it rather than on a copy of a SQLite file |
| M2 sequence matrix | 24 of 24 pre-registered sequences covered for C2, 4 of them C2 only | A2 and B2 still have no export or resume, so no M2 cost comparison exists |
| M2 known bounds | The Kernel stores digests and never object bytes; a reconstruction cannot bootstrap its own trust | M3 inherits both, and a change engine must not imply artifact bodies exist |
| `src/impact` | Absent | M3 starts from nothing in the core |
| UNI | Local `c1d4c1f`, binary `uni 0.9.4`, not modified | UNI keeps claims, evidence, staleness and assurance decisions |
| Tree | Clean, on `main`, nothing pushed | All work stays local |

M2's integration with a real product path was never demonstrated, and no consumer outside this
repository's language has read the format. Those remain unverified and M3 does not inherit them.

## The decision

M2 asked whether work survives its producer. M3 asks the next question: when the conditions change, does
the work become revisable without being rebuilt, and can the difference be explained?

The deliverable is a reusable impact engine with one technical domain and one reflective domain. It is
not a universal agent orchestrator. The user gets a structured answer to four questions: what changed in
the observed scope, what can still be reused and on what basis, what is now necessary, and why each
object is concerned or was kept.

The engine keeps human judgement. A changed constraint does not let it overwrite a recorded decision
with another one, and it never launches an external effect.

## Five dimensions, and why there is no `valid` boolean

The single most important modelling decision is the refusal to collapse meaning into one flag.

| Dimension | Question | Recorded as |
|---|---|---|
| History | What actually happened at the previous revision? | `history`: the recorded transition, decision or evidence |
| Currency | Can this result serve the current request? | `currency`: reusable, needs revalidation, unknown, unsupported |
| Business verdict | What does the authorized evaluation conclude? | `business_verdict`: opaque text owned by the domain or UNI, never computed here |
| Current authority | May this actor act now? | `authority`: authorized, refused, unresolved |
| Coverage | Over which dependencies does the conclusion hold? | `coverage`: closed profile, declared partial, opaque |

A test that passed on v1 remains historically passed. After its subject changes, its result may no longer
suffice for v2. That is neither proof that v2 fails nor permission to relabel v1 as never tested. A human
assertion stays recorded while its support becomes something to re-examine; the engine never rewrites its
wording, its author or its date.

## A dependency is what was consumed, not what was mentioned

A link, an arrow or a citation is not automatically a computational dependency. Provenance can explain;
only a consumed dependency can force re-evaluation. The same relation may serve both, and the engine must
know which property it actually established. A narrative graph may contain cycles without forcing the
evaluation graph to accept executable ones.

Every execution records what it read: the object or the logical query, the resolved version or snapshot,
the facet actually consumed, the trust projection used, and the fingerprint of the value consumed. A
historical reference pinned to v1 and a read of "the current version of X" are not the same thing, and
moving a head never rewrites a historical reference.

A narrower comparison than the whole canonical value requires a deterministic, typed, versioned
projection from the trust registry. An embedding or a similarity score is not an authorised equality
comparator. If the comparator's identity or version changes, the affected uses are revalidated; an old
fingerprint is never reused as if its definition had not moved.

## Profiles are granted, not self-declared

`closed_deterministic` means trusted deterministic evaluators, relevant inputs captured in a snapshot, no
undeclared ambient access, an acyclic graph and pinned comparators. A Rust read trait is not a sandbox: a
plugin with filesystem or network access can step around it. The closed profile is granted only to
built-in controlled evaluators or to an environment whose restriction has actually been established.

`declared_partial` means the domain declares dependencies without exhaustively capturing them. Known
impacts can be identified; absence of a known link is not a promise of universal non-effect.

`opaque` means hidden inputs, old production without usable traces, or an uninstrumented process. The
answer is a collection or review obligation, or conservative reconstruction of a defined zone.

A manifest supplied by an agent cannot award itself the closed profile. The trust and the right to run an
evaluator come from the consumer's allowed configuration. We do not claim to detect every hidden
dependency; a controlled omitted dependency proves only that the tested mechanism reacts to that one
omission.

## Queries, absences and dynamic dependencies

Reads are not limited to "I read these three files". A query for "every observation of this capability", a
file search or a "no blocking constraint remains" check also depends on the composition of the set. A new
element can change the answer without changing anything already read. M3 therefore records a query read
with the query's identity and version, its scope, the collection snapshot and the result fingerprint, and
in the initial profile invalidates conservatively on any change to the relevant collection. No fine
indexation is claimed.

The same rule covers a file that was absent and then created, a rule added, a newly present contradictory
observation, and a removed entry. Deletions become explicit tombstone transitions rather than silently
missing references. Conditional reads record their branch selector, and moving from read A to read B
publishes the new read set and drops A from the current dependencies in the same transaction while the old
set stays consultable in history.

A new dependency may be discovered during recomputation, so a plan is a known impact frontier, not an
immutable promise about every future operation. Its revisions and its reasons for extension are versioned.

## Propagation stops on consumed values, not on text

After recomputation the engine compares the facets actually consumed, not a hash of final text. A consumer
is reusable when all its relevant values and preconditions remain equivalent under an accepted comparison
contract and its coverage permits that conclusion. This refreshes no evidence by magic: content may be
identical while its provenance, its UNI evidence, its availability or its authority condition has moved.

When two parents of one node are affected, equivalence on the first does not erase the second's impact.
Stopping is decided per consumer, after examining all of its relevant dependencies.

Materialization publishes evaluation updates, obligations and a derived view patch. A new business decision
stays a proposal until the appropriate authority and admission. Before publication the plan's
preconditions are rechecked, and **the initial M3 uses a conservative compare-and-swap on the whole world
revision**: any concurrent change forces replanning. Finer per-read-set validation may come later and is
not assumed safe here.

## Obligations, not automatic re-run of everything

An obligation is structured work: target and snapshot, reason, work type, preconditions, expected evidence,
required authority and a dedup key. The minimal types are pure recompute, a new UNI verification, a new
external observation, a human re-review, and resolution of a missing reference or permission. The dedup
identity includes the target, the relevant context, the work type and the evaluation profile, so repeating a
plan does not create two identical tasks, while an incompatible new context does not abusively reuse the
old identity.

An agent receives the obligation and its authorized data, proposes a result, and then goes back through the
normal assurance and admission path. Closing a task because an agent said "done" is forbidden. A recorded
model output can be reread as an artefact; re-running a model is a new production and is never treated as a
demonstrated deterministic recomputation.

## The oracle is independent, and the proof is tested under hypotheses

For a closed deterministic scope, targets T and the same current inputs S1:

```text
observable(incremental(S0, changes, T, profile)) == observable(full_recompute(S1, T, profile))
```

This is a tested objective under stated assumptions, not a general theorem about agents and the real world.
`observable` includes the relevant target values, the authorized business verdicts, normalized obligations,
coverage reservations and applicability in the compared context. Run identifiers, durations, timestamps and
legitimately differing history details are not compared byte for byte.

The oracle is a full recompute that never receives an `EvaluationRecord`, an index or an invalidation
function, so it cannot call the incremental strategy even by accident. The essential small cases also carry
manual expectations, so a mistake common to an evaluator cannot make both paths equally wrong.

## What is refused

- A single `valid` flag, or any global truth claim about the world.
- A manifest that awards itself the closed profile.
- An embedding or a similarity score used as an equality comparator.
- Executable cycles in the evaluation graph, hidden by arbitrarily cutting an edge. Narrative cycles are
  fine because they are not executed.
- A new graph database, a mandatory server, multi-machine consensus, a CRDT, a general proof language, a
  universal causal model, reliable automatic discovery of every dependency, or autonomous external effects.
  Those are M4, an external effect ledger, or explicit non-goals.
- Publishing a stale plan as current, or letting a changed constraint authorise anything by itself.

## Status

This ADR records a decision to run one bounded experiment. It records no outcome. If a small UNI adaptation
plus an application journal covers the need at the same or lower cost, the recommendation will be to
reduce, keeping the dependency model, the tests and the useful formats rather than continuing out of
attachment to the name World Kernel.
