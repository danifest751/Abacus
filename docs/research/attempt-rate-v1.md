# Attempt rate v1 — candidate A' (memory-hard) on the CMP

> **WITHDRAWN (ADR 0010).** The bench folded each gathered segment with a plain `u64` sum, which a
> miner answers with two prefix-sum reads (`seg/8` reads -> 2), so the "memory-hardness is
> achieved" conclusion below does not hold. The gather was also one uncoalesced thread per entry
> (the 11.6–85 GB/s describe that kernel, not the hardware; cooperative warm reads reach ~416
> GB/s), the dataset was a constant `0x5A` fill, and `n=512` came out faster than `n=256` with 4x
> the gather and 8x the matmul — consistent with the cold-clock artifact of
> `gather-bandwidth-v1.md` (64 attempts run for about one second). The raw table is kept for the
> record. Superseded by `attempt-rate-v2.md`, which measures the attack (14–466x) and the warm
> rates.


Date: 2026-10-09. Bench: `cuda/attempt_bench.cu`, CMP 50HX (sm_75). Per attempt: gather `n^2` field
elements (each folded from a `seg`-byte random dataset segment) + Goldilocks `n x n` matmul.

| n | seg B | gather MB/attempt | attempts/s | effective gather GB/s |
|---:|---:|---:|---:|---:|
| 256 | 2560 | 167.8 | 69.1 | 11.6 |
| 256 | 65536 | 4295.0 | 19.8 | 85.0 |
| 512 | 2560 | 671.1 | 101.8 | 68.3 |

## Findings

- **Effective per-entry gather bandwidth is far below the cooperative number.** `gather_bench`
  (block-cooperative 64 KiB reads) reached ~416 GB/s; here, one field element per **independent random**
  segment gives only **11.6-85 GB/s**. The *layout* of the gather dominates the achievable bandwidth.
- **Memory-hardness is achieved** at `n=256, seg=2560`: the gather (~168 MB at 11.6 GB/s = ~14.5 ms)
  dwarfs the matmul (`n^3/44e9` = ~0.38 ms). So A' is genuinely gather-bound here — with a **modest**
  gather (168 MB), not the GB-scale the earlier 416-GB/s estimate implied, because the realistic gather
  bandwidth is so much lower.
- **Attempt rate is bandwidth-limited**: ~69 attempts/s at `n=256` and ~102 at `n=512` (the larger
  matmul amortises the per-attempt overhead and the larger gather reaches a higher effective
  bandwidth).
- Block rate = attempts/s x (target / 2^D); with the low attempt rate, difficulty must be modest or
  block times long.

## Reinterpretation of the balance

The earlier `memhard-balance-v1.md` used the **cooperative** bandwidth (416 GB/s), which overstates the
gather's speed and thus overstates the gather volume needed to dominate. Using the **realistic per-entry
bandwidth** (11.6-85 GB/s), the gather dominates with tens-to-hundreds of MB per attempt at `n=256`.
The design is memory-hard **and** bandwidth-bound, as intended, at a low attempt rate.

## Caveats

- The gather kernel is simple (one thread per entry, sequential segment read); a tuned layout could
  raise the effective bandwidth and shift the balance. This is a first measured point.
- The score hash over `C` and the verifier cost are not in this bench; both are `O(n^2)` and minor.

## Reproduce

```
nvcc -O3 -arch=sm_75 cuda/attempt_bench.cu -o attempt_bench
./attempt_bench 256 2560 2048 64
./attempt_bench 512 2560 2048 32
```
