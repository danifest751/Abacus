# ADR 0011 — A' measured: the one-word gather is compute-bound; a nonlinear large-slice gather is bandwidth-bound

Status: accepted research result (2026-10-09). Amends ADR 0007 and 0008 with warm, repeated
measurements (two suite runs, median of 7 each; `docs/research/gpu-suite-v1.md`,
`memhard-balance-v2.md`). No parameters adopted.

## Context

ADR 0008 concluded that A' with one gathered block per entry is compute-bound and named two paths to
memory-hardness, (i) a large-slice gather and (ii) an int8-representable modulus. Its rates were
cold-clock (matmul ~44 GMAC/s) and it modelled small random reads by bandwidth. The first large-slice
measurement (`attempt-rate-v1`) used a linear fold and a naive kernel and was withdrawn (ADR 0010).

## Measurements (CMP 50HX, warm, median of 7)

- Goldilocks matmul: ~175 GMAC/s (naive tiled kernel).
- Random small reads (8–32 B): ~3.1e9 reads/s — access-rate-bound, 25–99 GB/s.
- Warp-cooperative random reads of >= 2560 B segments: 527–533 GB/s, ~98% of the measured
  sequential stream (538 GB/s).
- Attempt with a nonlinear, warp-cooperative fold (`coop`), gathering `A` only: gather / matmul =
  3.2 (`n=256, seg=2560`), 79 (`n=256, seg=64 KiB`), 1.7 (`n=512, seg=2560`); reads run at
  515–548 GB/s. Gathering `B` too doubles the gather (`memhard-balance-v2`).
- Same attempt with a linear fold attacked by prefix sums: 2.2–53x faster than `coop`, matmul-bound.

## Decision

1. **Prototype A' (one 8-byte word per entry) is not memory-hard** for `n >= 256` (gather 0.45x–0.11x
   of the matmul). It stays a correctness harness for the gathered instance, not a memory-hard design.
2. **Path (i) is viable as a bandwidth-bound design** *if* the fold of each gathered segment is
   nonlinear and the segment exceeds the balance size (`n^3 * BW / (2 n^2 * rate)`: ~0.4 KiB at
   `n = 256`, ~1.5 KiB at `n = 1024` on this GPU). Then a tuned miner spends most of an attempt on DRAM
   traffic.
3. **Consequence for the research question**: in that regime the work is the gather, i.e. an
   Ethash-like bandwidth PoW to which a matmul is attached; the GPU-optimal linear algebra is no longer
   what is being paid for. A' therefore does not strengthen the "linear-algebra PoW" claim; it trades
   it for a memory-bandwidth claim, which must be judged on its own (ASIC with comparable memory,
   verifier dataset cost, time–memory trade-offs of the dataset).
4. **Parameter constraints recorded for any A' profile**: nonlinear fold; dataset `N >> 2 n^2` entries
   (if the operands take few distinct values, products can be grouped by value and multiplications
   drop — e.g. with `N < n` distinct values a row needs `N n` instead of `n^2` multiplications);
   verifier holds the dataset and re-reads `2 n^2 seg` bytes per block.

## Not decided

Path (ii) (int8-representable modulus) is not measured. No energy, ASIC model, HBM device or dataset
time–memory analysis exists, so no memory-hardness or ASIC-resistance claim is made.
