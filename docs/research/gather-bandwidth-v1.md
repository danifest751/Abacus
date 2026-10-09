# Gathered-read bandwidth v1 — candidate A' premise (corrected, warm)

Date: 2026-10-09. Bench: `cuda/gather_bench.cu` on the CMP 50HX (sm_75). Random 64 KiB segment reads
from a 256 MiB buffer versus a sequential streaming read.

**Correction.** A first (cold) run reported gather ~29 GB/s and sequential ~21 GB/s, from which an
earlier draft wrongly concluded that gathered access is ~6% of sequential bandwidth. Re-running gave
stable warm numbers: the cold runs were a **clock-ramp artifact** of the CMP idle governor.

## Warm results (256 MiB dataset, 64 KiB segments, four runs)

| run | gather GB/s | sequential GB/s | gather / seq |
|---:|---:|---:|---:|
| 1 (cold) | 25.0 | 21.5 | 1.16 |
| 2 | 415.8 | 474.7 | 0.876 |
| 3 | 416.1 | 477.0 | 0.872 |
| 4 | 413.8 | 473.6 | 0.874 |

## Findings

- Warm, **large-segment (64 KiB) gathered reads reach ~416 GB/s, ~87% of the ~475 GB/s sequential
  rate** — the gather is efficient, not pattern-starved, at this segment size.
- The cold run under-reported both by ~20x; **clock state dominates the absolute numbers**. Definite
  numbers need a locked clock or a long warm-up.
- This inverts the memory-hard intuition: because gathered bandwidth is high, a memory-hard layer needs
  a **very large** per-attempt gather (GB-scale) to dominate the matmul — see
  `memhard-balance-v1.md`. Small gathers are compute-dominated.
- Smaller segment sizes (8 KiB) were not measured warm; they may be less efficient and are the next
  check.

## Reproduce

```
nvcc -O3 -arch=sm_75 cuda/gather_bench.cu -o gather_bench
./gather_bench 256 64 64   # run several times; take the warm runs
```
