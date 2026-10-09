#!/usr/bin/env python3
"""Balance probe for candidate A' (gather vs matmul) — corrected.

An earlier version assumed a 1e12 MAC/s tensor-core matmul. That is wrong for this field: Goldilocks
/ 2^61-1 are 64-bit modular arithmetic, so int8 tensor cores do NOT apply (a small int8-representable
modulus would be needed). This version uses measured-grounded rates instead.

Rates (CMP 50HX, grounded):
  gathered bandwidth BW_g = 416 GB/s (measured warm, block-cooperative 64 KiB segments; the earlier
  29 GB/s was a cold-clock artifact, see docs/research/gather-bandwidth-v1.md);
  Goldilocks field matmul ~190 GMAC/s (measured naive kernel, warm; the earlier 44 GMAC/s was a
  cold-clock run, see docs/research/gpu-baseline-v1.md);
  int8 matmul ~69 GMAC/s (CPPMiner Pearl rate on the CMP, for reference).

For an attempt with `n^3` MACs and `G` gathered bytes:
  gather_s = G / BW_g ;  matmul_s = n^3 / rate.
Gather dominates only if G > n^3 * BW_g / rate — a large per-attempt gather (memory-hard PoW is
inherently bandwidth-bound). With one 32-byte block per `A` entry (G = 32 n^2) the matmul dominates.

Writes JSON to an ignored artifacts/.
"""

from __future__ import annotations

import json
import os

BW_G = 416e9          # gathered bytes/s (measured WARM, 64 KiB segments; cold runs under-report)
RATE_FIELD = 190e9   # Goldilocks MAC/s, naive tiled kernel, WARM (166-204 measured; 44 was cold-clock)
RATE_INT8 = 69e9     # int8 MAC/s (CPPMiner CMP reference)
BLOCK = 32           # one dataset block per A entry in the naive design


def required_gather_bytes_for_balance(n: int, rate: float, bw=BW_G) -> float:
    return (n**3) * bw / rate


def row(n: int, rate: float) -> dict:
    g_one = BLOCK * n * n
    gather_s = g_one / BW_G
    matmul_s = (n**3) / rate
    return {
        "n": n,
        "rate_MAC_s": rate,
        "gathered_bytes_one_block_per_entry": g_one,
        "gather_s": gather_s,
        "matmul_s": matmul_s,
        "gather_over_matmul": gather_s / matmul_s,
        "required_gather_bytes_for_balance": required_gather_bytes_for_balance(n, rate),
    }


def main() -> int:
    rows = []
    for n in (256, 512, 1024):
        rows.append({"field_matmul": row(n, RATE_FIELD), "int8_matmul": row(n, RATE_INT8)})
    out = {
        "date": "2026-10-09",
        "note": "corrected: no tensor-core assumption for the field; grounded rates only",
        "assumptions": {"gathered_BW_B_s": BW_G, "field_MAC_s": RATE_FIELD, "int8_MAC_s": RATE_INT8,
                        "block_bytes": BLOCK},
        "runs": rows,
    }
    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "memhard-balance-20261009b.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
