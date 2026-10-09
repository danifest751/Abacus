# Memory-hard balance v1 — candidate A' (gather vs matmul) — CORRECTED (warm)

Date: 2026-10-09. Probe: `scripts/memhard_balance_probe.py`.

Two corrections versus earlier drafts:
1. **No tensor-core assumption** for the field: Goldilocks / `2^61-1` are 64-bit modular arithmetic;
   int8 tensor cores do not apply.
2. **Warm gathered bandwidth.** A cold run reported ~29 GB/s; warm, the CMP reaches **~416 GB/s** for
   64 KiB gathered segments (~87% of the ~475 GB/s sequential rate). The earlier "6% of sequential" was
   a cold-clock artifact.

Rates used (CMP 50HX): `BW = 416 GB/s` (warm), field matmul `~44 GMAC/s`, int8 `~69 GMAC/s`.

Gather dominates iff `G > n^3 * BW / rate`.

## One 32-byte block per A entry (`G = 32 n^2`)

| n | rate | gather/matmul | gather bytes to balance |
|---:|---|---:|---:|
| 256 | 44 GMAC/s | 0.013 | **159 MB** |
| 512 | 44 GMAC/s | 0.007 | **1.27 GB** |
| 1024 | 44 GMAC/s | 0.003 | **10.2 GB** |
| 512 | 69 GMAC/s | 0.010 | 809 MB |
| 1024 | 69 GMAC/s | 0.005 | 6.5 GB |

## Findings

- With one block per entry the matmul dominates decisively (gather/matmul ≤ 0.013): **not memory-hard**.
- Because the gathered bandwidth is **high** (416 GB/s), making the gather dominate requires gathering
  **GB-scale data per attempt**: ~159 MB at `n=256`, ~1.3 GB at `n=512`, ~10 GB at `n=1024` (field).
- That is achievable only by reading most of a **multi-GB dataset per attempt**, i.e. the PoW becomes
  genuinely **bandwidth-bound** (at 416 GB/s, a 1.3 GB gather is ~3 ms; a 10 GB gather ~24 ms). This is
  the actual memory-hard design point: huge per-attempt gather from a multi-GB dataset.

## Implication

Candidate A' is memory-hard **only if the per-attempt gather is GB-scale** (Tenero-style but much
larger), reading most of a multi-GB dataset. The earlier "window" framing is withdrawn. The plain
candidate A (compute-bound) remains the baseline; path (ii) (int8 modulus + tensor cores) still needs a
GB-scale gather because the bandwidth is high.

## Feasible design point on the CMP (10 GB VRAM)

The tension is `dataset >> gather > n^3*BW/rate`. On the CMP this is feasible only at **small `n`**:

| n | k | gather needed | gather s | matmul s | dataset must be |
|---:|---:|---:|---:|---:|---:|
| 256 | 64 | 159 MB | 0.38 ms | 0.38 ms | >> 159 MB (e.g. ~2 GB) |

At `n=256, k=64`: gather ≈ matmul (balanced), the gather is ~8% of a 2 GB dataset (stays random
access), and the verifier is `2k*n^2 / n^3 = 0.5x` the work (`n > 2k` holds). With `k = 2..3`
(ADR 0009) the verifier term is negligible. Any fold of a gathered slice must be nonlinear, or
prefix sums remove the gather (ADR 0010). This fits the CMP's
10 GB. Larger `n` (512+) needs a 1.3-10 GB gather and a `>>`-larger dataset, which does **not** fit
the CMP — memory-hard A' at large `n` requires more VRAM (24-80 GB GPUs).

## Caveats

- Warm bandwidth measured with an in-kernel warm-up pass; clock state still matters, so lock clocks for
  a definitive number. The sequential reference suggests the DRAM can do ~475 GB/s; the gather reaches
  ~87% of that.
- The estimate ignores cache, TLB and dataset-construction costs.

## Reproduce

```
python scripts/memhard_balance_probe.py
```
