# Memory-hard balance v1 — candidate A' (gather vs matmul)

Date: 2026-10-09. Probe: `scripts/memhard_balance_probe.py` (estimate, not a measurement).
Assumptions: gathered bandwidth 29 GB/s (measured, CMP), 32-byte dataset block, `A` has `n^2` gathered
entries; matmul rate 44 GMAC/s (measured naive) and 1e12 MAC/s (assumed tensor core).

`gather_over_matmul = (32 n^2 / BW) / (n^3 / rate) = 32 * rate / (BW * n)`.

| n | naive matmul | tensor-core matmul |
|---:|---:|---:|
| 512 | 0.09 | **2.16** |
| 1024 | 0.05 | 1.08 |
| 2048 | 0.02 | 0.54 |
| 4096 | 0.01 | 0.27 |

## Findings

- **Memory-hard regime is a window.** The gather dominates only when the matmul is fast
  (tensor-core) **and** `n` is small: crossover at `n ≈ 1103` (tensor-core rate). For `n` above it, the
  `n^3` matmul dominates again and the memory layer is cosmetic; with the naive matmul the matmul
  dominates at all relevant `n`.
- Therefore A' is genuinely memory-hard only for a **tensor-core matmul with `n` roughly 256-1024**.
  At `n = 512` the gather is ~2.2x the matmul under the tensor-core assumption.
- Combined with the verifier constraint `n > 2k` (ADR 0006), a candidate profile is **`n = 512`,
  `k = 128`**: verifier margin 0.70x (from `verifier-throughput-v1.md`) and gather-dominated in the
  memory-hard window.

## Caveats

- The tensor-core rate is assumed, not measured here; the gathered bandwidth is one cold run
  (`gather-bandwidth-v1.md`) and is clock-sensitive. A measured tensor-core matmul and a warm,
  clock-stable gathered-bandwidth number are needed before fixing parameters.
- The estimate ignores latency effects, cache, and the dataset-construction cost; it is a scoping
  estimate.

## Reproduce

```
python scripts/memhard_balance_probe.py
```
