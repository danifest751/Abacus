# Abacus — experimental Verifiable Algebra PoW

A research laboratory asking whether a **linear-algebra computation that is intrinsically
GPU-optimal** can support a standalone, permissionless proof of work with cheap probabilistic
verification. **There is no currency and no production network.** A negative result is a valid
outcome.

English is the primary repository language.

## Where this stands

- **Candidate T** — the current PoW candidate (`spec/07`, ADR 0015): a header-seeded int8 network on
  tensor cores whose output-row pieces are lottery tickets; a block is verified by recomputing one row
  (31 ms on a CPU); 87.8% of a mining attempt runs on tensor cores (`docs/research/tnet-v1.md`).
- **Candidate A** — header-bound Freivalds matmul PoW (`spec/03`): sound and implementable. The
  instance comes from the block header, so there is no screening or reuse; Fiat–Shamir Freivalds
  verification errs with probability `<= 2^-63` per challenge; a full block check costs ~19 ms at
  `n = 256` on one CPU core (about 1/5 of a naive CPU product), mostly instance expansion.
- **But** the linear algebra adds cost, not security or usefulness; dense modular matmul is
  ASIC-friendly; blocks carry `8 n^2` bytes. See **[the assessment](docs/ASSESSMENT.md)**.
- **Candidate A'** (gathered operands, `spec/04`): bandwidth-bound only with a nonlinear large-slice
  gather, which makes it an Ethash-class bandwidth PoW (ADR 0011).
- **Candidate A8** — int8 matmul on tensor cores (`spec/05`, ADR 0012), the current line of work: exact
  products at ~78 TMAC/s on a CMP 50HX (~440x the Goldilocks kernel). Test 1 (ADR 0013): proving the
  product succinctly on every attempt costs more than the product; two escapes remain untested.
- **Candidates B (NTT/sumcheck) and C (MSM/KZG)**: verifiers only; weaker premises (ADR 0003).
- **Tensor-throughput attestation** (`spec/06`, ADR 0014) — the active track: an interactive
  commit-then-sample proof that a GPU did a stated amount of exact int8 matmul work; certifies 40–49
  TMAC/s on a CMP 50HX with millisecond verification (`crates/abacus-attest`, `cuda/attest_prover.cu`).
- Write-up of the PoW findings: [`docs/papers/tensor-pow-limits.md`](docs/papers/tensor-pow-limits.md).

## What is implemented

- Independent **Python and Rust verifiers**: Goldilocks field, Freivalds with Fiat–Shamir challenges,
  NTT, Fiat–Shamir sumcheck; all consensus derivations cross-checked (`tests/test_chain_parity.py`).
- A **chain prototype** (`crates/abacus-chain`): committed and enforced difficulty, median-time-past
  timestamps, greatest-work fork choice, bounded pull sync, a solo/pool protocol; a GPU miner
  (CPPminer `--algo abacus`) mines into it.
- **GPU benches** with a reproducible suite (`scripts/gpu_suite.sh`) and CPU probes (`scripts/`).

## Run locally

Python 3.12 and Rust 1.98 (no third-party crates; Python tests need `pytest`):

```sh
python scripts/check.py          # pytest, rustfmt, clippy -D warnings, cargo test, self-test
bash scripts/gpu_suite.sh        # on a CUDA host: matmul, gathered reads, A' attempt folds
```

Raw experiment output goes to an ignored `artifacts/`.

## Layout

```
spec/        specifications (lab, sumcheck/NTT, candidate A, candidate A')
reference/   Python reference (field, Freivalds, chain derivations, NTT, sumcheck, A' dataset)
crates/      Rust: abacus-verifier (verifiers, adapters, bench), abacus-chain (prototype, node, miner),
             abacus-attest (tensor-throughput attestation: verifier, reference prover)
cuda/        GPU benches (Goldilocks and int8 matmul, NTT, gathered reads, A' attempt, candidate T)
             and the attestation prover
scripts/     check gate, GPU suite, probes
tests/       Python tests incl. Python/Rust differential and parity
docs/        assessment, critical path, status, threat model, decisions (ADR 0001-0015), research record,
             papers
```

## Documentation

- [Assessment](docs/ASSESSMENT.md) — the current answer to the central question.
- [Critical path](docs/CRITICAL-PATH.md) · [Status](docs/STATUS.md) · [Threat model](docs/THREAT-MODEL.md)
  · [Review guide](docs/REVIEW.md) · [Research plan](01-ABACUS-RESEARCH-PLAN.md)
- Specifications: [`spec/01`](spec/01-abacus-lab-v1.md), [`spec/02`](spec/02-sumcheck-ntt-v1.md),
  [`spec/03`](spec/03-header-bound-freivalds-matmul-pow.md), [`spec/04`](spec/04-memory-hard-freivalds-matmul-pow.md),
  [`spec/05`](spec/05-int8-matmul-pow.md), [`spec/06`](spec/06-tensor-throughput-attestation.md),
  [`spec/07`](spec/07-deep-int8-network-pow.md).
- Decisions: [`docs/decisions/`](docs/decisions/) (ADR 0001–0015). Research record:
  [`docs/research/`](docs/research/README.md).

## Scope boundary

Freivalds and sumcheck verify a claimed result, not that a miner did the work. Nothing here
establishes usefulness, ASIC resistance, post-quantum security or mainnet parameters.

## License

MIT. See [LICENSE](LICENSE).
