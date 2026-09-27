# M3 pre-registered protocol: incremental, explainable revision

Date: 2026-09-27
Status: pre-registered. Written before the M3 confirmation matrix was executed.
Relates to: [ADR-002](ADR-002-revisable-work.md), [SPEC.md](SPEC.md), [M2-PROTOCOL.md](M2-PROTOCOL.md)

The primary outcome and the scenario families below are fixed before the retained runs. The protocol is
amended only to record a correction and its reason, and only before a run it affects.

## Hypotheses under test

| ID | Hypothesis | Measured by |
|---|---|---|
| H3-Correction | In a closed deterministic profile, the observable incremental result equals a full recompute from the same current inputs, the same rules and the same trust scope | the equivalence property, plus manual expectations on the essential cases |
| H3-Sélectivité | Where some dependencies remain reusable, evaluation work is actually avoided without omitting a relevant obligation | avoided evaluations and obligations per mutation |
| H3-Transfert | After the first domain freezes, the same core applies to a second through an adapter, without multiplying domain rules in the core | not measured in this session, see below |
| H3-Utilité | A person can understand why something is being re-examined and continue without reconstructing the whole history | requires a consented observation; automated tests do not establish it |

None of these implies prior superiority over a competent incremental application. R3, A3, B3 and C3 are
compared in section 18 of the mandate and that comparison is not run here.

## Primary outcome, pre-registered

| Outcome | Criterion |
|---|---|
| Correction holds | Zero unexplained divergence from the oracle on the supported profile, and zero wrongly-declared reuse in the confirmation cases |
| Selectivity holds | At least one mutation avoids evaluation work while every relevant obligation is still produced |
| Correction fails | Any invalid change admitted as current, or any stale plan published as current |

Refusing everything is not a pass. A system that blocks every legitimate path scores worse, and the
progression families exist to make that visible.

## The central property

For a closed deterministic scope, targets `T` and the same current inputs `S1`:

```text
observable(incremental(S0, changes, T, profile)) == observable(full_recompute(S1, T, profile))
```

`observable` includes the relevant target values, the authorized business verdicts carried through, the
normalized obligations, the coverage reservations and currency in the compared context. Run identifiers,
durations, timestamps and legitimately differing history details are not compared byte for byte.

The oracle receives no `EvaluationRecord`, no index and no invalidation function, so it cannot call the
incremental strategy. The essential cases also carry manual expectations, so a mistake common to an
evaluator cannot make both routes equally wrong.

In partial and opaque profiles the comparison is over obligations and stated limits, not a reference
truth. A second call to the same model is not a reference truth.

## Work performed, measured separately

Counted independently, never collapsed into one score: fingerprint comparisons, dependency visits,
evaluator re-executions, query resolutions, and persistence operations. A single "recomputed nodes"
counter can hide an expensive traversal, so it is not the measure.

```text
reexecutions_evitees = executions_reference_complete - executions_systeme
ratio_evite = reexecutions_evitees / executions_reference_complete
```

If the denominator is zero the ratio is written "not applicable". A system that does more work keeps its
negative result. The ratio is never converted into token savings, euros or unobserved human time.

## Scenario matrix, 24 stories

Coverage target, not a statistical power claim. `tests/incremental_revision.rs` and
`tests/m3_confirmation.rs` run these; the coverage table is printed by
`cargo run --example m3_revision`.

| ID | Story | Covered here |
|---|---|---|
| M3-01 | identical query on an identical snapshot | yes |
| M3-02 | change to an object nothing consumed | yes |
| M3-03 | a direct dependency changed | yes |
| M3-04 | a chain of three derivations | yes |
| M3-05 | input changed, output identical | yes |
| M3-06 | diamond with two affected parents | yes |
| M3-07 | a presentation-only projection | yes |
| M3-08 | same content, assurance turned stale | partial: recorded assurance references are carried, UNI staleness is consumed, not simulated |
| M3-09 | current authority withdrawn | partial: authority is reported per finding, withdrawal is modelled as an authority dimension |
| M3-10 | evaluator or comparator version changed | yes |
| M3-11 | conditional read switches from A to B | not covered: no conditional evaluator in the fixture |
| M3-12 | a collection gains a relevant member | yes |
| M3-13 | an observed absence becomes a value | yes |
| M3-14 | a read entry is deleted | yes, as a tombstone |
| M3-15 | an observation expires on explicit time | not covered: no time-triggered evaluator |
| M3-16 | a value returns to an earlier one | yes |
| M3-17 | support replaced, wording identical | partial: declared support is honoured, a same-wording swap is not distinguished |
| M3-18 | a new contradiction in a collection | not covered: obligation only, no arbitration is implemented |
| M3-19 | partial or opaque declared reads | yes |
| M3-20 | executable cycle versus narrative cycle | yes, the executable one is refused |
| M3-21 | a concurrent change before publication | yes, the plan is refused and replanning is required |
| M3-22 | crash then reopen and retry | not covered: the engine holds no durable store of its own yet |
| M3-23 | export, import, local change | partial: the M2 path is exercised, the M3 pass over an imported snapshot is not |
| M3-24 | a second domain through an adapter after the core freezes | not covered |

Cross the stories with unknown fields, missing keys, wrong digests, unsupported versions, forbidden
references and resource limits. A syntactically valid JSON object does not prove the format's invariants.

## Mutation checks

Targeted harness mutations must each break a corresponding test, and are confined to the test side. No
production control receives a disable flag.

1. drop one dependency edge, and a chain test must fail
2. ignore a query read, and a collection test must fail
3. compare final text instead of consumed facets, and a selectivity test must fail
4. ignore an evaluator version change, and a cache test must fail
5. accept a self-declared coverage, and a profile test must fail
6. publish despite a stale plan, and the publication test must fail
7. close an obligation without assurance, and the obligation test must fail

## Reporting rule

The protocol publishes failures and non-supports alongside successes. A story that is not covered is
listed as not covered. Work avoided is reported next to work done, so a selective pass and a pass that
merely re-ran everything are never confused.

## Scope of the conclusion

The fixture is invented arithmetic, which makes it checkable by hand and useless as evidence about real
work. No reliability rate follows from a hand-designed suite, no line count becomes human time, and a
capability demonstrated on a closed profile is not a demonstration for a second domain. M3's established
property, if it holds, is precise and narrow: keep what can be kept, re-examine what has to be, and make
the difference inspectable.
