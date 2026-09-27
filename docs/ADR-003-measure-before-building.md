# ADR 003: the next increment is a comparison, not a fourth capability

Date: 2026-09-27
Status: accepted, then reversed on its own reversal case by
[ADR-004](ADR-004-the-facet-advantage-scales.md). The reduction below was applied and then undone. The
decision to measure rather than to build M4 is the part of this ADR that held: it is what produced the
measurement that refuted its own conclusion.
Supersedes: nothing
Relates to: [ADR-001](ADR-001-portable-continuation.md), [ADR-002](ADR-002-revisable-work.md), [M3-PROTOCOL.md](M3-PROTOCOL.md)

## The decision, and the reasoning that produced it

The continuation rule in the M3 mandate is mechanical, so it is applied mechanically.

| Condition for continuing to M4 | Observed | Met |
|---|---|---|
| Correction holds in the announced profile | yes, on the closed deterministic fixture | yes |
| Selectivity is measured on relevant cases | yes, evaluation is avoided where consumed facets hold | yes |
| Transfer to a second domain reveals a useful abstraction | not attempted, not measured | **no** |

Two of three. The rule does not fire. M4, whose scope is hypothesis branches and revisable decisions, is
not authorised by the evidence. I am not starting it.

The rule for reduction requires B3 to cover the need at a comparable or lower cost. That has never been
measured, for M2 or for M3. So reduction is not authorised either. And "conclude partially" is exactly
where the evidence sits: the core works, and the second domain, the human utility and the cost comparison
are unverified.

The pattern across three milestones is the actual finding. M1 concluded "no admission superiority, cost
measured". M2 concluded "capability established for C2, differential value not measured". M3 concluded
"capability demonstrated for C on a closed profile, differential value not measured". Three milestones, one
recurring sentence. Each was an honest result, and together they are a signal: every remaining increment has
been spent adding capability, and capability is not the thing in doubt.

So the next increment is not a milestone. It is the one measurement that would settle the question, and the
mandate already asks for it in section 17: an equal-information comparison of a full recompute, a competent
application incremental engine, the same application reusing UNI, and the World Kernel core, on the same
inputs, with work performed counted separately from correctness.

## What the comparison is, and what it may not become

A3 and B3 are given the same fixture, the same evaluators, the same snapshots, the same trust scope and the
same ordered target list. They may choose their own representation. Neither reads an expected result. The
harness and the result format are shared; the competing decision mechanisms are not.

A3 is the application mechanism, not a strawman. It does the obvious competent thing: keep a cache entry per
target holding the read set and the produced value, re-fingerprint the reads against the current snapshot,
reuse when they all hold, re-execute when one does not. It has no portable graph, no five dimensions, no
obligations, no explanations and no profiles. That is not a handicap invented to make C look good; it is what
an application actually is, and the cost of the portable machinery is the thing under measurement.

The comparison ends on a number, whichever way it falls. If A3 and B3 reach the same observable results with
the same work avoided, and with fewer lines, the portable graph has no measured value on this scope, and the
recommendation is to reduce to UNI plus adapters, keeping the dependency model, the read and facet contracts,
the closed profile rules, the corpora and the explanation codes. If the application mechanism cannot
express something the core expresses, the difference is named precisely and the cost of the difference is
recorded.

The forbidden reading is "A3 failed, therefore C wins". A3 is competent by construction or the experiment is
invalid, exactly as in M1.

## What this decision does not do

It does not abandon the dependency model or the read and facet contracts, which are the genuinely reusable
parts. It does not add a fourth capability, a graph database, a scheduler or a second domain. It does not
claim the comparison is free: it is the most expensive thing on the list, and it is the last thing worth
building.

## When the answer arrives

The reduction is applied if the comparison favours the application mechanism. If it favours the core, the
next decision is about IntentLane and a second domain, and that decision is a product judgement about
Guillaume's time rather than a technical one. Either way the experiment has a stopping point, which the
previous three milestones did not have.

## A note on the wider situation, stated once

World Kernel is an unbounded research expense. It has produced three negative-to-neutral results and a
genuinely useful amount of learned machinery. It competes with a dated commercial obligation. This ADR does
not decide that question, because it is not mine to decide, but a decision date is now attached to the
experiment: after the comparison, there is a reduce-or-stop, not another milestone.

---

# Outcome, recorded 2026-09-27

The comparison was run. The result is in `experiments/incumbent-comparison/`, produced by
`cargo run --example incumbent_comparison` and checked against a fresh measurement by
`the_recorded_result_file_matches_a_fresh_measurement` in `tests/m3_comparison.rs`.

## What was measured

R3, A3, B3 and C3 received the same seven scenarios. The scenarios are the seven essential M3 cases
plus one added here: a node carrying a field that nothing consumes, with only that field moving.

| | A3 and B3 | C3 |
|---|---|---|
| same observable result on every scenario | yes, 7 of 7 | yes, 7 of 7 |
| evaluator runs avoided on six of the seven | the same as C3 | the same as A3 |
| evaluator runs avoided on `unconsumed_field_change` | 1 of 3 | 3 of 3 |
| lines of mechanism | 238 | 1542 |

## B3 is not free, and the fixture cannot show that on its own

The seven scenarios declare no assurance, so on all seven B3 is A3 with an empty list. Reporting "B3 added
no work" from that would have been reporting a null result as a positive one. So the declaration was
measured once, separately, on an unchanged world where UNI declares stale a reference the support object
actually relied on: B3 re-runs that one consumer, answers identically, and costs one extra evaluation
against A3's zero.

Two things follow. Consuming a staleness answer is not free, it is a cost paid to avoid trusting something
that no longer holds. And B3 buys no correctness here: it produces the same answer A3 produced, more
slowly. If the reference had been stale for a reason the snapshot cannot show, the answer would differ,
and this fixture cannot produce that case. B3 is therefore recorded as unproven rather than as either
useful or useless.

## The pre-registered condition, clause by clause

The condition in this ADR was one sentence with two claims. It is recorded as written and judged
clause by clause, because it did not come out clean.

| Clause | Met | Evidence |
|---|---|---|
| A3 and B3 reach the same observable result as C3 on every scenario | yes | 7 of 7 agreed with the full recompute |
| the core saves nothing the application could not already do | **no** | on 1 of 7 the core ran 0 evaluators where the application ran 2 |
| at a comparable price | yes | 2 evaluator runs on a 3-evaluator fixture, for 1542 lines against 238 |

The second clause is false, and I am not going to round it up. The core buys exactly one thing the
application did not have on this fixture: a consumed facet is a narrower cache key than a whole value, so
a node carrying a field nobody reads does not invalidate its consumers. That is a real advantage, it is
the only one measured, and it cost 1304 net lines to get it on a graph of three derived nodes.

## Why the reduction is still recommended

The decision is not "the core is worthless". It is narrower and more defensible than that.

Everything the core produces beyond avoided work is unconsumed by the fixture. A consumer asked to
collapse the five dimensions into one bit would succeed on every scenario here, and the core would not
have stopped it. The obligation, the disposition, the explanation, the consumer-granted profile and the
protection of a recorded decision are all capabilities the fixture never asks for. The reduction is a
decision not to keep paying for capabilities that nothing has measured, at a moment when the only
capability that *was* measured buys 2 evaluator runs on a 3-evaluator fixture.

So: reduce the Kernel to UNI plus adapters, and do not start M4.

## What survives, and why each item survives

- The five impact dimensions, as a rule a consumer can be held to, not as a struct in the Kernel.
- The obligation, because a cache entry cannot tell a consumer that work is owed.
- The disposition, because a cache reports a value and not what happened to it.
- The explanation, because a cache cannot say why a consumer was kept.
- Consumer-granted profiles, because a cache decides its own competence.
- The decision rule, because a cache would happily recompute a recorded human decision.

## The case that would reverse this

The one measured advantage grows with the number of unconsumed fields per node and with graph width. On
a graph of thousands of nodes with real payload objects, a consumed-facet key could save a large
fraction of a full recompute, and 1542 lines would stop looking like a bad price. This fixture is three
nodes of arithmetic, so it cannot show that, and the reversal case is not hypothetical: it is the shape
of the real system.

The correction is cheap and does not need a new milestone. Keep the M3 experiment and its fixture, and
re-run `cargo run --example incumbent_comparison` against a graph with wide nodes. If the saving scales
past the line count, the reduction is wrong and the core comes back. Until that measurement exists, the
reduction stands and the cost of being wrong is one example runner.
