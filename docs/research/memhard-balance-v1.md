# Memory-hard balance v1 — candidate A' (gather vs matmul) — CORRECTED

Date: 2026-10-09. Probe: `scripts/memhard_balance_probe.py`.

**Correction.** An earlier version of this note assumed a `1e12` MAC/s tensor-core matmul. That is
wrong for this field: Goldilocks / `2^61-1` are 64-bit modular arithmetic, so int8 tensor cores do
**not** apply (a small int8-representable modulus would be needed). This version uses grounded rates.

Rates (CMP 50HX): gathered bandwidth `BW = 29 GB/s` (measured); Goldilocks field matmul `~44 GMAC/s`
(measured naive); int8 matmul `~69 GMAC/s` (CPPMiner Pearl rate on the CMP, for reference).

For `n^3` MACs and `G` gathered bytes: `gather_s = G/BW`, `matmul_s = n^3/rate`. Gather dominates iff
`G > n^3 * BW / rate`.

## With one 32-byte block per A entry (`G = 32 n^2`)

| n | rate | gather s | matmul s | gather/matmul | G needed to balance |
|---:|---|---:|---:|---:|---:|
| 256 | 44 GMAC/s | 0.072 ms | 0.38 ms | 0.19 | 22 MB |
| 512 | 44 GMAC/s | 0.29 ms | 3.05 ms | 0.09 | 88 MB |
| 512 | 69 GMAC/s | 0.29 ms | 1.95 ms | 0.15 | 56 MB |
| 1024 | 44 GMAC/s | 1.16 ms | 24.4 ms | 0.05 | 710 MB |

## Findings

- **With one block per entry the matmul dominates** (gather/matmul 0.05-0.19): the memory layer is
  **cosmetic** on the CMP for both the field and the int8 rate.
- To be memory-hard, the per-attempt **gather volume must be large**: `G > n^3 * BW / rate`
  (e.g. ~56-88 MB at `n=512`, ~710 MB at `n=1024` for the field). That is Tenero-scale (its gathered
  design reads 16 MiB per attempt) — and memory-hard PoW is then inherently **bandwidth-bound and
  slow** (at 29 GB/s a 56 MB gather is ~2 ms per attempt).
- **Tensor cores require a small (int8-representable) modulus.** If the field stays Goldilocks/
  `2^61-1`, there is no tensor-core acceleration and the matmul dominates strongly. A faster rate
  (int8) helps but does not by itself flip the balance; the gather volume does.

## Implication

Candidate A' is **not memory-hard as first sketched** (one gathered block per entry). To make the
gather dominant the design must gather a large slice per attempt (Tenero-style) and/or use an
int8-representable modulus with tensor cores, and accept that the PoW becomes bandwidth-bound. This is
a real re-design, recorded in ADR 0008; the plain candidate A (compute-bound) remains the baseline.

## Reproduce

```
python scripts/memhard_balance_probe.py
```
