# World Kernel

World Kernel is an experimental local runtime for grounded changes. It connects an exact candidate,
the observations it read, an assurance result, current authority, an atomic commit and a replayable
receipt.

It is deliberately not an agent framework, a model router, a policy engine or the source of truth for
integrated products. Producers propose. Assurance providers evaluate. The Kernel admits and records a
change inside the World it owns.

## Current slice

The Rust library exposes one deep mutation interface:

```rust
kernel.submit(&grounded_change, &authority)
```

It returns either a typed rejection or a receipt. The implementation checks the exact candidate,
trusted assurance provider, current authority, world revision, dependency revisions, context coverage,
patch preconditions and idempotency before publishing the projection and event in one SQLite
transaction.

`Kernel::snapshot` reads the current projection. `Kernel::replay` reconstructs it from the event log
without invoking a model or an external effect.

The first integration seams preserve product ownership:

- `UniCollector` hashes the candidate before and after UNI verification, asks UNI to export and
  validate its versioned evidence bundle, requires valid `artifact_files` evidence for that exact
  path and digest, and only then binds the byte-stable accepted report to the candidate.
- Kollio documents become versioned observations while Kollio remains their canonical owner.
- Kollio impact assessments become reassessment frontiers, never truth verdicts.

## M1 benchmark

An equal-information benchmark compares three systems over one versioned 80-case corpus: an
application-specific SQLite gate, the same gate with the verifier outcome produced through the UNI
seam, and UNI plus the Kernel. All three receive the same deserialized case, so no system gets a hidden
oracle or an extra dependency.

```bash
cargo run --example admission_benchmark
```

The recorded run is in
[experiments/admission-benchmark](experiments/admission-benchmark/README.md), with the method, the
limits and a negative result: on these 80 conditions the portable envelope prevented no class of
error that a competent application transaction does not also prevent. Whether that justifies the
envelope is an open product decision, recorded in
[RESULTS.md](experiments/admission-benchmark/RESULTS.md).

## Transferable experience

A past attempt, packaged so it can be evaluated somewhere else. The output is a plan, not a boolean, and
the rule it exists to protect is that **similarity is not applicability**: two contexts can match on every
loud key and still differ on the one that matters.

```bash
cargo run --example transfer_benchmark
```

`src/experience.rs` refuses to collapse four states that look alike: a fact that is unknown, a fact that is
absent, a capability that was never checked, and a key nobody declared. `src/transfer.rs` compares the
capsule's declared conditions against a target and returns one of five statuses with the obligations the
status implies. Four of those requirements are construction sites rather than rules to remember: an
instantiated target candidate is built with an empty assurance list, adaptability is a declared boolean
defaulting to false, a recurrent prior failure is an obligation rather than a sentence, and a plan bound to
a revision that has since moved refuses to instantiate.

The recorded result is a **tie** with a competent non-Kernel baseline, and it is recorded as one. 30 closed
cases across four families, zero false direct transfers on either side, one real capability difference
found, seven mutations each failing a test. A tie funds nothing further. The result is in
[experiments/transfer-benchmark](experiments/transfer-benchmark/RESULTS.md).

## Branch convergence

Two alternatives, one adoption. Alpha and Beta each look fine on their own, and combining them breaks a
rule neither broke. That is the case a value-comparison merge passes and a real gate must not.

```bash
cargo run --example m4_branch
cargo run --example transfer_benchmark
```

`src/branch.rs` forks from an immutable base, prepares a proposal against pinned source and target,
reconstructs the candidate, and checks the **target's own rules on that candidate** before anything moves.
A refusal names the rule and the versions it looked at, the target does not change, and both branch
histories stay readable. A revised branch that composes to exactly the limit is admissible, because a test
that blocks every adoption is not a test.

Scope is tranche 1 only, pre-registered in [M4-PROTOCOL.md](docs/M4-PROTOCOL.md) and authorised by
[ADR-004](docs/ADR-004-the-facet-advantage-scales.md). No multi-parent merge, no automatic re-grounding, no
automatic merge of any kind, and no second impact engine. The recorded result is in
[experiments/branch-convergence](experiments/branch-convergence/RESULTS.md).

## The impact engine

When the conditions change, recorded work has to become revisable without being rebuilt, and the difference
has to be explainable. The engine for that was built, measured, reduced away, and then restored, because
the measurement that justified the reduction turned out not to hold on a real shape of payload. Details in
[ADR-003](docs/ADR-003-measure-before-building.md) and
[ADR-004](docs/ADR-004-the-facet-advantage-scales.md).

What the engine enforces is also written down as rules a consumer can be held to: five separate results
instead of one validity flag, a recompute that
cannot promote a record, profiles granted by the consumer, a recorded human decision that is never
recomputed, a dependency that is what a run actually consumed, a publication that is a conservative
compare-and-swap, and nothing that launches an external effect. See
[IMPACT-CONTRACT.md](docs/IMPACT-CONTRACT.md).

It is in `src/impact`, and the comparison that judges it stays re-runnable:

```bash
cargo run --example m3_revision
```

The recorded M3 result, its coverage and its limits are in
[experiments/incremental](experiments/incremental/README.md).


## The comparison, and why the experiment stops here

Three milestones produced the same sentence: capability established, differential value not measured. The
only increment that could settle it was to measure the core against a competent application that had the
same information. So the last run is a comparison, not a milestone.

```bash
cargo run --example incumbent_comparison
cargo run --example m4_branch
```

R3 is a full recompute, A3 is a competent application cache on whole values, B3 is the same application
consuming UNI staleness, and C3 is the core. All four got the same seven scenarios. All four reached the
same observable result. On six of the seven, A3 and B3 avoided exactly the work C3 avoided. On the seventh,
the core ran no evaluator where the application ran two, because a consumed facet is a narrower cache key
than a whole value.

Everything else the core produces, the five dimensions, the obligation, the disposition, the explanation,
the granted profile and the protection of a recorded decision, is not consumed by the fixture. The result
is a reduction, not a victory: reduce the Kernel to UNI plus adapters, and do not start M4. The Kernel
is now admission, history, portable continuity and adapters. The clause
"the core saves nothing the application could not already do" is false and is recorded as false. The
decision and its reversal case are in
[ADR-003](docs/ADR-003-measure-before-building.md), the numbers in
[RESULTS.md](experiments/incumbent-comparison/RESULTS.md).

## Verify

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
cargo run --example vertical_slice
cargo run --example admission_benchmark
cargo run --example m2_continuation
cargo run --example m3_revision
```

## Scope limits

This prototype has no external effect dispatcher, signatures, distributed authority, negative-query
read sets or generic merge protocol. `AuthoritySource` is checked synchronously at admission but is not
yet a versioned authority ledger. The UNI collector detects ordinary mutation and post-verification
substitution, but it does not provide an immutable filesystem snapshot against a concurrent A-B-A
mutation. The host process and configured UNI binary remain trusted. Those limitations are part of the
experiment, not hidden guarantees.

See [docs/BOUNDARIES.md](docs/BOUNDARIES.md) and [docs/EXPERIMENT.md](docs/EXPERIMENT.md).

## Documentation map

- [docs/SPEC.md](docs/SPEC.md) is the normative contract and active implementation milestone.
- [docs/BOUNDARIES.md](docs/BOUNDARIES.md) records state ownership, trust and current proof limits.
- [docs/IMPACT-CONTRACT.md](docs/IMPACT-CONTRACT.md) states the impact rules a consumer can be held to, and
  the stable codes, bound to the engine by a test.
- [docs/M4-PROTOCOL.md](docs/M4-PROTOCOL.md) and [docs/M5-PROTOCOL.md](docs/M5-PROTOCOL.md) are the
  pre-registered scopes of the branch and transfer tranches.
  [ADR-004](docs/ADR-004-the-facet-advantage-scales.md) is the reversal of the reduction and the M4 decision.
- [docs/EXPERIMENT.md](docs/EXPERIMENT.md) records the experimental question and falsification gates.
- [AGENTS.md](AGENTS.md) is the short entry point for OpenCode and other coding agents.
