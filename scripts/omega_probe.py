#!/usr/bin/env python3
"""Work-accounting probe (candidate A, open question 1): multiplication counts, naive vs Strassen.

Dense `n x n` multiplication costs `O(n^omega)`, not `n^3`. The work model must use the best known
cost. This probe reports exact **scalar multiplication counts** (deterministic, no timing) for the
naive algorithm and for a recursive Strassen split, plus the implied exponent.

Strassen: a size-`n` multiply uses 7 size-`n/2` multiplies (7^log2 n = n^log2 7 = n^2.807...);
additions rise, multiplications fall. Writes JSON to an ignored `artifacts/`.
"""

from __future__ import annotations

import json
import math
import os


def naive_mults(n: int) -> int:
    return n**3


def strassen_mults(n: int, base: int) -> int:
    if n <= base:
        return n**3
    return 7 * strassen_mults(n // 2, base)


def strassen_adds(n: int, base: int) -> int:
    if n <= base:
        return n * n * (n - 1)
    half = n // 2
    return 7 * strassen_adds(half, base) + 18 * half * half


def main() -> int:
    base = 64
    rows = []
    for k in range(7, 13):  # n = 128 .. 8192
        n = 1 << k
        nm = naive_mults(n)
        sm = strassen_mults(n, base)
        ratio = nm / sm
        saved = math.log(ratio) / math.log(n)  # ~ 3 - log2(7) asymptotically
        rows.append(
            {
                "n": n,
                "naive_mults": nm,
                "strassen_mults": sm,
                "strassen_adds": strassen_adds(n, base),
                "ratio_naive_over_strassen": ratio,
                "work_saved_exponent": saved,
            }
        )

    out = {
        "date": "2026-10-09",
        "note": "exact operation counts; not a work model; additions grow for Strassen",
        "strassen_base": base,
        "log2_7": math.log2(7),
        "work_saved_exponent_asymptotic": 3.0 - math.log2(7),
        "runs": rows,
    }

    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "omega-probe-20261009.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)

    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
