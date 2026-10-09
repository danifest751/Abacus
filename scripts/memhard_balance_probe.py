#!/usr/bin/env python3
"""Balance probe for candidate A' (gather vs matmul), from measured CMP 50HX rates.

Per attempt the miner gathers the `2 n^2` operand entries of `A, B` from the epoch dataset and
computes the `n^3` Goldilocks product. Rates (warm, median of 7, `docs/research/gpu-suite-v1.md`):

  RATE_FIELD     = 177e9   Goldilocks MAC/s, naive tiled kernel (175-179 for n = 512..2048; 167 at 256)
  ACCESS_RATE    = 3.1e9   random 8-32 byte reads/s (small reads are bound by the access rate)
  BW_COOP        = 530e9   warp-cooperative random reads of >= 2560-byte segments (~98% of the
                           measured 538 GB/s sequential stream)

Two designs:

- prototype (`crates/abacus-chain`): one 8-byte word per entry -> `2 n^2` random accesses;
- large slice: each entry folds a `seg`-byte segment with a **nonlinear** fold -> `2 n^2 seg` bytes
  read cooperatively. (A linear fold is answered by prefix sums and is not counted as a gather.)

Gather dominates iff its time exceeds the matmul time. Writes JSON to an ignored `artifacts/`.
"""

from __future__ import annotations

import json
import os

RATE_FIELD = 177e9  # Goldilocks MAC/s (warm, measured)
ACCESS_RATE = 3.1e9  # random small reads per second (measured, 8 and 32 byte segments)
BW_COOP = 530e9  # bytes/s, cooperative random reads of >= 2560 B segments (measured)
WORD = 8  # bytes consumed per entry by the prototype gather


def matmul_s(n: int, rate: float = RATE_FIELD) -> float:
    return n**3 / rate


def prototype_gather_s(n: int) -> float:
    return 2 * n * n / ACCESS_RATE


def slice_gather_s(n: int, seg: int) -> float:
    return 2 * n * n * seg / BW_COOP


def balance_segment_bytes(n: int, rate: float = RATE_FIELD) -> float:
    """Segment size at which the cooperative large-slice gather equals the matmul time."""
    return matmul_s(n, rate) * BW_COOP / (2 * n * n)


def row(n: int, seg: int = 2560) -> dict:
    m = matmul_s(n)
    p = prototype_gather_s(n)
    s = slice_gather_s(n, seg)
    return {
        "n": n,
        "matmul_s": m,
        "prototype_gather_s": p,
        "prototype_gather_over_matmul": p / m,
        "slice_seg_bytes": seg,
        "slice_gather_s": s,
        "slice_gather_over_matmul": s / m,
        "slice_read_MB_per_attempt": 2 * n * n * seg / 1e6,
        "balance_seg_bytes": balance_segment_bytes(n),
    }


def main() -> int:
    out = {
        "date": "2026-10-09",
        "note": "A' balance from measured warm CMP 50HX rates; nonlinear fold assumed for slices",
        "assumptions": {"field_MAC_s": RATE_FIELD, "access_rate_per_s": ACCESS_RATE, "coop_BW_B_s": BW_COOP},
        "runs": [row(n, seg) for n in (64, 256, 512, 1024) for seg in (2560, 65536)],
    }
    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "memhard-balance-v2.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
