# World Kernel agent contract

This repository is the independent World Kernel experiment. It is not part of IntentLane, UNI or
Kollio.

## Read order

1. Read `docs/SPEC.md` before planning or changing behavior. It is the normative product and protocol
   specification, and it names the active milestone.
2. Read `docs/BOUNDARIES.md` when work touches ownership, trust, UNI, Kollio or external state.
3. Read `docs/EXPERIMENT.md` when work touches benchmarks, falsification or claims about value.
4. Inspect the code and tests before relying on a document's description of current behavior. Running
   code wins when a status line has drifted.

## Current checkpoint

- Baseline commit: `c24b3fc`.
- The admitted-change slice, UNI exact-candidate collector, SQLite rollback and restart recovery are
  implemented.
- The active milestone is `M1` in `docs/SPEC.md`: an equal-information A/B/C admission benchmark with
  80 deterministic cases.

## Working rules

- Keep the Kernel independent. Integrate UNI and Kollio through their public surfaces. Do not edit
  their repositories as part of World Kernel work.
- Keep domain policy in adapters or experiment fixtures. The Kernel owns generic admission and
  history mechanics only.
- Preserve the distinction between impact and truth, assurance and authority, internal commit and
  external effect.
- Add no external effect dispatcher before the M1 continuation gate passes.
- Use test-driven development for behavior changes. A failing contract test must precede the fix.
- Update `docs/SPEC.md` when a durable requirement or milestone status changes. Update
  `docs/BOUNDARIES.md` when a proved guarantee or trust assumption changes. Do not create another
  architecture overview.
- Keep unrelated user changes intact. Do not push or modify remote state without explicit approval.

## Verification

Run all of these before handing work back:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
cargo run --example vertical_slice
jq empty schemas/world-change-v0.experimental.schema.json
```

When changing `ProcessUniRunner`, also build UNI and run the ignored live contract test as documented
in `docs/SPEC.md`.

