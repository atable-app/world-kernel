# ADR 004: the reduction is reversed, and M4 continues

Date: 2026-09-27
Status: accepted
Supersedes: the reduction recorded in [ADR-003](ADR-003-measure-before-building.md)
Relates to: [ADR-001](ADR-001-portable-continuation.md), [ADR-002](ADR-002-revisable-work.md), [ADR-003](ADR-003-measure-before-building.md), [IMPACT-CONTRACT.md](IMPACT-CONTRACT.md)

## What happened, in order

[ADR-003](ADR-003-measure-before-building.md) compared a full recompute, a competent application cache,
the same application consuming UNI staleness, and the impact core on the same seven scenarios. All four
reached the same observable result. The application avoided the same work as the core on six of the seven.
On the seventh the core ran no evaluator where the application ran two, because a consumed facet is a
narrower cache key than a whole value.

That is one measured advantage, worth 2 evaluator runs, against 1542 lines of engine. Two of the three
pre-registered clauses were met and the third, "the core saves nothing the application could not already
do", was false and recorded as false. The reduction was applied on the narrower ground that every other
capability the core produced was unconsumed by the fixture, so the decision was not to keep paying for
capability nothing had measured.

ADR-003 named the condition under which that decision was wrong, and named the command to test it:

> If a competent application had to grow a facet comparator, a disposition or an obligation to pass these
> scenarios, the reduction would be wrong. It did not have to, and the one difference the core does buy, a
> consumed facet, is available to the application for a fraction of the lines.
>
> The correction is cheap and does not need a new milestone. Keep the M3 experiment and its fixture, and
> re-run `cargo run --example incumbent_comparison` against a graph with wide nodes. If the saving scales
> past the line count, the reduction is wrong and the core comes back.

## The command was run

`cargo run --example incumbent_comparison` now measures the reversal case across fan-out widths. One
observed input carrying one unconsumed field, N readers of the single field they actually use, and a
change that touches only that unconsumed field:

| Consumers | A3, whole-value cache | C3, facet cache | Avoided |
|---|---|---|---|
| 3 | 3 | 0 | 3 |
| 10 | 10 | 0 | 10 |
| 100 | 100 | 0 | 100 |
| 1 000 | 1 000 | 0 | 1 000 |
| 10 000 | 10 000 | 0 | 10 000 |
| 1 000, eight unconsumed fields | 1 000 | 0 | 1 000 |

The advantage is exactly the fan-out, and it is unbounded within any graph size worth building. Padding
the node from one unconsumed field to eight changes nothing, which is the finding rather than a detail: a
whole-value cache asks only whether the node moved, never how much of it was consumed. One field beyond
the consumed one is enough.

The condition ADR-003 set is met. The saving does scale past the line count, in the only sense that can be
measured here: it is linear and unbounded rather than a fixed 2.

## Why the fixture could not have found it

The arithmetic fixture gives every node one field, and its consumer reads that field. That is the single
shape of payload where a whole-value key and a consumed-facet key coincide, so the seven scenarios could
not separate the two mechanisms except in the one case added by hand. Real payloads carry provenance,
labels, timestamps and hashes that evaluators do not read. The fixture was not wrong; it was unrepresentative
of the thing the decision was about, and the decision rested on it.

This is the error worth naming. I treated a measurement on a closed three-node fixture as a statement about
the mechanism in general, then built a decision on it, and only ran the check the same document said was
required. The check was cheap, it was written into the decision, and I still deferred it.

## What no money figure means here

No cost per evaluation was measured, so there is no break-even fan-out and no claim about euros, tokens or
human time. The avoided runs are a count. What the count does is remove the objection the reduction rested
on, and that is enough: the reduction said the core bought nothing worth its lines, and that is now
falsified on its own stated condition.

## The decision

The reduction is reversed. `src/impact` is restored to the shipped surface. The Kernel is admission,
history, portable continuity, the impact engine, and the adapters.

The contract document survives and now describes rules the Kernel enforces rather than rules a removed
engine happened to enforce. `the_contract_document_and_the_measured_codes_agree` still binds the document
to the engine, and still fails in both directions.

M1's negative result stands unchanged: no admission superiority was demonstrated, and nothing here touches
it. The corpus never consumed a receipt, so replay and handoff value remain unmeasured.

## What M4 is and is not authorised to do

A request for M4, branch exploration and convergent adoption, arrived as an implementation brief. It is
adopted as a direction. It is not adopted at the scale it was written at.

The brief states its own scope rule, and it is the right one:

> If only the first part is achievable, deliver `partial M4-Core` with what is verified. Do not mark M4
> complete because the spec files and a few structs exist.

So the scope is tranche 0 and tranche 1 of that brief: the real audit, a continuation ADR, a pre-registered
protocol, and the Alpha/Beta fixture with an independent oracle, isolated snapshots, and the proof that a
combination of 110 is blocked while a composition of 100 is admissible after a decision. The remaining
tranches, and the 32-story matrix, are not authorised by this ADR and are not claimed.

One thing the brief gets right and this ADR endorses: M4 is not the invention of branches, provenance or
merges. Git, Dolt, LangGraph, W3C PROV and Bazel's Skyframe all exist. M4's contribution is a contract for
carrying alternatives together with their hypotheses, justifications, obligations and adoption conditions.
Its relevance and cost are still to be compared against competent applications, exactly as M1 and M3 were.

## The continuation condition, restated

The first red test is the one the brief names and it is the whole of tranche 1: Alpha and Beta are each
correct from their common base, their combination is wrong, and the target must not move. A test that
passes by blocking every adoption does not count, so a revised Beta that composes to exactly the limit must
be admissible after a decision and the required assurance.

If that test cannot be written without building a second impact engine, that is the signal the brief
warns about, and the answer is to reduce rather than to proceed.
