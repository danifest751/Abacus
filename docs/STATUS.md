# Research status

Date: 2026-10-09. Phase: the primary PoW candidate is **candidate T** (ADR 0015, spec/07), a deep
requantized int8 network with row-piece tickets; it passed its first test. Side track: interactive
tensor-throughput attestation (ADR 0014). Earlier PoW findings: `docs/papers/tensor-pow-limits.md`.

## Results

- **Candidate T (spec/07)**: per attempt `L` layers of int8 GEMM + fixed-point requantization with
  epoch weights; tickets are pieces of output rows; a block carries `(nonce, i, c)` and is verified by
  recomputing one row. Measured (`tnet-v1`): 87.8% of an attempt on tensor cores at `n = 8192, L = 8`
  (59 TMAC/s), CPU verification 31 ms (8 threads), single-row mining 42–45x more expensive per
  ticket, fair lottery, GPU/Rust byte parity.
- **Candidate A (spec/03)** is a sound permissionless PoW at the construction level: header-derived
  instance (no screening, no reuse), Fiat–Shamir Freivalds verification with per-challenge error
  `<= 2^-63` (ADR 0009); a full block check takes ~19 ms at `n = 256` on one CPU core, about 1/5 of a
  naive CPU product, dominated by instance expansion (`verifier-throughput-v2`).
  Recorded limits: no usefulness, ASIC-friendly, `8 n^2`-byte blocks, linear algebra adds cost but
  not security (`ASSESSMENT.md`).
- **Candidate A' (spec/04)**: the one-word gather is compute-bound; a nonlinear large-slice gather is
  bandwidth-bound for a tuned miner (ADR 0011), which turns A' into an Ethash-class bandwidth PoW. A
  linear fold is broken by prefix sums (ADR 0010).
- **Candidate A8 (spec/05)**: exact int8 product on tensor cores, ~78 TMAC/s on the CMP 50HX (~440x
  the Goldilocks kernel), verified by Freivalds over Goldilocks. Blocker: committing to and shipping
  `C` costs as much as the product below `n ~ 1500` and is 16–256 MiB where the product dominates.
  Test 1 (ADR 0013): a per-attempt succinct commitment costs more than the product (19–57x measured,
  >= 1.4x ideal at `n = 4096`), so the single-GEMM form is stopped; escapes E1 (winner-only proof of a
  plain hash) and E2 (deep requantized chain) remain; E1's winner proof measured at ~4–9 min on 8 CPU
  threads for `n = 4096` (`e1-hash-proof-v1`). Pearl already deploys an int8-GEMM PoW (`prior-art-v1`).
- **Candidates B and C**: verifiers only; B's verification advantage is a `log n` factor (ADR 0003).
- **Tensor-throughput attestation (spec/06)**: commit-then-sample over fresh int8 products; GPU prover
  byte-compatible with the Rust verifier; certifies 40–49 TMAC/s on a CMP 50HX (GEMM alone 74–80),
  online verification 7–26 ms; cheaters skipping 1/5/10% of rows passed 56/16/0 of 80 challenges as
  predicted (`attest-v1`).
- Research record and current notes: `docs/research/README.md`.

## Implemented

- Verifiers in Python (`reference/`) and Rust (`crates/abacus-verifier`): Goldilocks field, Freivalds,
  Fiat–Shamir challenges bound to `(preheader, C)`, NTT, Fiat–Shamir sumcheck; malformed input is
  rejected, never panics.
- Chain prototype (`crates/abacus-chain`, encoding v2): committed and enforced difficulty,
  median-time-past timestamps, greatest-work fork choice, optional A' one-word gather, pull sync with
  resource bounds, solo/pool protocol with per-miner accepted/stale/rejected accounting
  (`chain-prototype-v2`).
- Parity: every consensus derivation is compared Python vs Rust (`tests/test_chain_parity.py`); CUDA
  vs Rust is checked end to end by mining into the node.
- GPU benches (`cuda/`) and the suite runner `scripts/gpu_suite.sh` (warm, repeated, hashed); CPPminer
  backend on encoding v2 (`cppminer-backend-v2`).
- Probes (`scripts/`): instance structure, work exponent, Freivalds forgery and soundness, A' balance,
  toy mining.
- Gate: `python scripts/check.py` — pytest, rustfmt, clippy with warnings as errors, Rust tests,
  self-test.

## Not done

- **D5** (matched CPU/GPU, energy x time) and **D6** (nonlinear anchor for external data).
- A parameter profile `(n, bits, k)`; proof-size reduction; dataset time–memory analysis for A'.
- External cryptographic review of the current state.
- By scope: no coin, rewards, transactions, signatures, gossip or external testnet.

## Next

Candidate T (ADR 0015): fuse requantization into the GEMM epilogue, analyse precomputation on epoch
weights, then wire T into the chain prototype and CPPminer.

Attestation (ADR 0014): a cheaper expansion PRF to tighten the certified rate, sustained-throughput
schedules, and a literature check against GPU-telemetry puzzles. Write-up: external review of
`docs/papers/tensor-pow-limits.md`. PoW candidates: no further work planned (E1 needs a GPU prover;
E2 is untested).
