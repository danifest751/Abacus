"""Sumcheck protocol over the Goldilocks field.

A multilinear polynomial ``f(x_1..x_n)`` is given by its evaluations on the Boolean hypercube,
``table[i] = f(bits of i)`` with bit ``j`` of ``i`` equal to ``x_{j+1}`` (LSB = variable 1). The
``sumcheck`` protocol proves ``S = sum_{b in {0,1}^n} f(b)`` with ``O(n)`` field elements and a
single evaluation of ``f`` at a random point; the verifier here recomputes that evaluation from the
table via the multilinear extension. This is the building block for verifying a claimed NTT (the
STARK/Plonk linear layer) without recomputing it.
"""

from __future__ import annotations

import random
from typing import List, Sequence, Tuple

from reference import goldilocks as gl


def full_sum(table: Sequence[int]) -> int:
    return sum(table) % gl.P


def _fold_first(table: List[int], r: int) -> List[int]:
    one_minus_r = gl.sub(1, r)
    return [gl.add(gl.mul(table[2 * i], one_minus_r), gl.mul(table[2 * i + 1], r)) for i in range(len(table) // 2)]


def eval_mle(table: Sequence[int], n: int, rs: Sequence[int]) -> int:
    """Multilinear extension ``f(r_1..r_n)`` computed by folding the table."""
    if len(table) != 1 << n or len(rs) != n:
        raise ValueError("shape")
    t = list(table)
    for r in rs:
        t = _fold_first(t, r)
    return t[0]


def prove(table: Sequence[int], n: int, rng: random.Random) -> Tuple[int, List[Tuple[int, int]], List[int]]:
    """Honest prover. Returns ``(claimed_sum, rounds, challenges)``.

    ``rounds[j] = (g_j(0), g_j(1))`` for the degree-1 round polynomial of variable ``j+1``.
    """
    if len(table) != 1 << n:
        raise ValueError("shape")
    t = list(table)
    claimed = full_sum(t)
    rounds: List[Tuple[int, int]] = []
    rs: List[int] = []
    for _ in range(n):
        g0 = sum(t[0::2]) % gl.P
        g1 = sum(t[1::2]) % gl.P
        rounds.append((g0, g1))
        r = rng.randrange(gl.P)
        rs.append(r)
        t = _fold_first(t, r)
    return claimed, rounds, rs


def verify(
    table: Sequence[int],
    n: int,
    claimed_sum: int,
    rounds: Sequence[Tuple[int, int]],
    rs: Sequence[int],
) -> bool:
    """Check a sumcheck transcript against the table."""
    if len(table) != 1 << n or len(rounds) != n or len(rs) != n:
        return False
    cur = claimed_sum % gl.P
    for (g0, g1), r in zip(rounds, rs):
        if gl.add(g0, g1) != cur:
            return False
        cur = gl.add(gl.mul(g0, gl.sub(1, r)), gl.mul(g1, r))
    return eval_mle(table, n, rs) == cur
