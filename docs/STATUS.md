# Research status

Date: 2026-10-09 (updated after the first code review, ADR 0009/0010). Phase: candidate A work-model
calibration; chain prototype frozen at the E6 gate.

English is primary.

## Implemented

- Transparent Python reference verifier: Freivalds matrix-product verification over Goldilocks
  (`P = 2**64 - 2**32 + 1`), with field helpers, `matmul`, `matvec` and a deterministic RNG.
- Independent Rust verifier (`crates/abacus-verifier`) with explicit bounds, `u128` products, shape
  and canonicality checks that reject (never panic) on malformed input, and unit tests; a self-test
  binary and stdin/stdout differential adapters.
- Python/Rust parity: a 64-case Freivalds corpus, Fiat–Shamir challenge parity, and **every consensus
  derivation** of the chain (preheader, instance, product, score, block id, FS challenges, A'
  dataset, gather indices, gathered instance, sumcheck transcript) via `abacus-vectors`
  (`tests/test_chain_parity.py`); `reference/chain.py` is the single Python reference.
- Goldilocks field, NTT and a **Fiat–Shamir** sumcheck (verifier-derived challenges), in Python and
  Rust, with primitivity, round-trip/linearity, completeness, soundness and zero-challenge-forgery
  tests.
- Candidate A assessment and work model (ADR 0005); the Fiat–Shamir Freivalds binding (ADR 0004)
  with commit-then-expand (ADR 0006) over `(preheader, C)`; the corrected soundness bound
  `<= 2^-63` per challenge, `k = 2..3` suggested (ADR 0009, `scripts/freivalds_soundness_probe.py`).
  Falsifier probes: instance structure, omega, Freivalds forgery, Freivalds soundness.
- A toy CPU mine-and-verify loop with the chain's derivations (`scripts/mine_sim.py`).
- **Chain prototype** (`crates/abacus-chain`): encoding v2 (difficulty committed in the preheader),
  difficulty enforced against the ancestor-derived retarget, median-time-past timestamps, work
  counted from the required target (`u128`), greatest-cumulative-work fork choice
  (`docs/research/local-prototype-v1.md`).
- **Multi-node P2P prototype** (`p2p.rs`, `bin/abacus-node.rs`): pull sync with bounded lines,
  timeouts and a connection cap; lock-free fetch/validate; periodic `--resync`; reorg by adopting a
  higher-work chain; a solo/pool `JOB`/`SUB` protocol with per-miner extranonce ranges and per-miner
  accounting (`docs/research/multi-node-testnet-v1.md`). This is the E6 gate.
- A **CPPminer backend** (`--algo abacus`, branch `feat/abacus-backend` in `danifest751/CPPminer`) built
  against encoding v1. **It must be updated for v2** before it can mine against this node (ADR 0010,
  `docs/research/cppminer-backend-v1.md`).
- ASIC posture and candidate A' (ADR 0007, 0008, spec/04): a data-dependent epoch dataset
  (`reference/memhard_dataset.py`, `build_dataset`), a balance probe
  (`docs/research/memhard-balance-v1.md`) and a gather-bandwidth probe
  (`docs/research/gather-bandwidth-v1.md`).
- One local check command: `python scripts/check.py`.

The ordinary check gate uses standard libraries; the Rust crates have no third-party dependencies.
No GPU job, paid CI or GitHub Actions. Experiments write to an ignored `artifacts/`.

## GPU

- `cuda/goldilocks_matmul_bench.cu` — Goldilocks matmul throughput baseline on the CMP 50HX (sm_75):
  ~44–64 GMAC/s with a naive tiled kernel; ~415x a naive single-threaded CPU at n=512. A
  **throughput baseline**, not a matched D5 comparison (`docs/research/gpu-baseline-v1.md`).
- `cuda/gather_bench.cu` — warm cooperative gathered reads ~416 GB/s.
- `cuda/attempt_bench.cu` — A' attempt rate. **v1 results withdrawn** (linear fold, uncoalesced
  gather; ADR 0010). v2 (nonlinear fold) is not yet compiled or measured.

## Open findings (not claims)

- **A' is not shown to be memory-hard.** One block per entry is compute-bound (ADR 0008); the
  large-slice result was withdrawn (ADR 0010). Storage need is `8 * N` bytes, and no time-memory
  trade-off analysis exists.
- **D5 is not done.** Only a throughput point exists; the matched energy x time CPU/GPU comparison
  that D5 requires has not been run.
- **D6 (nonlinear anchor) is not done.**
- The `(n, D, k)` profile must be re-derived under ADR 0009.

## Not implemented (by scope)

- No coin, rewards, transactions, mempool, signatures or wallets.
- No gossip, peer discovery, rate limiting or peer scoring; no external testnet (E7).
- No production miner; no external cryptographic review yet.

## Next

Per `docs/CRITICAL-PATH.md`: (1) re-derive the `(n, D, k)` profile under ADR 0009 and re-run the
verifier bench; (2) the matched CPU/GPU D5 scope; (3) the D6 anchor note. Chain, pool and CPPminer
work stays parked (CRITICAL-PATH §6) except for the v2 compatibility update.
