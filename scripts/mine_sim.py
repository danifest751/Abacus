#!/usr/bin/env python3
"""Minimal CPU mine-and-verify loop for candidate A (header-bound Freivalds matmul PoW).

Toy calibration of the whole flow with the **same derivations as the Rust chain**
(``reference.chain``): preheader/nonce -> (A, B) -> C = A*B -> score hash vs target, with
Fiat-Shamir Freivalds verification bound to (preheader, C). Small n, low target bits; this measures
attempts/s on the CPU and sanity-checks that a found block verifies. Not a miner, not a work model.
"""

from __future__ import annotations

import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from reference.chain import instance, preheader, score, score_lead, verify_fs  # noqa: E402
from reference.freivalds import P, matmul  # noqa: E402,F401

CHAIN_ID = b"\xab" * 32
PREV = b"\x00" * 32


def mine(n: int, target_bits: int, max_attempts: int):
    t0 = time.perf_counter()
    for nonce in range(max_attempts):
        ph = preheader(CHAIN_ID, 2, 0, PREV, 0, target_bits, nonce)
        A, B = instance(ph, n)
        C = matmul(A, B, n)
        if score_lead(score(ph, C)) >= target_bits:
            dt = time.perf_counter() - t0
            return {"found": True, "attempts": nonce + 1, "seconds": dt,
                    "attempts_per_s": (nonce + 1) / dt, "preheader": ph.hex(),
                    "ph": ph, "A": A, "B": B, "C": C}
    dt = time.perf_counter() - t0
    return {"found": False, "attempts": max_attempts, "seconds": dt,
            "attempts_per_s": max_attempts / dt}


def verify(ph: bytes, A, B, C, n: int, k: int) -> bool:
    return verify_fs(ph, A, B, C, n, k)


def main() -> int:
    n = 24
    k = 8
    res = mine(n, target_bits=8, max_attempts=2000)
    verified = False
    if res["found"]:
        verified = verify(res["ph"], res["A"], res["B"], res["C"], n, k)

    out = {
        "date": "2026-10-09",
        "note": "toy CPU mine+verify loop; derivations identical to crates/abacus-chain (v2)",
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
