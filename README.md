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

## M3 impact engine

A fresh consumer is not enough: when the conditions change, recorded work has to become revisable without
being rebuilt, and the difference has to be explainable. `src/impact` answers that. It keeps five results
separate, history, currency, business verdict, current authority and coverage, because a result can be
historically true and no longer usable without either being false.

```bash
cargo run --example m3_revision
```

The engine reports, per object, whether no change can reach it, whether a change reached it and its consumed
values held, whether it was recomputed to the same value or a new one, and what is now necessary. A recorded
human decision is never recomputed; when its declared support moves, the engine produces an obligation and
changes nothing. The recorded result, its coverage and its limits are in
[experiments/incremental](experiments/incremental/README.md).

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
- [docs/EXPERIMENT.md](docs/EXPERIMENT.md) records the experimental question and falsification gates.
- [AGENTS.md](AGENTS.md) is the short entry point for OpenCode and other coding agents.
