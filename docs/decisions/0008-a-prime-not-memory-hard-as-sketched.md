# ADR 0008 — A' is not memory-hard as first sketched; correct the balance

Status: accepted correction. Revises the A' balance claim (ADR 0007, spec/04).

## Context

An earlier balance estimate for candidate A' assumed a `1e12` MAC/s tensor-core matmul, concluding the
gather dominates in a window (`memhard-balance-v1.md`). That assumption is invalid: the Freivalds
field is Goldilocks / `2^61-1`, i.e. **64-bit modular arithmetic**, to which **int8 tensor cores do not
apply**. With grounded rates (CMP 50HX: field matmul ~44 GMAC/s, int8 ~69 GMAC/s, gathered bandwidth
~29 GB/s), the matmul **dominates** when each `A` entry gathers one 32-byte block
(gather/matmul 0.05-0.19). The memory layer is cosmetic.

## Decision

1. **Withdraw the "memory-hard window" claim.** A' as first sketched (one gathered block per entry) is
   **compute-bound**, not memory-hard, on the CMP for both the field and int8 rates.
2. To make the gather dominant, the per-attempt gather must satisfy `G > n^3 * BW / rate`
   (e.g. ~56-88 MB at `n=512`, ~710 MB at `n=1024` for the field). That is Tenero-scale and makes the
   PoW **bandwidth-bound and slow** (a 56 MB gather is ~2 ms at 29 GB/s).
3. Two redesign paths, to be evaluated before any further memory-hard claim:
   - **(i) Large-slice gather** (Tenero-style) with the field matmul: gather tens of MiB per attempt so
     the read pattern, not the multiplies, dominates.
   - **(ii) int8-representable modulus** (small prime) so the matmul can use tensor cores, with
     Freivalds over that prime; then the int8 rate applies. Still needs a large gather to flip the
     balance because the gather is so slow.
4. The plain candidate A (compute-bound, ASIC-friendly) remains the measured baseline.

## Consequences

- The ASIC tension (ADR 0007) is **unresolved**: neither path removes it. Path (ii) uses tensor cores
  (as ASIC-friendly as GPUs); path (i) is memory-hard in the Tenero sense but still ASIC-gettable.
- New measurements required before re-claiming memory-hardness: a **measured int8 tensor-core matmul
  rate** on the CMP and a **warm, clock-stable gathered bandwidth**; then re-run the balance probe.
- `spec/04` §5 and `docs/CRITICAL-PATH.md` are updated to require these before the A' gate passes.
