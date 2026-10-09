# TNet v1 on a second GPU architecture (RTX 3090, Ampere)

Date: 2026-10-09. Status: **current**. spec/07, ADR 0016. Extends `tnet-v1` / `tnet-v2` (Turing).

## Setup

- GPU: NVIDIA GeForce RTX 3090 (Ampere, sm_86, 24 GiB, 350 W limit), driver 570.211.01, CUDA 12.8;
  a rented VM provided by the owner. Same source as the Turing runs: `cuda/tnet_bench.cu` (sha256
  `0b010e05…`), `nvcc -O3 -arch=sm_86 -lcublas`; 3 s warm-up; median of 7 (frozen) or 15 attempts.
- `epoch = 00..1f`, `hd = 20..3f` as in `spec/vectors/`. Raw: `artifacts/tnet-run3/gpu-3090.jsonl`
  (sha256 `b6929fb4…`), `smi-3090.csv` (1 s samples; not in Git).
- During the runs the board was power-limited: median 328 W (max 346 W), median SM clock 1,628 MHz.

## Attempt cost

| GPU | n | b | attempt ms | GEMM | requant | expand | hash | tensor share | attempt TMAC/s | GEMM TMAC/s | ns / ticket |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| RTX 3090 | 8192 | 65536 | 334.5 | 290.1 | 23.9 | 13.6 | 6.8 | 86.7% | 105.2 | 121.3 | **159.5** |
| RTX 3090 | 8192 | 8192 | 41.6 | 35.9 | 3.0 | 1.8 | 0.9 | 86.3% | 105.8 | 122.6 | 158.6 |
| RTX 3090 | 4096 | 4096 | 6.6 | 5.1 | 0.8 | 0.5 | 0.2 | 77.8% | 83.1 | 106.8 | 101.0 |
| CMP 50HX (`tnet-v2`) | 8192 | 65536 | 611.6 | 539.6 | 41.2 | 20.1 | 10.7 | 88.2% | 57.5 | 65.2 | 292 |

Lottery: 902 tickets at 14 bits against 896 expected (frozen), 4 at 20 bits against 3.75. `X_L`
statistics are identical to the Turing run (0.73% zeros, 2.08% saturated, mean `|x|` 43.45), as they
must be for identical inputs.

## Ticket cost by strategy (RTX 3090, `L = 8`)

| n | b | ns / ticket | relative |
|---:|---:|---:|---:|
| 8192 | 65536 | 159.5 | 1x |
| 8192 | 256 | 253 | 1.6x |
| 8192 | 4 (single rows) | 8,120 | 51x |
| 4096 | 4096 | 101 | 1x |
| 4096 | 4 (single rows) | 5,200 | 51x |

## Parity

The 3090's sample tickets are accepted by the Rust references with identical pieces: `(nonce 0,
i 1751, c 18)` at the frozen parameters (16 leading zero bits), `(1, 6693, 12)` at `b = 8192` (20 bits),
both via `tnet check` of the coin repository, and `(10, 521, 2)` at `n = 4096` via `abacus-tnet check`.

## Findings

1. Parity holds on a second architecture and CUDA version (Turing/CUDA 13.3, Ampere/CUDA 12.8).
2. The 3090 mines 1.83x faster per ticket than the CMP 50HX (159.5 vs 292 ns) at ~328 W, about 52 µJ
   per ticket (board power, coarse sampling).
3. Faster tensor cores raise the share of the non-GEMM phases: tensor share 86.7% against 88.2%;
   requantization is 7.2% of an attempt. A fused requantization epilogue is worth more on newer GPUs
   (miner optimisation; consensus unchanged).
4. Batching stays decisive: single-row mining costs 51x per ticket (42–45x on Turing).

## Caveats

- One board, power-limited at its 350 W setting; a VM (no control over clocks). cuBLAS default
  algorithms; no fused epilogue.
