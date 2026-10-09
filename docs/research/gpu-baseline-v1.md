# GPU baseline v1 — Goldilocks matmul (candidate A)

Date: 2026-10-09. Host: local server, **CMP 50HX** (Turing, sm_75), CUDA 13.3. Kernel:
`cuda/goldilocks_matmul_bench.cu` (naive 16x16 tiled shared-memory matmul over Goldilocks,
`P = 2**64 - 2**32 + 1`). CPU baseline: naive triple loop, `unsigned __int128` modular reduce.

## Method

- Random dense `A, B` (uniform `< P`), row-major `n x n`, `C = A*B` over Goldilocks.
- One timed kernel launch after a warm-up; CPU timed separately.
- Correctness: at `n = 512` every CPU entry equals the GPU entry (`cpu_matches_gpu = true`).
- MAC = one Goldilocks multiply + add; throughput in GMAC/s.

## Results

| n | GPU s | GPU GMAC/s | CPU s | CPU GMAC/s | GPU / CPU |
|---:|---:|---:|---:|---:|---:|
| 512 | 0.002104 | 63.8 | 0.873 | 0.1537 | **415x** |
| 1024 | 0.024008 | 44.7 | — | — | — |
| 2048 | 0.194821 | 44.1 | — | — | — |

## Findings

- The GPU is **~415x** the naive CPU at n=512 for field matmul, and sustains ~44 GMAC/s even with a
  **naive, non-tensor-core** kernel. This corroborates §2 of `01-ABACUS-RESEARCH-PLAN.md`: for
  candidate A the GPU advantage is not a hypothesis.
- The GPU throughput is roughly flat (44-64 GMAC/s) across n; the CPU is memory/compute bound in
  pure Python-free C at ~0.15 GMAC/s.

## Caveats

- The GPU kernel is deliberately simple; a tensor-core int8 GEMM would be far faster, so 44 GMAC/s is
  a **floor**, not a tuned number. The CPU baseline is a naive loop, not BLAS — the 415x is
  illustrative, not a rigorous hardware ranking.
- This is a **throughput baseline**, not a work model, miner or consensus. It makes no claim about
  difficulty, reuse or the existence of a work function.

## Next (GPU)

- A tensor-core matmul baseline and an NTT baseline, on the same matched CPU scope, to bound the
  "GPU-optimal" claim for both candidate A (matmul) and B (NTT).
- The decisive GPU work (D5) waits for a work model; this baseline only establishes the premise.

## Reproduce

```
nvcc -O3 -arch=sm_75 cuda/goldilocks_matmul_bench.cu -o gl_mm_bench
./gl_mm_bench 512
./gl_mm_bench 2048
```
