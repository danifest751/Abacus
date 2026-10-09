# Memory-hard balance v2 — candidate A' (gather vs matmul) from measured rates

Date: 2026-10-09. Status: **current**. Supersedes `memhard-balance-v1`. Probe:
`scripts/memhard_balance_probe.py` (JSON in `artifacts/memhard-balance-v2.json`).

## Model

Per attempt the miner gathers the `2 n^2` entries of `A, B` and computes `n^3` Goldilocks MACs.
Measured CMP 50HX rates (`gpu-suite-v1.md`, warm, medians):

| quantity | value | used for |
|---|---:|---|
| matmul | 177e9 MAC/s | `n^3 / rate` |
| random small reads (8–32 B) | 3.1e9 reads/s | prototype gather: `2 n^2` reads |
| cooperative reads (>= 2560 B segments) | 530e9 B/s | large-slice gather: `2 n^2 * seg` bytes |

The large-slice design assumes a **nonlinear** fold of each segment (a linear fold is removed by prefix
sums, ADR 0010). The model reproduces the measured `coop` ratios within 5% (`gpu-suite-v1.md`
finding 5).

## Results

| n | prototype gather / matmul | slice 2560 B gather / matmul | slice 64 KiB gather / matmul | balance segment |
|---:|---:|---:|---:|---:|
| 64 | 1.78 | 26.7 | 684 | 96 B |
| 256 | 0.45 | 6.7 | 171 | 383 B |
| 512 | 0.22 | 3.3 | 85 | 767 B |
| 1024 | 0.11 | 1.7 | 43 | 1533 B |

"Balance segment" is the segment size at which the cooperative gather equals the matmul time
(`n^3 * BW / (2 n^2 * rate)`; linear in `n`).

## Findings

- **The prototype A' (one 8-byte word per entry) is not memory-hard** for `n >= 256`: the gather is
  0.45x–0.11x of the matmul. This confirms ADR 0008 with measured rates.
- **A large-slice A' is gather-dominated** once the segment exceeds the balance size (hundreds of
  bytes to ~1.5 KiB for `n = 256..1024`). At `n = 256, seg = 2560 B` the gather is 6.7x the matmul;
  the per-attempt read is 336 MB (A and B).
- The design point trades attempt rate for bandwidth: the attempt is then dominated by DRAM traffic,
  i.e. an Ethash-like bandwidth PoW with a matmul attached. The matmul share shrinks as the segment
  grows, so the "linear algebra" part stops being the work.

## Open (not measured)

- Verifier cost: a node must hold the dataset (2 GiB here) and re-read 336 MB per block at
  `n = 256, seg = 2560 B` — about 17 ms at 20 GB/s on a CPU, before the Freivalds check.
- Time–memory trade-offs of the data-dependent dataset (checkpointing, pebbling).
- Other devices (HBM GPUs, ASIC models) and energy per attempt.
