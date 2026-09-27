# Incumbent comparison: the reduction result

Decision: **reduce** — Reduce the Kernel to UNI plus adapters and stop building M4.

Pre-registered condition: A3 and B3 reach the same observable result as C3 on every scenario, and the core saves nothing the application could not already do at a comparable price.

| Clause | Met | Evidence |
|---|---|---|
| A3 and B3 reach the same observable result as C3 on every scenario | yes | all 7 scenarios agreed with the full recompute |
| the core saves nothing the application could not already do | **no** | NOT MET. On 1 of 7 scenarios the core ran fewer evaluators than the application, by 2 runs in total, because a consumed facet is a narrower key than a whole value. |
| at a comparable price | yes | the saving is 2 evaluator runs on a closed 3-evaluator fixture, bought with 1630 lines of engine against 257 lines of application |

What the decision rests on: Two of the three pre-registered clauses are met outright. The third is not, and it is the only measured advantage the core has over a competent application. The reduction is therefore a decision not to pay for capability that nothing measured, on the strength of one small measured saving that a consumer could buy back for a fraction of the price. It is not a claim that the capability is worthless.

## What each system is

| System | What it is | Role | Lines |
|---|---|---|---|
| R3 | full recompute | correction oracle, not a weak baseline, not a competitor | an oracle, not a price |
| A3 | application cache on whole values | the competent application mechanism, not a strawman | 257 |
| B3 | A3 consuming UNI staleness | the same application, told which assurance went stale | 257 |
| C3 | the World Kernel impact core | the portable mechanism under test, restored to the Kernel by ADR-004 | 1630 |

## Scenarios

| Scenario | R3 | A3 | B3 | C3 | Agreed |
|---|---|---|---|---|---|
| mutation_1_limit_100_to_90 | 3 | 2 | 2 | 2 | true |
| mutation_2_limit_90_to_70 | 3 | 3 | 3 | 3 | true |
| mutation_3_label_change | 3 | 0 | 0 | 0 | true |
| mutation_5_changed_input_same_output | 3 | 1 | 1 | 1 | true |
| mutation_6_diamond | 3 | 3 | 3 | 3 | true |
| unconsumed_field_change | 3 | 2 | 2 | 0 | true |
| mutation_7_unchanged_world | 3 | 0 | 0 | 0 | true |

## The reversal case ADR-003 required

does the consumed-facet advantage grow past the line count on a graph of wide nodes, as ADR-003 required before the reduction could stand?

Method: one observed input carrying unconsumed fields, N readers of the single field they actually use, and a change that touches only an unconsumed field. The most favourable honest case for the engine: a whole-value cache cannot see which part of a node was consumed.

| Consumers | Unconsumed fields | A3 runs | C3 runs | Avoided |
|---|---|---|---|---|
| 3 | 1 | 3 | 0 | 3 |
| 10 | 1 | 10 | 0 | 10 |
| 100 | 1 | 100 | 0 | 100 |
| 1000 | 1 | 1000 | 0 | 1000 |
| 10000 | 1 | 10000 | 0 | 10000 |
| 1000 | 8 | 1000 | 0 | 1000 |

Verdict: the advantage scales linearly with the number of consumers and is unbounded within any graph size worth building. One unconsumed field is enough; more of them change nothing.

Not measured: no cost per evaluation, so no break-even fan-out and no money figure. The avoided runs are a count and nothing else.

## Findings

- Same observable result everywhere: **true**
- Same work avoided everywhere: **false**
- B3 is A3 when nothing is declared: **true**
- B3 given a reference it relied on: 1 executions against A3's 0, same answer **true**
  - the fixture declares no assurance, so B3 on these scenarios is A3 with an empty list. Given a reference it actually relied on, B3 re-runs that consumer and answers identically for one more evaluation. Consuming a declaration is not free.
- Evaluator runs the consumed-facet cache saved: **2**

## What survives the reduction

- the five impact dimensions, because they keep a consumer from collapsing them
- the obligation, because an application cache cannot tell a consumer that work is owed
- the disposition, because a cache reports a value and not what happened to it
- the explanation, because a cache cannot say why a consumer was kept
- consumer-granted profiles, because a cache decides its own competence
- the decision rule, because a cache would recompute a recorded human decision

## What goes

- nothing is removed from the Kernel on the strength of this comparison: ADR-004 reversed the reduction after the reversal case fired
- M4, which would have added story to a scope with no measured value

## What would have made this decision wrong

The reduction would be wrong if A3 had to grow a facet comparator, a disposition or an obligation to pass these scenarios. It did not have to. The one difference the core does buy, a consumed facet, is available to the application for a fraction of the lines, and on a graph with many unconsumed fields that advantage would grow rather than shrink, which is the case to re-measure before anyone rebuilds this.

## Not measured

Nothing in this document is evidence about IntentLane, Sarah, Apple, a real budget, a real
operator, or a human being. Those paths were not run.

- no IntentLane path: the application mechanism was not run against a real consumer
- no Sarah path: the fixture is invented arithmetic, not a real operator
- no Apple path: no public surface was exercised
- no human utility: nothing here measures whether anyone is better served
- no generative DAG: the fixture is closed and hand-checkable
- no M2 or M3 cross-system comparison: this is the incremental scope only
- no M4: the decision in ADR-003 was to measure before building, not to build
