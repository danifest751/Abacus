#!/usr/bin/env python3
"""Freivalds forgery probe (ADR 0004).

Shows that a single header-derived challenge is forgeable: a cheater builds `C' = C + M` with
`M r = 0` (so `C' r = C r`) and beats the score target without doing the matmul. Then shows that
`k` Fiat–Shamir challenges bound to the committed `C` reject the forgery.

Writes JSON to an ignored `artifacts/`. Bounded toy measurement, not a work model.
"""

from __future__ import annotations

import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from reference.chain import fs_challenges  # noqa: E402  (bound to (preheader, C); ADR 0006/0010)
from reference.freivalds import (  # noqa: E402
    P,
    freivalds_verify,
    freivalds_verify_multi,
    matmul,
    random_matrix,
    random_vector,
)

PREHEADER = b"abacus/probe"


def forge(A, B, n, r, rng):
    """Return C' = (A*B) + M with M != 0 and M r = 0 (each row orthogonal to r)."""
    base = matmul(A, B, n)
    M = [0] * (n * n)
    nonzero = False
    inv_r0 = pow(r[0], P - 2, P)
    for i in range(n):
        s = 0
        for j in range(1, n):
            m = rng.randrange(P)
            M[i * n + j] = m
            if m:
                nonzero = True
            s = (s + m * r[j]) % P
        M[i * n + 0] = (-s * inv_r0) % P
    Cprime = [(base[t] + M[t]) % P for t in range(n * n)]
    return Cprime, nonzero


def main() -> int:
    rng = random.Random(20261009)
    n = 8
    k = 8
    A = random_matrix(rng, n)
    B = random_matrix(rng, n)
    C = matmul(A, B, n)

    # single, header-derived challenge (known before C' is chosen)
    r = random_vector(rng, n)
    if r[0] == 0:
        r[0] = 1
    Cprime, nonzero = forge(A, B, n, r, rng)

    single_passes_forgery = freivalds_verify(A, B, Cprime, n, r) and Cprime != C and nonzero

    # Fiat–Shamir multi-challenge bound to C
    rs_forged = fs_challenges(PREHEADER, Cprime, n, k)
    fs_rejects_forgery = not freivalds_verify_multi(A, B, Cprime, n, rs_forged)
    fs_accepts_honest = freivalds_verify_multi(A, B, C, n, fs_challenges(PREHEADER, C, n, k))

    out = {
        "date": "2026-10-09",
        "note": "single-challenge Freivalds is forgeable; Fiat-Shamir multi-challenge is not",
        "n": n,
        "k": k,
        "single_challenge_forgery_passes": single_passes_forgery,
        "fiat_shamir_rejects_forgery": fs_rejects_forgery,
        "fiat_shamir_accepts_honest": fs_accepts_honest,
    }

    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "freivalds-forgery-20261009.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)

    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
