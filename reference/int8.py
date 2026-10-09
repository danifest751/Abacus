"""Candidate A8 (spec/05): int8 instance, exact int32 product, Fiat-Shamir Freivalds over Goldilocks.

Mirrors ``crates/abacus-verifier/src/int8.rs`` and the A8 functions of ``crates/abacus-chain``;
``tests/test_chain_parity.py`` compares them with Rust.
"""

from __future__ import annotations

from typing import List, Sequence, Tuple

from reference.chain import DOM_EXPAND, le32, sha256
from reference.freivalds import P, freivalds_verify_multi

DOM_INSTANCE_I8 = b"abacus/instance-i8"
DOM_SCORE_I8 = b"abacus/score-i8"
DOM_CHECK_I8 = b"abacus/check-i8"
MAX_N = 1 << 16  # MAX_N * 128 * 128 < 2^31


def expand_bytes(seed: bytes, count: int) -> bytes:
    out = bytearray()
    c = 0
    while len(out) < count:
        out += sha256(DOM_EXPAND, seed, le32(c))
        c += 1
    return bytes(out[:count])


def instance_i8(ph: bytes, n: int) -> Tuple[List[int], List[int]]:
    raw = expand_bytes(sha256(DOM_INSTANCE_I8, ph), 2 * n * n)
    v = [x - 256 if x > 127 else x for x in raw]
    return v[: n * n], v[n * n:]


def matmul_i8(a: Sequence[int], b: Sequence[int], n: int) -> List[int]:
    if n > MAX_N or len(a) != n * n or len(b) != n * n:
        raise ValueError("shape")
    c = [0] * (n * n)
    for i in range(n):
        for k in range(n):
            aik = a[i * n + k]
            if aik:
                row = b[k * n:(k + 1) * n]
                for j in range(n):
                    c[i * n + j] += aik * row[j]
    return c


def encode_c_i32(c: Sequence[int]) -> bytes:
    return b"".join(int(x).to_bytes(4, "little", signed=True) for x in c)


def score_i8(ph: bytes, c: Sequence[int]) -> bytes:
    return sha256(DOM_SCORE_I8, ph, encode_c_i32(c))


def fs_challenges_i8(ph: bytes, c: Sequence[int], n: int, k: int) -> List[List[int]]:
    root = sha256(DOM_CHECK_I8, ph, encode_c_i32(c))
    vals: List[int] = []
    counter = 0
    while len(vals) < k * n:
        h = sha256(root, le32(counter))
        for off in (0, 8, 16, 24):
            vals.append(int.from_bytes(h[off:off + 8], "little") % P)
        counter += 1
    return [vals[i * n:(i + 1) * n] for i in range(k)]


def verify_fs_i8(ph: bytes, a, b, c, n: int, k: int) -> bool:
    if k <= 0 or n > MAX_N or len(a) != n * n or len(b) != n * n or len(c) != n * n:
        return False
    if any(not -(1 << 31) <= x < (1 << 31) for x in c):
        return False
    af, bf, cf = ([x % P for x in v] for v in (a, b, c))
    return freivalds_verify_multi(af, bf, cf, n, fs_challenges_i8(ph, c, n, k))
