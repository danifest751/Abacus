#!/usr/bin/env python3
"""A8 test 1 (ADR 0012): cost of a succinct proof for C = A * B versus the int8 GEMM itself.

Why per attempt: the score must bind all of C (a linear score is computable as A (B r) without the
product; a raw-hash score cannot be linked to a separate proof commitment without a SNARK of the hash,
and sampled consistency checks are grindable). So the score is H(preheader || root) where `root` is the
proof commitment to C, and the commitment is built on **every attempt**. Per attempt:

  work = GEMM(n) + commit(C)        commit = RS-encode N = n^2 entries at rate rho, Merkle-hash the codeword
  (the matmul sumcheck itself is O(n^2) field operations and is ignored here: it favours A8)

Two commit estimates from measured CMP 50HX rates:
  measured : our fused NTT (`artifacts/ntt-run1.txt`) + SHA-256 of the codeword at 190 GB/s;
  ideal    : lower bound for any implementation — 3 passes of the codeword over DRAM at the sequential
             538 GB/s + SHA-256 of the codeword at 195 GB/s (FRI folding, transposes and twiddles ignored).
GEMM: measured cuBLAS int8 for n <= 8192 (`int8-matmul-v1`); larger n extrapolated at 78 TMAC/s.

Proof size (FRI-style, lambda = 100 bits, fold by 2 down to 2^8, 32-byte digests, 16-byte extension
elements, no path pruning): q = ceil(lambda / log2(1/rho)) queries, each opening one Merkle path and two
elements per round.

Writes JSON to an ignored `artifacts/` and prints a table.
"""

from __future__ import annotations

import json
import math
import os

# Measured cuBLAS int8 GEMM (ms), CMP 50HX, run 5.
GEMM_MS = {1024: 0.0434, 2048: 0.1963, 4096: 1.0229, 8192: 7.0796}
GEMM_RATE = 78e12  # MAC/s for extrapolation beyond 8192
# Measured fused NTT (ms) by log2(length), CMP 50HX (ntt-run1). BabyBear two-adicity limits it to 2^27.
NTT_MS = {
    "babybear": {20: 0.262, 21: 0.713, 22: 1.954, 23: 4.567, 24: 10.523, 25: 24.828, 26: 56.922, 27: 129.89},
    "goldilocks": {20: 0.604, 21: 1.402, 22: 3.171, 23: 7.148, 24: 16.398, 25: 36.873, 26: 82.479, 27: 183.339},
}
ELEM_BYTES = {"babybear": 4, "goldilocks": 8}
SHA_GBPS_MEASURED = 190e9
SHA_GBPS_PEAK = 195e9
DRAM_BPS = 538e9
LAMBDA = 100


def gemm_ms(n: int) -> float:
    return GEMM_MS.get(n, n**3 / GEMM_RATE * 1e3)


def commit_ms(n: int, field: str, rho_inv: int, model: str) -> float | None:
    log_len = int(math.log2(n * n * rho_inv))
    code_bytes = (n * n * rho_inv) * ELEM_BYTES[field]
    if model == "measured":
        ntt = NTT_MS[field].get(log_len)
        if ntt is None:
            return None
        return ntt + code_bytes / SHA_GBPS_MEASURED * 1e3
    # ideal lower bound: read C, write codeword, read codeword (3 passes) + hash the codeword
    return 3 * code_bytes / DRAM_BPS * 1e3 + code_bytes / SHA_GBPS_PEAK * 1e3


def proof_bytes(n: int, rho_inv: int) -> int:
    log_len = int(math.log2(n * n * rho_inv))
    q = math.ceil(LAMBDA / math.log2(rho_inv))
    rounds = max(0, log_len - 8)
    per_query = sum(32 * (log_len - r) + 2 * 16 for r in range(rounds))
    return q * per_query


def row(n: int, field: str, rho_inv: int) -> dict:
    g = gemm_ms(n)
    m = commit_ms(n, field, rho_inv, "measured")
    i = commit_ms(n, field, rho_inv, "ideal")
    return {
        "n": n,
        "field": field,
        "rho_inv": rho_inv,
        "gemm_ms": g,
        "commit_measured_ms": m,
        "commit_ideal_ms": i,
        "overhead_measured": None if m is None else m / g,
        "overhead_ideal": i / g,
        "tensor_share_ideal": g / (g + i),
        "proof_KiB": proof_bytes(n, rho_inv) / 1024,
        "C_MiB": n * n * 4 / 2**20,
    }


def main() -> int:
    rows = [row(n, f, r) for n in (1024, 2048, 4096, 8192, 16384, 32768) for f in ("babybear", "goldilocks")
            for r in (2, 4)]
    out = {"date": "2026-10-09", "note": "A8 test 1: per-attempt commitment cost vs int8 GEMM", "runs": rows}
    os.makedirs("artifacts", exist_ok=True)
    with open(os.path.join("artifacts", "a8-proof-cost.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2)
    print(f"{'n':>6} {'field':>10} {'1/rho':>5} {'gemm ms':>9} {'commit ms':>10} {'ideal ms':>9} "
          f"{'ovh meas':>8} {'ovh ideal':>9} {'tensor%':>7} {'proof KiB':>9}")
    for r in rows:
        meas = "-" if r["commit_measured_ms"] is None else f"{r['commit_measured_ms']:.2f}"
        ovm = "-" if r["overhead_measured"] is None else f"{r['overhead_measured']:.1f}"
        print(f"{r['n']:>6} {r['field']:>10} {r['rho_inv']:>5} {r['gemm_ms']:>9.3f} {meas:>10} "
              f"{r['commit_ideal_ms']:>9.3f} {ovm:>8} {r['overhead_ideal']:>9.2f} "
              f"{100 * r['tensor_share_ideal']:>6.0f}% {r['proof_KiB']:>9.0f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
