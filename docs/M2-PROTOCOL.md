# M2 pre-registered protocol: portable continuation

Date: 2026-09-27
Status: pre-registered. Written before the first M2 retained run.
Relates to: [ADR-001](ADR-001-portable-continuation.md), [SPEC.md](SPEC.md) section 8

This document fixes the primary outcome, the criteria and the scenario matrix before results exist. It
may be amended only to record a correction and its reason, before the affected runs. Amendments after a
retained run invalidate that run for confirmation purposes.

## Primary outcome, pre-registered

The primary outcome is decided on the continuation consumer, per system, over the 8 families below:

| Outcome | Criterion |
|---|---|
| Continuation established | The fresh consumer reconstructs the announced projection in every valid scenario, every benign scenario progresses to a real admitted transition where one is legitimate, and no resumption depends on the producer |
| Continuation blocked correctly | Rejections are correct and specific, a missing or changed relevant dependency produces an explicit re-verification obligation, and an out-of-scope change does not invalidate anything |
| Continuation failed | An invalid scenario is admitted, or a valid one is refused for a reason other than a declared boundary |

A system is not credited for refusing everything. Family 1 and the benign variants of families 2, 3
and 5 exist so that a consumer which refuses indiscriminately scores worse, not better.

## Secondary measures, pre-registered

Measured separately and never combined into a single score:

1. **Continuation errors**: invalid scenarios admitted.
2. **Unjustified refusals**: valid scenarios refused for an undeclared reason.
3. **Benign progression**: valid scenarios reaching a real next transition.
4. **Reconstruction success**: the reconstructed projection is identical to the expected canonical
   projection, compared by reference, revision and digest.
5. **Producer dependency during replay**: producer workspace reads, producer tool invocations, and
   external effects, counted by instrumented seams and asserted to be zero during replay.
6. **Work actually performed**: steps recomputed, bytes transmitted, measured durations, resources
   consumed.
7. **Manual interventions observed**, with a stable written definition recorded before the runs.
8. **Integration cost per new consumer**: common code, business-adaptation code, and core modifications,
   counted with a recorded command.

## Scope of the conclusion

The 80 M1 cases, their 240 executions and the variants of any single scenario are not independent
observations of a real population. Nothing in this protocol converts hand-designed tests into a
universal reliability probability, and no line count is converted into human time. M1's historical cost
and M2's new cost are reported separately and are never summed into a single trend.

## Scenario matrix, 24 sequences over 8 families

Valid, invalid and limit variants per family. The count is adjusted to real coverage and never
inflated: a family that cannot be honestly populated is reported as not covered rather than padded.

| # | Family | Scenario | Expected |
|---|---|---|---|
| 1 | Fresh consumer, unchanged context | New consumer, different path and destination, same current context, authorized actor | reconstruct, then a real next transition is admitted |
| 2 | Fresh consumer, unchanged context | Same, but the actor lacks the current capability | reconstruct, then continuation is blocked with an explicit authorization obligation, past still readable |
| 3 | Path and workspace portability | Producer workspace moved and removed; artifact referenced only by a portable path | reconstruct from the package alone, no producer path touched |
| 4 | Path and workspace portability | Referenced artifact bytes absent from the package | reconstruct the projection, expose an explicit absent-body obligation, refuse to resume what cannot be verified |
| 5 | Crash and lost response | Commit, then the producing process disappears before its response | re-open existing, repeat the identical proposal, one transition only, stable response |
| 6 | Crash and lost response | Interruption after a write, before commit | no visible transition, retry produces one transition |
| 7 | Read dependency modified | A read the proposal depended on advanced in the consumer's world | block, with the stale dependency named |
| 8 | Read dependency modified | An unrelated object advanced, outside the declared coverage | continuation unaffected, no false invalidation |
| 9 | Assurance reference substituted | Assurance reference altered to bind another candidate | block, exact-subject binding preserved |
| 10 | Assurance reference substituted | Assurance reference consistent, bytes unchanged, evaluator policy hash changed | distinguish recorded history from current applicability; recompute obligation, do not rewrite the past |
| 11 | Resource tampered | Exported resource body altered, digests unchanged | integrity failure naming the resource |
| 12 | Resource tampered | Package truncated before the announced head | detected against the out-of-band expected head |
| 13 | Order and history | Transitions reordered | rejected, the recorded order is the only accepted order |
| 14 | Order and history | An unknown event type present | corrupt-state error, never a guess at semantics |
| 15 | Format and version | Unknown package format version | explicitly unsupported, no partial consumption |
| 16 | Format and version | Unknown field, missing key, unknown enum variant | strict parse failure |
| 17 | Format and version | Reducer semantics version the consumer does not implement | explicitly unsupported |
| 18 | Import boundary | Path escaping the destination, and a symlink escape | refused before any write |
| 19 | Import boundary | Oversized resource and excessive record count | refused with a bounded failure, destination untouched |
| 20 | Import boundary | Package carrying a command or a privileged policy | not executed, not loaded, recorded as refused |
| 21 | Benign progression | Three successive legitimate transitions across one exported package | all three reconstruct and resume, no duplicate identity |
| 22 | Benign progression | A new proposal on new revisions after a completed one | a genuinely new proposal, the old identity not reused abusively |
| 23 | Trust profile | Consumer trust configuration is absent and not derivable from the package | authenticity reported as unestablished, integrity still reported separately |
| 24 | Trust profile | Consumer trust anchor does not match the package | authenticity failure, digest integrity unaffected |

Scenarios 1 and 21 are the progression anchors. Scenario 8 is the negative control for false
invalidation. Scenario 23 and 24 are the trust boundary.

## Metamorphic properties

Added because the matrix alone cannot distinguish a consumer that checks the right things from one that
happens to match fixed expectations:

1. **Path move without semantic change.** Exporting the same logical state from a different producer
   path yields a package that reconstructs an identical projection and identical continuation verdict.
2. **Export determinism.** Two exports of identical inputs with identical promises produce identical
   package bytes, excluding any recorded wall clock.
3. **Relevant dependency change.** Changing a dependency inside the declared coverage moves the verdict
   from applicable to review-required, and names the dependency.
4. **Out-of-scope change.** Changing something outside the declared coverage does not move the verdict
   and does not claim to invalidate a dependency that was never declared.
5. **Idempotent replay.** Replaying the same package twice into the same isolated destination produces
   the same projection and no duplicate transition.

Mutations act on the real consumer. No production control receives a disable flag, and no test
manufactures a win by weakening a check under a flag.

## Crash symmetry, protocol

Interruption is simulated in a subprocess created by the test only. No user session or process is ever
signalled. Three observable interruption points are exercised for A, B and C alike:

1. before the transaction opens,
2. after a write, before commit,
3. after commit, before the response is returned.

A trigger-raised SQLite error and the actual disappearance of a process are not the same experience and
are recorded as two distinct observations. On retry of the same logical change, the protocol requires:
no duplicate internal transition, intact state, and a stable appropriate response. The existing storage
is re-opened; no fresh World is constructed. A new proposal on new revisions must not abusively reuse
the old identity.

## Equal-information comparison, protocol

| System | Required capability |
|---|---|
| A2 | A competent application SQLite gate with its own log or snapshot, its own export, and application-level resume |
| B2 | The same application, integrating the real UNI mechanisms where they are relevant, including `brief` and bundles |
| C2 | UNI plus the World Kernel, with the continuation consumer |

All three receive the same business inputs, the same current-state sources, the same capabilities, the
same trust anchors, the same historical data and the same access budgets. They may choose their own
representation. None reads an expected result. The harness and the result format are shared; the
competing decision logic is not.

The question is not how to make A2 unable to succeed. If A2 and B2 also succeed, the recorded finding
is the cost actually observed to build and reuse each continuation, and the cost is documented rather
than dismissed as a theoretical impossibility of the baseline.

## Interoperability requirement

At least one consumer outside this repository's language, minimal TypeScript or Node, without importing
the Kernel's Rust library, must read the experimental format and verify at least three things: the
resource digests, the announced closure of the package, and one simple projection. This is an
interoperability test. It is not evidence of real adoption and it is not a requirement to rewrite the
UNI verifiers.

## Demonstration in an existing product

A real technical path already present in IntentLane is used, chosen after inspecting the files rather
than invented. Its organization is respected, including `observe`, `deriveClaimVerdict`, `claims` and
Journey View. No new score and no new journeys syntax is introduced. An AppIntentsTesting observation
requires a real host app, its test target, signature and metadata, and actually observed Apple
identifiers; where those are missing the capability is reported as not tested, even if a fixture passes.

Two experiences are distinguished: a deterministic independent reader first, then a real runtime or
agent replacement when one is available. The absence of the producer's conversational context is a
property to test, not a reason to give a baseline less data.

Kollio is not integrated into M2. Template remains an autonomous product whose precise role is not
inferred from its name.

## Reporting rule

The protocol publishes failures and non-supports alongside successes. A scenario that is not covered is
listed as not covered. A system that does not implement a capability reports it as unsupported and
receives no credit for it.
