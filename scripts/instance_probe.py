#!/usr/bin/env python3
"""Instance-structure probe (D3/D4 template) for the Abacus laboratory.

Question: does a permissionless linear instance admit a cheaper route? This probe measures the D4
falsifier directly: a **structured** (low-rank) instance collapses the multiplication cost, while a
**uniform** (header-derived) instance is full rank and dense, so screening yields no gain.

It reports, per (n, r):
  - rank of a random dense matrix (expected: full) and of a low-rank matrix (expected: r);
  - wall time of the dense `A*B` (cost ~ n^3) versus the cheap `U*(Vt*B)` (cost ~ 2*r*n^2);
  - the analytic ratio n / (2 r).

Writes JSON to an ignored `artifacts/`. This is a bounded toy measurement, not a work model.
"""

from __future__ import annotations

import json
import os
import random
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from reference.freivalds import P, random_matrix, random_vector  # noqa: E402,F401


def mm(a, b, m, k, nn, p=P):
    """Row-major `(m x k) @ (k x nn)` modulo p."""
    c = [0] * (m * nn)
    for i in range(m):
        base = i * k
        crow = i * nn
        for t in range(k):
            ait = a[base + t]
            if ait == 0:
                continue
            brow = t * nn
            for j in range(nn):
                c[crow + j] = (c[crow + j] + ait * b[brow + j]) % p
    return c


def rank_mod_p(a, rows, cols, p=P):
    a = list(a)
    r = 0
    for c in range(cols):
        piv = None
        for i in range(r, rows):
            if a[i * cols + c] % p:
                piv = i
                break
        if piv is None:
            continue
        if piv != r:
            a[r * cols:(r + 1) * cols], a[piv * cols:(piv + 1) * cols] = (
                a[piv * cols:(piv + 1) * cols],
                a[r * cols:(r + 1) * cols],
            )
        inv_piv = pow(a[r * cols + c], p - 2, p)
        for j in range(c, cols):
            a[r * cols + j] = a[r * cols + j] * inv_piv % p
        for i in range(rows):
            if i != r and a[i * cols + c] % p:
                f = a[i * cols + c]
                for j in range(c, cols):
                    a[i * cols + j] = (a[i * cols + j] - f * a[r * cols + j]) % p
        r += 1
        if r == rows:
            break
    return r


def structure_collapse(n, r, rng):
    """Return correctness + analytic ratio for a low-rank instance."""
    U = [rng.randrange(P) for _ in range(n * r)]
    Vt = [rng.randrange(P) for _ in range(r * n)]
    B = [rng.randrange(P) for _ in range(n * n)]
    A = mm(U, Vt, n, r, n)
    dense = mm(A, B, n, n, n)
    cheap = mm(U, mm(Vt, B, r, n, n), n, r, n)
    return {
        "correct": dense == cheap,
        "rank_A": rank_mod_p(A, n, n),
        "rank_random_B": rank_mod_p(B, n, n),
        "analytic_ratio": n / (2.0 * r),
    }


def measure(n, r, rng):
    U = [rng.randrange(P) for _ in range(n * r)]
    Vt = [rng.randrange(P) for _ in range(r * n)]
    B = [rng.randrange(P) for _ in range(n * n)]
    A = mm(U, Vt, n, r, n)

    t0 = time.perf_counter()
    mm(A, B, n, n, n)
    dense_s = time.perf_counter() - t0

    t0 = time.perf_counter()
    mm(U, mm(Vt, B, r, n, n), n, r, n)
    cheap_s = time.perf_counter() - t0

    return {
        "n": n,
        "r": r,
        "dense_s": dense_s,
        "cheap_s": cheap_s,
        "measured_ratio": dense_s / cheap_s if cheap_s else None,
        "analytic_ratio": n / (2.0 * r),
        "correct": mm(A, B, n, n, n) == mm(U, mm(Vt, B, r, n, n), n, r, n),
    }


def main():
    rng = random.Random(20261009)
    rows = [measure(n, r, rng) for n in (16, 24, 32) for r in (1, 2, 4)]

    # Screening check: random dense matrices are full rank.
    rng2 = random.Random(1)
    n = 32
    full_rank = all(rank_mod_p(random_matrix(rng2, n), n, n) == n for _ in range(8))

    out = {
        "date": "2026-10-09",
        "note": "bounded toy measurement; not a work model",
        "dense_random_matrices_full_rank": full_rank,
        "runs": rows,
    }

    os.makedirs("artifacts", exist_ok=True)
    path = os.path.join("artifacts", "instance-probe-20261009.json")
    with open(path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)

    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
