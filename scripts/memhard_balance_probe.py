#!/usr/bin/env python3
"""Balance probe for candidate A': gather (memory) vs matmul (compute), spec/04.

If the matmul dominates the attempt, the memory-hard layer is cosmetic. This estimates the two
components under explicit assumptions and reports where they are comparable.

Assumptions (labelled, not measured here):
  gathered bandwidth BW_g = 29 GB/s (measured on the CMP; docs/research/gather-bandwidth-v1.md);
  dataset block = 32 bytes; A has n^2 entries, each a gathered block -> gather bytes = 32*n^2;
  matmul: n^3 multiplies at a naive rate (44 GMAC/s, measured) and at a tensor-core rate (1e12 MAC/s).

Writes JSON to an ignored artifacts/.
"""

from __future__ import annotations

import json
import os

GATHERED_BW = 29e9          # bytes/s (measured, CMP)
BLOCK = 32                  # bytes per gathered block
NAIVE_RATE = 44e9           # MAC/s (measured naive GPU kernel)
TC_RATE = 1e12              # MAC/s (assumed tensor-core)


def components(n: int, bw=GATHERED_BW, rate=NAIVE_RATE) -> dict:
    gather_bytes = BLOCK * n * n
    gather_s = gather_bytes / bw
    matmul_s = (n**3) / rate
    return {
        "n": n,
        "gather_bytes": gather_bytes,
        "gather_s": gather_s,
        "matmul_s": matmul_s,
        "gather_over_matmul": gather_s / matmul_s,
    }


def main() -> int:
    rows = []
    for n in (512, 1024, 2048, 4096):
        rows.append({"naive_matmul": components(n, rate=NAIVE_RATE),
                     "tensor_core_matmul": components(n, rate=TC_RATE)})

    # n where gather_s == matmul_s under the tensor-core rate: 32 n^2 / BW = n^3 / TC
    crossover_tc = (BLOCK / GATHERED_BW) / (1.0 / TC_RATE)

    out = {
        "date": "2026-10-09",
        "note": "estimate under labelled assumptions; not a measurement",
        "assumptions": {"gathered_BW_B_s": GATHERED_BW, "block_bytes": BLOCK,
                        "naive_MAC_s": NAIVE_RATE, "tensor_core_MAC_s": TC_RATE},
        "crossover_n_tensor_core": crossover_tc,
        "runs": rows,
    }
    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "memhard-balance-20261009.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
