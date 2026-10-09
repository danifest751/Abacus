#!/usr/bin/env python3
"""Freivalds forgery probe (ADR 0004).

Shows that a single header-derived challenge is forgeable: a cheater builds `C' = C + M` with
`M r = 0` (so `C' r = C r`) and beats the score target without doing the matmul. Then shows that
`k` Fiat–Shamir challenges bound to the committed `C` reject the forgery.

Writes JSON to an ignored `artifacts/`. Bounded toy measurement, not a work model.
"""

from __future__ import annotations

import hashlib
import json
import os
import random
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from reference.freivalds import (  # noqa: E402
    P,
    freivalds_verify,
    freivalds_verify_multi,
    matmul,
    random_matrix,
    random_vector,
)


def fs_challenges(C, n, k, preheader=b"", domain=b"abacus/check"):
    """k challenge vectors, Fiat-Shamir bound to the committed C.

    Hash C **once** to a root, then expand the k*n field elements from the root. Hashing the whole
    C per element would make verification O(k*n^3) bytes and defeat the purpose (ADR 0006).
    """
    cbytes = b"".join(int(x).to_bytes(8, "little") for x in C)
    root = hashlib.sha256(domain + preheader + cbytes).digest()
    vals = []
    counter = 0
    need = k * n
    while len(vals) < need:
        h = hashlib.sha256(root + counter.to_bytes(4, "little")).digest()
        for off in (0, 8, 16, 24):
            vals.append(int.from_bytes(h[off:off + 8], "little") % P)
        counter += 1
    return [vals[i * n:(i + 1) * n] for i in range(k)]


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
    rs_forged = fs_challenges(Cprime, n, k)
    fs_rejects_forgery = not freivalds_verify_multi(A, B, Cprime, n, rs_forged)
    fs_accepts_honest = freivalds_verify_multi(A, B, C, n, fs_challenges(C, n, k))

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
