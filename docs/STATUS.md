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
- Goldilocks field (`P = 2**64 - 2**32 + 1`), NTT and the sumcheck protocol, in both Python and
  Rust, with primitivity, round-trip/linearity and completeness/soundness tests.
- One local check command: `python scripts/check.py`.

The ordinary check gate uses standard libraries; the Rust crate has no third-party dependencies.
No GPU job, paid CI or GitHub Actions. Experiments will write to an ignored `artifacts/`.

## GPU

- `cuda/goldilocks_matmul_bench.cu` — Goldilocks matmul throughput baseline (CPU vs GPU), measured on
  the CMP 50HX (sm_75): GPU ~415x naive CPU at n=512, ~44 GMAC/s (naive tiled kernel). See
  `docs/research/gpu-baseline-v1.md`. This is a **baseline**, not a miner.

## Not implemented (by scope)

- No miner, no work model, no difficulty, no chain, no network, no coin.
- No external cryptographic review yet.

## Next

D2 (candidate work functions and falsifiers) is recorded in
`docs/decisions/0002-candidate-work-functions.md`; the verifier now covers candidate B (NTT +
sumcheck). Next: run the decisive experiments D3-D5 (screening, precomputation/amortisation,
matched CPU/GPU scope) at the smallest profile where the effects appear, and design the nonlinear
anchor for A/B (see `docs/CRITICAL-PATH.md`).
