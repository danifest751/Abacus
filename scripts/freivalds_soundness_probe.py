#!/usr/bin/env python3
"""Freivalds soundness probe (ADR 0009): per-challenge error is ~1/p over F_p, not 1/2.

For a wrong product, ``E = A*B - C != 0``. A challenge ``r`` passes iff ``E r = 0``. If ``r`` is
uniform over ``F_p^n``, any nonzero row ``e`` of ``E`` gives ``Pr[e . r = 0] = 1/p`` (Schwartz-Zippel
for a nonzero linear form), so one challenge errs with probability at most ``1/p``. The classical
``1/2`` bound holds for ``r in {0,1}^n``. This probe measures both empirically over a **small** prime
so the rates are observable, for worst-case ``E`` (a single nonzero entry) and random ``E``.

Writes JSON to an ignored ``artifacts/``. Bounded toy measurement.
"""

from __future__ import annotations

import json
import os
import random

SMALL_P = 101  # small prime so that 1/p is measurable


def passes(e_rows, r, p):
    return all(sum(a * b for a, b in zip(row, r)) % p == 0 for row in e_rows)


def rate(n, p, trials, rng, binary, worst):
    hits = 0
    for _ in range(trials):
        if worst:
            e = [[0] * n for _ in range(n)]
            e[rng.randrange(n)][rng.randrange(n)] = rng.randrange(1, p)
        else:
            while True:
                e = [[rng.randrange(p) for _ in range(n)] for _ in range(n)]
                if any(any(row) for row in e):
                    break
        r = [rng.randrange(2) for _ in range(n)] if binary else [rng.randrange(p) for _ in range(n)]
        hits += passes(e, r, p)
    return hits / trials


def measure(n=8, p=SMALL_P, trials=40000, seed=20261009):
    rng = random.Random(seed)
    return {
        "n": n,
        "p": p,
        "trials": trials,
        "field_r_worst_E": rate(n, p, trials, rng, binary=False, worst=True),
        "binary_r_worst_E": rate(n, p, trials, rng, binary=True, worst=True),
        "field_r_random_E": rate(n, p, trials, rng, binary=False, worst=False),
        "bound_field_1_over_p": 1 / p,
        "bound_binary_1_over_2": 0.5,
    }


def main() -> int:
    out = {
        "date": "2026-10-09",
        "note": "per-challenge Freivalds error: ~1/p for r uniform in F_p, ~1/2 for r in {0,1}",
        "run": measure(),
        # LE64 mod P maps 2^32 - 1 residues twice, so a coordinate takes any value with probability
        # at most 2/2^64; a nonzero linear form then vanishes with probability at most 2^-63.
        "goldilocks_per_challenge_bound_log2": -63,
    }
    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "freivalds-soundness-20261009.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
