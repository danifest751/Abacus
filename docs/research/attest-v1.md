# Tensor-throughput attestation v1 — measurements (CMP 50HX)

Date: 2026-10-09. Status: **current**. spec/06, ADR 0014.

## Setup

- Prover: `cuda/attest_prover.cu` (sha256 `889c3e63…`), `nvcc -O3 -arch=sm_75 -lcublas`, on the lab GPU host
  (CMP 50HX, driver 610.43.03, CUDA 13.3, cuBLAS 13.6), sequential mode.
- Verifier: `abacus-attest-verify` (crate sources `lib.rs bd0287a7…`, `net.rs 9b4cd80b…`), release build,
  on the same host (loopback) and on a Windows PC over the LAN (Ryzen 7 8745HS).
- `k = 32` opened rows; 3 rounds per configuration after two warm-up rounds. Raw:
  `artifacts/attest-run1/` (not in Git).

## Honest prover (loopback)

| n | m | MACs | prove ms (3 rounds) | proven TMAC/s | GEMM-only TMAC/s | expand / GEMM / leaf / Merkle ms | open ms | verify ms | precompute s |
|---:|---:|---:|---|---:|---:|---|---:|---:|---:|
| 4096 | 16 | 1.1e12 | 45.3 / 45.3 / 44.1 | 24.3–24.9 | 67.5–68.8 | 14.8 / 16.1 / 9.5 / 1–3 | 2.5 | 6.7 | 3.8–4.1 |
| 8192 | 4 | 2.2e12 | 51.6 / 53.8 / 55.5 | 39.6–42.6 | 75.9–80.0 | 14.8 / 28.4 / 7.6 / 0.7 | 4.0–4.5 | 12.7–14.5 | 3.8 |
| 8192 | 8 | 4.4e12 | 109.4 / 110.0 / 110.3 | 39.9–40.2 | 73.8–74.2 | 31.5 / 59.4 / 15.6 / 1.0 | 3.9–4.4 | 12.8 | 7.5 |
| 16384 | 2 | 8.8e12 | 185.0 / 181.3 / 180.0 | 47.6–48.9 | 66.1–69.3 | 35.7 / 129.1 / 14.1 / 0.7 | 7.6–11.7 | 25.6–26.0 | 7.5–8.1 |
| 16384 | 4 | 1.8e13 | 363.5 / 369.7 / 365.7 | 47.6–48.4 | 68.1–68.4 | 74.7 / 258.0 / 28.1 / 1.1 | 6.7–11.0 | 25.0–25.5 | 15.1–16.3 |

Every round accepted (32/32 rows). "Proven" divides the certified MACs by the verifier-measured
CHAL→ROOT time; "GEMM-only" is the prover's own event timing of the cuBLAS calls.

## Over the LAN (verifier on a separate PC)

| n | m | prove ms | proven TMAC/s | open ms | verify ms |
|---:|---:|---|---:|---:|---:|
| 8192 | 4 | 53.7 / 53.1 / 53.4 | 41.0–41.4 | 19.1–19.3 | 12.1–12.3 |
| 16384 | 2 | 179.7 / 180.5 / 180.4 | 48.7–49.0 | 36.8–37.3 | 23.3–24.8 |

The first challenge after the GPU had been idle measured 408 ms (5.4 TMAC/s): the idle governor; a
warm-up challenge is part of the procedure.

## Cheating prover (`--cheat-rows`, `n = 4096`, `m = 4`, `k = 32`)

The prover commits zeros instead of computing the last rows of every product. Two runs of 40 rounds
each (`cheat-run0.jsonl` with the previous build, `cheat.jsonl` with the final one):

| skipped rows | f | passed / 80 | expected (sampling bound) | mean correct rows / 32 |
|---:|---:|---:|---:|---:|
| 41 | 1.0% | 56 | 58.0 | 31.5–31.9 |
| 205 | 5.0% | 16 | 15.5 | 30.5–30.7 |
| 410 | 10.0% | 0 | 2.7 | 27.9–29.1 |

Detection matches the bound `(1 - f)^k`. A skipped row is always caught when opened (the secret
Freivalds check rejects it).

## Findings

1. The protocol works end to end on real hardware: exact int8 GEMM on tensor cores, a 32-byte
   commitment instead of 1–4 GiB of products, online verification in 7–26 ms, a few MiB of traffic.
2. It certifies **40–49 TMAC/s** at `n >= 8192` — 53–71% of the GEMM's own rate. The remainder is the
   SHA-256 instance expansion (~20% at `n = 16384`) and row hashing (~8%); a two-stream pipeline did
   not help (+16% at `n = 4096`, none at `n >= 8192`: the GEMM occupies every SM).
3. Cheating is caught at the predicted rate; `k = 128` would catch `f = 5%` with 99.9%.

## Next

- A cheaper PRF for the expansion (e.g. ChaCha) to bring the certified rate closer to the GEMM rate,
  and to shorten the verifier's precompute (3.8 s for `m n^2 = 2.7e8` bytes with the in-house SHA-256).
- Sustained-throughput schedules (repeated challenges), and a literature check against GPU-telemetry
  puzzles before any novelty claim.
