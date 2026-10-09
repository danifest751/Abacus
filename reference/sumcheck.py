"""Sumcheck protocol over the Goldilocks field, non-interactive (Fiat-Shamir).

A multilinear polynomial ``f(x_1..x_n)`` is given by its evaluations on the Boolean hypercube,
``table[i] = f(bits of i)`` with bit ``j`` of ``i`` equal to ``x_{j+1}`` (LSB = variable 1). The
protocol proves ``S = sum_{b in {0,1}^n} f(b)`` with ``O(n)`` field elements. The challenges are
derived by the **verifier** from a running transcript hash (table, ``n``, claimed sum, rounds so far);
a verifier that accepts prover-chosen challenges is forgeable (any sum passes with ``r_j = 0``). The
verifier recomputes ``f(r)`` from the full table, so it is ``O(2^n)``: a building block, not a
succinct verifier. Mirrors ``crates/abacus-verifier/src/sumcheck.rs``.
"""

from __future__ import annotations

import hashlib
from typing import List, Sequence, Tuple

from reference import goldilocks as gl

DOM_SUMCHECK = b"abacus/sumcheck"


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


def _le64(x: int) -> bytes:
    return int(x).to_bytes(8, "little")


def transcript_init(table: Sequence[int], n: int, claimed: int) -> bytes:
    body = DOM_SUMCHECK + _le64(n) + _le64(len(table)) + b"".join(_le64(x) for x in table) + _le64(claimed)
    return hashlib.sha256(body).digest()


def transcript_round(state: bytes, g0: int, g1: int) -> Tuple[bytes, int]:
    nxt = hashlib.sha256(state + _le64(g0) + _le64(g1)).digest()
    return nxt, int.from_bytes(nxt[:8], "little") % gl.P


def challenges(table: Sequence[int], n: int, claimed: int, rounds: Sequence[Tuple[int, int]]) -> List[int]:
    state = transcript_init(table, n, claimed)
    rs = []
    for g0, g1 in rounds:
        state, r = transcript_round(state, g0, g1)
        rs.append(r)
    return rs


def prove(table: Sequence[int], n: int) -> Tuple[int, List[Tuple[int, int]]]:
    """Honest prover. Returns ``(claimed_sum, rounds)``, ``rounds[j] = (g_j(0), g_j(1))``."""
    if len(table) != 1 << n:
        raise ValueError("shape")
    t = list(table)
    claimed = full_sum(t)
    state = transcript_init(table, n, claimed)
    rounds: List[Tuple[int, int]] = []
    for _ in range(n):
        g0 = sum(t[0::2]) % gl.P
        g1 = sum(t[1::2]) % gl.P
        rounds.append((g0, g1))
        state, r = transcript_round(state, g0, g1)
        t = _fold_first(t, r)
    return claimed, rounds


def verify(table: Sequence[int], n: int, claimed_sum: int, rounds: Sequence[Tuple[int, int]]) -> bool:
    """Check a transcript against the table; challenges are recomputed, never taken from the prover."""
    if len(table) != 1 << n or len(rounds) != n or not 0 <= claimed_sum < gl.P:
        return False
    if any(not (0 <= g0 < gl.P and 0 <= g1 < gl.P) for g0, g1 in rounds):
        return False
    rs = challenges(table, n, claimed_sum, rounds)
    cur = claimed_sum
    for (g0, g1), r in zip(rounds, rs):
        if gl.add(g0, g1) != cur:
            return False
        cur = gl.add(gl.mul(g0, gl.sub(1, r)), gl.mul(g1, r))
    return eval_mle(table, n, rs) == cur
