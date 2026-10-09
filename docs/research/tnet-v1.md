# Candidate T v1 — deep requantized int8 network PoW: attempt cost, verification, ticket cost

Date: 2026-10-09. Status: **current** for the GPU results; the CPU verification figures are
superseded by the transposed SIMD verifier of `tnet-v2`. spec/07, ADR 0015.

## Setup

- GPU: `cuda/tnet_bench.cu` (sha256 `0b010e05…`) on the CMP 50HX (sm_75, driver 610.43.03, CUDA 13.3,
  cuBLAS 13.6); 3 s warm-up, median of 15 attempts per configuration, `w = 256`, `M = round(2^24 / (74
  sqrt(n)))`. Raw: `artifacts/tnet-run1/gpu.jsonl` (not in Git).
- CPU verifier: `abacus-tnet bench` (Rust reference `tnet.rs` sha256 `4cfcb330…`), release build,
  AMD Ryzen 7 8745HS. Raw: `artifacts/tnet-run1/verify-cpu.jsonl`.
- Parity: a ticket found by the GPU at `n = 256, b = 64, L = 4` is recomputed by the Rust reference:
  identical bytes, 11 leading zero bits (target 10).

## Attempt cost on the GPU

| n | b | L | attempt ms | GEMM | requant | expand | hash | tensor share | attempt TMAC/s | ns / ticket |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 4096 | 4096 | 8 | 10.99 | 8.68 | 1.32 | 0.65 | 0.35 | 79.0% | 50.0 | 168 |
| 4096 | 4096 | 16 | 21.09 | 17.45 | 2.63 | 0.66 | 0.35 | 82.7% | 52.1 | 322 |
| 8192 | 8192 | 8 | 74.01 | 64.97 | 5.20 | 2.57 | 1.34 | **87.8%** | 59.4 | 282 |
| 8192 | 4096 | 8 | 37.14 | 32.55 | 2.62 | 1.29 | 0.68 | 87.6% | 59.2 | 283 |
| 8192 | 8192 | 4 | 39.00 | 32.53 | 2.60 | 2.55 | 1.34 | 83.4% | 56.4 | 149 |

Activations of `X_L`: 0.7% zeros, 0.9–3.9% saturated, mean `|x|` 39–48 (all eight bits in use). With a
power-of-two scale instead of `M` (first run) the same layers saturated 15% (`n = 4096`) or collapsed
to mean `|x|` 11 (`n = 8192`).

Lottery: tickets at 20 bits found 1, 1, 3, 2, 4 against 0.9, 0.9, 3.8, 1.9, 3.8 expected.

## Ticket cost by strategy (same `n`, `L = 8`)

| n | b | ns / ticket | relative |
|---:|---:|---:|---:|
| 4096 | 4096 | 168 | 1x |
| 4096 | 256 | 282 | 1.7x |
| 4096 | 4 (single rows) | 7,550 | 45x |
| 8192 | 8192 | 282 | 1x |
| 8192 | 4 (single rows) | 11,782 | 42x |

Computing few rows (GEMV-like) is far more expensive per ticket than batching; no strategy found
beats the honest batched forward pass.

## Verification on a CPU (one row through all layers)

| n | L | 1 thread | 8 threads | epoch weights (once per epoch) |
|---:|---:|---:|---:|---:|
| 4096 | 8 | 24.1 ms | 11.2 ms | 1.3 s, 128 MiB |
| 8192 | 8 | 102.6 ms | 30.7 ms | 5.0 s, 512 MiB |

## Findings

1. The test criteria of ADR 0015 are met at `n = 8192, L = 8`: 87.8% of an attempt on tensor cores,
   31 ms verification on 8 CPU threads (103 ms single-threaded), no cheaper ticket path among those
   measured, a fair lottery, healthy activations.
2. At `n = 4096` the separate requantization pass (12%) keeps the tensor share at 79–83%; fusing it into
   the GEMM epilogue is the main remaining optimisation.
3. Block payload is `(nonce, i, c)`: a few bytes, against megabytes for every Freivalds-based design.

## Caveats

- One GPU; cuBLAS kernels; the CPU verifier is a straightforward Rust loop (a SIMD int8 dot product would
  be several times faster).
- Precomputation attacks on fixed epoch weights are argued, not measured (spec/07 §4).
