# Incumbent comparison

R3, A3, B3 and C3 on the same inputs, with work performed counted separately from correctness.

- Method, the clause-by-clause judgement and the reversal case:
  [../../docs/ADR-003-measure-before-building.md](../../docs/ADR-003-measure-before-building.md)
- Generated human view: [RESULTS.md](RESULTS.md)
- Machine record: [results.json](results.json)

## How to run

```bash
cargo run --example incumbent_comparison
cargo run --example incumbent_comparison -- --render-only
```

The second command rebuilds `RESULTS.md` from the checked-in `results.json` without running a scenario,
so the human view can never claim something the recorded measurement does not contain.

## What the four systems are

| System | What it is |
|---|---|
| R3 | a full recompute, used as the correction oracle, not as a weak baseline |
| A3 | a competent application cache on whole values, not a strawman |
| B3 | the same application, told which UNI assurance went stale |
| C3 | the World Kernel impact core |

All four receive the same snapshots, the same evaluators, the same trust configuration and the same
ordered target list. They choose their own representation. None is given an expected result. The harness
and the result format are shared; the competing decision mechanisms are not.

The application mechanism lives in
[../../tests/support/application_incremental.rs](../../tests/support/application_incremental.rs) and the
scenarios in [../../tests/support/incumbent_comparison.rs](../../tests/support/incumbent_comparison.rs).
They are shared with [../../tests/m3_comparison.rs](../../tests/m3_comparison.rs), so a number in
`results.json` is the number the test just measured. That test asserts the committed artifact matches a
fresh measurement; if it fails, re-run the example rather than editing the artifact.

## Scope limits

B3 is A3 on all seven scenarios, because the fixture declares no assurance for B3 to consume. What a
declaration costs is measured once, separately, and recorded as a cost rather than as a null result. B3's
correctness on a case where staleness would actually change the answer is not covered.

This is the incremental scope only. It is not an M2 comparison and not an M3 confirmation matrix. It runs
no IntentLane path, no Sarah path and no Apple path, and it measures no human utility. The fixture is
closed, invented arithmetic that can be checked by hand, not a real budget. The consumed-facet advantage
is measured on three derived nodes and would grow on a graph of wide nodes, which is the case to
re-measure before anyone rebuilds what this reduction removed.
