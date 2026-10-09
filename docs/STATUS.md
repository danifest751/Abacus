# Research status

Date: 2026-10-09. Phase: laboratory bootstrap (verifier + differential corpus).

English is primary.

## Implemented

- Transparent Python reference verifier: Freivalds matrix-product verification modulo
  `P = 2**61 - 1`, with field helpers, `matmul`, `matvec` and a deterministic RNG.
- Independent Rust verifier (`crates/abacus-verifier`) with explicit bounds, `u128` products and
  unit tests; a self-test binary and a stdin/stdout differential adapter.
- Deterministic Python/Rust differential corpus (64 cases, mixed true/tampered) plus Python
  accept/reject and field-edge tests.
- One local check command: `python scripts/check.py`.

The ordinary check gate uses standard libraries; the Rust crate has no third-party dependencies.
No GPU job, paid CI or GitHub Actions. Experiments will write to an ignored `artifacts/`.

## Not implemented (by scope)

- No miner of any kind (CPU or GPU), no work model, no difficulty, no chain, no network, no coin.
- No external cryptographic review yet.

## Next

See `docs/CRITICAL-PATH.md`: state candidate work functions and their linearity/precompute
falsifiers, then run the decisive experiments D3–D5 (screening, precomputation/amortisation,
matched CPU/GPU scope) at the smallest profile where the effects appear.
