# Research record

Experiment notes, newest first within each topic. Superseded and withdrawn notes are kept unchanged
below a status block that says why; raw data lives in an ignored `artifacts/` and each note records
its source hashes and environment.

| Topic | Current note | Superseded / withdrawn |
|---|---|---|
| **Candidate T / TNet v1: frozen parameters, verifier, robustness (ADR 0016)** | [tnet-v2](tnet-v2.md) | — |
| TNet v1 on Ampere (RTX 3090): attempt cost, parity | [tnet-ampere-v1](tnet-ampere-v1.md) | — |
| Candidate T: first test (ADR 0015) | [tnet-v1](tnet-v1.md) | — (v2 extends it; verification figures superseded by v2) |
| int8 tensor-core matmul (candidate A8) | [int8-matmul-v1](int8-matmul-v1.md) | — |
| A8 succinct-proof cost (test 1) | [a8-proof-cost-v1](a8-proof-cost-v1.md) | — |
| A8 escape E1 (winner-only STARK of the hash) | [e1-hash-proof-v1](e1-hash-proof-v1.md) | — |
| Prior art (Pearl, Nockchain, Aleo, PoNW, verifiable inference) | [prior-art-v1](prior-art-v1.md) | — |
| Interactive tensor-throughput attestation (ADR 0014) | [attest-v1](attest-v1.md) | — |
| GPU matmul, gathered reads, A' attempt folds | [gpu-suite-v1](gpu-suite-v1.md) | [gpu-baseline-v1](gpu-baseline-v1.md) (cold clock), [gather-bandwidth-v1](gather-bandwidth-v1.md) (64 KiB only, single launch), [attempt-rate-v1](attempt-rate-v1.md) (**withdrawn**: linear fold), [attempt-rate-v2](attempt-rate-v2.md) (attack vs naive kernel) |
| A' balance (gather vs matmul) | [memhard-balance-v2](memhard-balance-v2.md) | [memhard-balance-v1](memhard-balance-v1.md) (bandwidth-only model, cold rate) |
| Verifier cost | [verifier-throughput-v2](verifier-throughput-v2.md) | [verifier-throughput-v1](verifier-throughput-v1.md) (`k = 128` profile) |
| Chain prototype, sync, pool | [chain-prototype-v2](chain-prototype-v2.md) | [local-prototype-v1](local-prototype-v1.md), [multi-node-testnet-v1](multi-node-testnet-v1.md) (encoding v1) |
| GPU miner (CPPminer) | [cppminer-backend-v2](cppminer-backend-v2.md) | [cppminer-backend-v1](cppminer-backend-v1.md) (encoding v1, mislabelled A' table) |

Write-up for external readers: [`docs/papers/tensor-pow-limits.md`](../papers/tensor-pow-limits.md).

CPU probes (`scripts/`, outputs in `artifacts/`), re-run on 2026-10-09 with unchanged conclusions:

| Probe | Result |
|---|---|
| `instance_probe.py` | random dense matrices are full rank; a rank-`r` instance is `n / 2r` cheaper (measured 1.9–16.3x vs analytic 2–16x) — screening matters only if the miner can choose the instance |
| `omega_probe.py` | Strassen saves 1.14x (n = 128) to 2.23x (n = 4096) multiplications; work is `n^omega`, not `n^3` |
| `freivalds_forgery_probe.py` | one header-derived challenge is forgeable; Fiat–Shamir challenges bound to `(preheader, C)` reject the forgery |
| `freivalds_soundness_probe.py` | per-challenge error `~1/p` for `r` uniform in `F_p` (0.0098 at `p = 101`), `~1/2` for `r in {0,1}` |
| `mine_sim.py` | the Python reference mines and verifies a block with the chain's derivations |
| `memhard_balance_probe.py` | see `memhard-balance-v2` |
