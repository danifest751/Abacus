#!/usr/bin/env python3
"""Minimal CPU mine-and-verify loop for candidate A (header-bound Freivalds matmul PoW).

Toy calibration of the whole flow: header/nonce -> (A, B) -> C = A*B -> score hash vs target, with
Fiat-Shamir Freivalds verification. Small n, low target bits; this measures attempts/s on the CPU and
sanity-checks that a found block verifies. Not a miner, not a work model.
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from reference.freivalds import P, freivalds_verify_multi, matmul  # noqa: E402
from freivalds_forgery_probe import fs_challenges  # noqa: E402


def expand(seed: bytes, count: int) -> list:
    out = []
    i = 0
    while len(out) < count:
        h = hashlib.sha256(b"abacus/matmul" + seed + i.to_bytes(4, "little")).digest()
        for off in (0, 8, 16, 24):
            out.append(int.from_bytes(h[off:off + 8], "little") % P)
        i += 1
    return out[:count]


def encode(C) -> bytes:
    return b"".join(int(x).to_bytes(8, "little") for x in C)


def instance(preheader: bytes, n: int):
    seed = hashlib.sha256(b"abacus/instance" + preheader).digest()
    vals = expand(seed, 2 * n * n)
    return vals[:n * n], vals[n * n:2 * n * n]


def mine(n: int, target_bits: int, max_attempts: int):
    limit = 1 << (256 - target_bits)
    t0 = time.perf_counter()
    for attempt in range(1, max_attempts + 1):
        preheader = b"ABACUS" + attempt.to_bytes(8, "little")
        A, B = instance(preheader, n)
        C = matmul(A, B, n)
        score = int.from_bytes(hashlib.sha256(b"abacus/score" + preheader + encode(C)).digest(), "big")
        if score < limit:
            dt = time.perf_counter() - t0
            return {"found": True, "attempts": attempt, "seconds": dt,
                    "attempts_per_s": attempt / dt, "preheader": preheader.hex(),
                    "A": A, "B": B, "C": C}
    dt = time.perf_counter() - t0
    return {"found": False, "attempts": max_attempts, "seconds": dt,
            "attempts_per_s": max_attempts / dt}


def verify(A, B, C, n: int, k: int) -> bool:
    return freivalds_verify_multi(A, B, C, n, fs_challenges(C, n, k))


def main() -> int:
    n = 24
    k = 8
    res = mine(n, target_bits=8, max_attempts=2000)
    verified = False
    if res["found"]:
        verified = verify(res["A"], res["B"], res["C"], n, k)

    out = {
        "date": "2026-10-09",
        "note": "toy CPU mine+verify loop; not a miner or work model",
        "n": n,
        "k": k,
        "target_bits": 8,
        "found": res["found"],
        "attempts": res["attempts"],
        "attempts_per_s": res["attempts_per_s"],
        "verified": verified,
    }
    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "mine-sim-20261009.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(json.dumps(out, indent=2))
    return 0 if (res["found"] and verified) else 1


if __name__ == "__main__":
    raise SystemExit(main())
