# Gathered-read bandwidth v1 — candidate A' premise

Date: 2026-10-09. Bench: `cuda/gather_bench.cu` on the CMP 50HX (sm_75). Random 8/64 KiB segment
reads from a large buffer versus a sequential streaming read. Purpose: check whether a memory-hard,
header-random **gather** layer (ADR 0007) can shift the bottleneck from multiply throughput to memory
access pattern.

## Results

| dataset | segment | gather GB/s | sequential GB/s | gather / seq |
|---:|---:|---:|---:|---:|
| 256 MiB | 8 KiB | 22.1 | 21.5 | 1.03 |
| 512 MiB | 8 KiB | 22.0 | 22.0 | 1.00 |
| 256 MiB | 64 KiB | 28.9 | 469.6 | 0.06 |

## Findings

- **Gathered random reads are far slower than sequential streaming**: at 64 KiB segments the gather
  reaches ~29 GB/s versus ~470 GB/s for a contiguous read (≈6%). The gathered pattern is
  **access-pattern bound**, not raw-bandwidth bound — exactly the property a memory-hard layer wants,
  because an ASIC then needs the same random-access capability, not just wide DRAM.
- The 8 KiB rows show both gather and sequential near ~22 GB/s; those runs were early (cold GPU). The
  64 KiB run (later, warmer) shows the real contrast. **Clock state dominates the absolute numbers**:
  the idle governor ramps the CMP under load, so a locked-clock or longer warm-up run is needed for
  trustworthy absolute bandwidth.

## Caveat and next

- Re-run with a warm-up pass and (if possible) locked clocks before quoting absolute GB/s.
- This only establishes that a gathered layer is pattern-bound; the full candidate A' must still be
  designed (dataset construction, index derivation, how `A,B` are assembled) and its falsifiers tested
  (monotone work in bytes fetched; no cross-attempt caching; verifier dataset cost).

## Reproduce

```
nvcc -O3 -arch=sm_75 cuda/gather_bench.cu -o gather_bench
./gather_bench 256 64 64
```
