# GPU baseline v1 — Goldilocks matmul (candidate A)

> **Status: superseded** (2026-10-09) by [gpu-suite-v1](gpu-suite-v1.md). This run timed one launch on a cold GPU: the CMP idle governor kept the clocks low, so the
> 44-64 GMAC/s are ~4x low (warm: 168-179 GMAC/s). The "D5" section is not a D5 result: one naive
> single-threaded CPU point, no energy measurement. "A tensor-core int8 GEMM would be far faster" does
> not apply to 64-bit Goldilocks arithmetic (ADR 0008).
>
> The original record follows unchanged.

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

## D5 — matched CPU/GPU work rate (candidate A)

Same field (Goldilocks), same machine (CMP 50HX), same kernel family (dense `n x n` product):

- n = 512: GPU 63.8 GMAC/s vs **naive CPU 0.154 GMAC/s → ~415x**.
- The GPU rate is roughly flat (44-64 GMAC/s) across n; the CPU rate is ~0.15 GMAC/s.

Interpretation: the GPU advantage is a **constant factor**, not a superlinear break, so the work
function stays **portable** across CPU and GPU (the D5 threat — a memory-rich device winning
superlinearly — does not fire for dense matmul). Caveat: the CPU baseline is naive; BLAS/Strassen
would narrow the constant, and Strassen also lowers the work exponent (see `omega_probe.py`), but
neither is superlinear in the way that would break monotonicity.

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
