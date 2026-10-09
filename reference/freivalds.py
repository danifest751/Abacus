"""Abacus research laboratory — transparent Python reference verifier.

Field: arithmetic modulo Goldilocks ``P = 2**64 - 2**32 + 1`` (a prime). Matrices are row-major ``n x n`` lists of
residues. The verifier implements **Freivalds' algorithm**: given a claimed product ``C`` of
``A`` and ``B``, it checks ``A @ (B @ r) == C @ r`` for a random vector ``r`` (``O(n**2)`` instead
of ``O(n**3)``). It tests the *result*, not that a miner did the work.
"""

from __future__ import annotations

import random
from typing import List, Sequence

P = 0xFFFFFFFF00000001  # 2**64 - 2**32 + 1 (Goldilocks), prime

Matrix = List[int]
Vector = List[int]


def mul_mod(a: int, b: int) -> int:
    return (a * b) % P


def add_mod(a: int, b: int) -> int:
    return (a + b) % P


def matmul(a: Sequence[int], b: Sequence[int], n: int) -> Matrix:
    if len(a) != n * n or len(b) != n * n:
        raise ValueError("matrix shape")
    c = [0] * (n * n)
    for i in range(n):
        for k in range(n):
            aik = a[i * n + k]
            if aik == 0:
                continue
            for j in range(n):
                idx = i * n + j
                c[idx] = (c[idx] + aik * b[k * n + j]) % P
    return c


def matvec(m: Sequence[int], v: Sequence[int], n: int) -> Vector:
    if len(m) != n * n or len(v) != n:
        raise ValueError("shape")
    out = [0] * n
    for i in range(n):
        acc = 0
        for j in range(n):
            acc = (acc + m[i * n + j] * v[j]) % P
        out[i] = acc
    return out


def freivalds_verify(a: Sequence[int], b: Sequence[int], c: Sequence[int], n: int, r: Sequence[int]) -> bool:
    """Check ``A @ (B @ r) == C @ r``."""
    return matvec(a, matvec(b, r, n), n) == matvec(c, r, n)


def freivalds_verify_multi(a, b, c, n: int, rs: Sequence[Sequence[int]]) -> bool:
    if not rs:
        return False
    return all(freivalds_verify(a, b, c, n, r) for r in rs)


def random_matrix(rng: random.Random, n: int) -> Matrix:
    return [rng.randrange(P) for _ in range(n * n)]


def random_vector(rng: random.Random, n: int) -> Vector:
    return [rng.randrange(P) for _ in range(n)]
