# int8 matmul v1 — candidate A8 on the CMP 50HX tensor cores

Date: 2026-10-09. Status: **current**. ADR 0012, spec/05.

## Method

- Host `me4me`, CMP 50HX (sm_75), driver 610.43.03, CUDA 13.3, cuBLAS 13.6. During the run SM
  1950 MHz, memory 7000 MHz, 98–131 W, 56–59 °C.
- `cuda/int8_matmul_bench.cu` (sha256 `8db4ce33…`), run by `scripts/gpu_suite.sh` (run 5; the script was
  afterwards changed only in a newline escape). 3 s warm-up, median of 7 CUDA-event timings.
- Paths on the same random int8 matrices: **cuBLAS** `cublasGemmEx` (int8 in, int32 out,
  `CUBLAS_COMPUTE_32I`, tensor-core IMMA) and a plain tiled **CUDA-core** kernel. Exactness: compared
  with a CPU product for `n <= 512`; for every `n` the cuBLAS result passes two host Freivalds checks over
  Goldilocks, and a copy with one corrupted entry fails.
- **Hash of `C`**: SHA-256 over `C` (`4 n^2` bytes) in independent 1 KiB chunks, one GPU thread per
  chunk — a cost model for committing to `C` (no padding block, no tree combine).
- A separate ad-hoc run before the suite agreed within 3% (`artifacts/int8-run1.txt`). Raw:
  `artifacts/gpu-run5/int8.jsonl` (not in Git).

## Results

| n | tensor TMAC/s | CUDA-core TMAC/s | tensor / CUDA-core | GEMM ms | hash `C` ms | hash / GEMM | `C` size |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 256 | 1.64 | 0.481 | 3.4x | 0.0102 | 0.0553 | 5.40 | 256 KiB |
| 512 | 9.36 | 0.574 | 16.3x | 0.0143 | 0.0566 | 3.95 | 1 MiB |
| 1024 | 24.8 | 0.604 | 41.0x | 0.0434 | 0.0611 | 1.41 | 4 MiB |
| 2048 | 43.8 | 0.614 | 71.3x | 0.1963 | 0.1310 | 0.67 | 16 MiB |
| 4096 | 67.2 | 0.609 | 110.3x | 1.0229 | 0.3692 | 0.36 | 64 MiB |
| 8192 | 77.7 | 0.607 | 127.9x | 7.0796 | 1.3782 | 0.20 | 256 MiB |

Every row: cuBLAS result exact (Freivalds-checked; CPU-checked for `n <= 512`), corrupted copy rejected.
For comparison, the Goldilocks kernel in the same run: 163.5–177.0 GMAC/s.

## Findings

1. **Tensor cores change the scale of the work**: ~78 TMAC/s (~155 TOPS) at `n = 8192`, ~440x the
   Goldilocks kernel and ~128x int8 on CUDA cores. Exact integer results are verified with the existing
   Goldilocks Freivalds machinery.
2. **The commitment to `C` is the bottleneck at small `n`.** Hashing `C` costs more than the GEMM below
   `n ~ 1500` and still 20% at `n = 8192`; a faster tree hash would move the crossover, but not its
   `n^2 / n^3` scaling.
3. **Shipping `C` is infeasible** where the GEMM dominates (16–256 MiB per block). A8 needs a succinct
   argument for `C = A * B` (ADR 0012), which is the next experiment.

## Caveats

- One GPU; cuBLAS heuristics choose the kernel; no energy measurement; no tuned CPU int8 GEMM (the CPU
  check column is a naive loop, ~11.8 GMAC/s single-threaded on the host's Ryzen 5 5500).
- The hash timing is a lower-bound cost model; a real commitment adds padding and a tree combine.
