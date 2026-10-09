# Abacus Network — verifiable GPU-algebra proof of work

Private research into whether a **linear-algebra computation that is intrinsically GPU-optimal**
can support standalone proof of work. This repository currently implements a reference
verifier (Freivalds and sumcheck) and studies candidate PoW constructions. **There is no
currency, production network or GPU miner.**

**English is the primary repository language.**

## The one open question

Can a permissionless PoW be built from a linear, GPU-optimal computation (matmul, NTT, MSM)
with **cheap probabilistic verification**, while resisting decomposition, precomputation and
reuse? Linearity is the enemy, so a **nonlinear anchor** is the central research problem. A
negative result is a valid outcome.

## Implemented

- **Verifiers** in Python and Rust, built independently and checked against each other: `Freivalds`
  matrix-product verification over the Goldilocks field; NTT; the sumcheck protocol;
  SHA-256; and `k`-challenge Fiat-Shamir-bound Freivalds (ADR 0004, 0009). Every consensus
  derivation is compared Python vs Rust (`tests/test_chain_parity.py`).
- **Chain prototype** (`crates/abacus-chain`): mine and verify blocks; bit-count difficulty; an
  ancestor-derived, **enforced and committed** retarget with median-time-past timestamps;
  greatest-cumulative-work fork choice; and an optional A' layer (a data-dependent epoch dataset
  with header-random gathered operands, spec/04).
- **Multi-node P2P prototype** (`p2p.rs`, `bin/abacus-node.rs`): pull sync, greatest-work adoption and
  reorg rollback; a three-process demo converges.
- **Falsifier probes and decisions**: instance-structure collapse, work exponent, Freivalds forgery,
  Freivalds soundness, memory-hard balance, and GPU attempt rate, with records in `docs/decisions/`.
- **GPU baselines** (`cuda/`): Goldilocks matmul CPU vs GPU, gathered-read bandwidth, and the A'
  attempt rate on the CMP 50HX.
- One local check command (`python scripts/check.py`).

The ordinary check gate uses standard libraries only. Cargo builds offline without third-party
crates. GitHub Actions are intentionally absent; checks run locally.

## Where this stands

- **Candidate A** (header-bound Freivalds matmul PoW) is a **positive result**: sound (with `k`
  Fiat-Shamir challenges), simple, GPU-optimal; recorded downsides are ASIC-friendliness and no
  usefulness (ADR 0005).
- **Candidate A'** (gathered operands over an epoch dataset) is **not shown to be memory-hard**: with
  one block per entry the matmul dominates (ADR 0008), and the large-slice attempt-rate result was
  withdrawn because its linear segment fold collapses under prefix sums — a measured 14–466x
  attacker advantage on the CMP (ADR 0010, `docs/research/attempt-rate-v2.md`).
- **Candidate B** (NTT/sumcheck) is a secondary study; **C** (MSM/KZG) is a not-post-quantum reference
  (ADR 0003).
- Design bugs found and fixed: a single-challenge Freivalds forgery (ADR 0004), an `O(k*n^3)`
  Fiat-Shamir derivation blow-up (ADR 0006), an over-pessimistic soundness bound that forced
  `k = 128` and large `n` (ADR 0009), and the consensus, A' and sumcheck issues of the 2026-10-09
  review (ADR 0010).
- The local multi-node gate (E6) passes; no external testnet, consensus hardening or audit exists yet.

## Run locally

Tested with Python 3.12.10 and Rust 1.98.1 on Windows.

```sh
python scripts/check.py
```

This runs formatting-independent Python tests and the Rust workspace tests, plus the
deterministic Python/Rust differential corpus. Experiments write to an ignored `artifacts/`.

## Documentation

- [Research plan](01-ABACUS-RESEARCH-PLAN.md) — objective, candidates, test plan, gates.
- [Critical path](docs/CRITICAL-PATH.md) — the single open decision and decisive experiments.
- [Status](docs/STATUS.md) — what is implemented.
- [Threat model](docs/THREAT-MODEL.md) — linearity, precomputation, reuse, resources, quantum and ASIC posture.
- Specifications: [`spec/01`](spec/01-abacus-lab-v1.md) (lab), [`spec/02`](spec/02-sumcheck-ntt-v1.md)
  (sumcheck/NTT), [`spec/03`](spec/03-header-bound-freivalds-matmul-pow.md) (candidate A),
  [`spec/04`](spec/04-memory-hard-freivalds-matmul-pow.md) (candidate A').
- Decisions: `docs/decisions/0001`-`0010` (research boundary, work functions, verification cost,
  Freivalds binding, candidate A, Fiat-Shamir derivation, ASIC posture, A' balance, Freivalds
  soundness, review fixes).
- Research notes: `docs/research/` (GPU baselines, verifier throughput, memory-hard balance and
  attempt rate, local prototype, multi-node testnet).
- [Language policy](docs/LANGUAGE.md).

## Scope boundary

Abacus is a research laboratory, not a production PoW or monetary blockchain. Freivalds and
sumcheck **verify a claimed result**, not that a miner did the work; that gap is the research.
Small-instance results do not establish GPU advantage, ASIC resistance, post-quantum security
or mainnet parameters. Use "experimental Verifiable Algebra PoW" until specific claims have
adequate evidence.

## Credits and license

MIT. See [LICENSE](LICENSE).
