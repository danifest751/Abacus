# Attempt rate v2 — candidate A' gather folds and the prefix-sum attack (CMP 50HX)

Date: 2026-10-09. Bench: `cuda/attempt_bench.cu` v2 (sha256 `f676e653c7cc0a91…`), CMP 50HX (sm_75,
driver 610.43.03, CUDA 13.3), SM clock 1950 MHz under load, memory 7000 MHz, 118–220 W. Each run:
3 s warm-up, then CUDA-event timing of `attempts` gathers, `attempts` matmuls and `attempts` full
attempts. Dataset: pseudo-random fill (v1 used a constant `0x5A`). Three repeats per row; the spread
is below 2%, so the median is shown. Raw JSON lines: `artifacts/attempt-rate-v2-20261009.jsonl`
(not in Git).

Folds (`fold` argument):

- **mix** (v2): `acc = mix64(acc ^ word)` over the segment — sequential, nonlinear; every word must be
  read.
- **sum** (v1): `acc += word` — linear.
- **prefix**: the attack on `sum`. A per-epoch prefix table gives any segment sum from **two reads**
  (`pre[off+seg] - pre[off]`), so the attacker reads 16 bytes per entry instead of `seg`.

| n | seg B | fold | attempts/s | gather ms | matmul ms | gather/matmul | read MB/attempt | gather GB/s |
|---:|---:|---|---:|---:|---:|---:|---:|---:|
| 256 | 2560 | mix | 348 | 2.765 | 0.093 | 29.7 | 167.8 | 60.7 |
| 256 | 2560 | sum | 348 | 2.760 | 0.094 | 29.5 | 167.8 | 60.8 |
| 256 | 2560 | **prefix** | **6731** | 0.050 | 0.096 | 0.52 | 1.0 | 21.1 |
| 256 | 65536 | mix | 14.5 | 69.18 | 0.095 | 728 | 4295 | 62.1 |
| 256 | 65536 | sum | 14.1 | 70.98 | 0.095 | 747 | 4295 | 60.5 |
| 256 | 65536 | **prefix** | **6729** | 0.050 | 0.096 | 0.52 | 1.0 | 21.0 |
| 512 | 2560 | mix | 77.3 | 12.36 | 0.694 | 17.8 | 671.1 | 54.3 |
| 512 | 2560 | sum | 79.1 | 12.08 | 0.697 | 17.3 | 671.1 | 55.6 |
| 512 | 2560 | **prefix** | **1053** | 0.187 | 0.740 | 0.25 | 4.2 | 22.4 |

## Findings

- **The v1 design is broken by a precomputation.** Against the linear `sum` fold, the prefix-sum
  attacker is **19x** faster at `n=256, seg=2560`, **466x** at `n=256, seg=64 KiB` and **14x** at
  `n=512, seg=2560`, and its attempt becomes **matmul-bound** (gather/matmul 0.25–0.52). The v1
  conclusion "memory-hardness is achieved" (`attempt-rate-v1.md`) is refuted by measurement.
- **The nonlinear fold costs the honest miner nothing.** `mix` and `sum` run at the same rate (both are
  bandwidth-bound), so the fix has no honest-side penalty in this kernel.
- **With the nonlinear fold, the gather dominates**: 18–730x the matmul time. This is gather-bound,
  but it is a **naive** gather: one thread per entry reading its segment sequentially (uncoalesced),
  ~55–62 GB/s. A warp-cooperative gather reaches ~416 GB/s (`gather-bandwidth-v1.md`), i.e. ~7x
  faster, which would put `n=256, seg=2560` at about 0.4 ms gather vs 0.09 ms matmul — still
  gather-dominated, but by a much smaller factor. The tuned-miner rate is **not** measured here.
- **v1 was cold-clock.** Warm, `n=256, seg=2560` runs at 348 attempts/s versus 69 reported in v1, and
  `n=512` is now slower than `n=256` (77 vs 348), as expected; v1's inversion was the clock artifact.
- The matmul here is 0.093 ms at `n=256` (~180 GMAC/s), consistent with the warm re-measurement in
  `gpu-baseline-v1.md` (166–204 GMAC/s), not with the 44 GMAC/s cold figure.

## Caveats

- `B` is a constant fill and there is no score hash or instance expansion in the loop; only the
  gather and the matmul are timed.
- The segment fold is a bench construction; the prototype A' (`crates/abacus-chain`, CPPminer) gathers
  one 8-byte word per entry and has no segment fold.
- Not memory-hardness evidence for a tuned miner: no cooperative gather, no ASIC model, no
  time-memory trade-off analysis of the dataset.

## Reproduce

```
nvcc -O3 -arch=sm_75 cuda/attempt_bench.cu -o attempt_bench
for cfg in "256 2560 2048 64" "256 65536 2048 16" "512 2560 2048 32"; do
  for fold in 0 1 2; do ./attempt_bench $cfg $fold 3; done
done
```
