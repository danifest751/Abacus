import random

from reference import goldilocks as gl
from reference import sumcheck


def _rand_table(rng, n):
    return [rng.randrange(gl.P) for _ in range(1 << n)]


def test_completeness():
    rng = random.Random(11)
    for n in range(1, 9):
        table = _rand_table(rng, n)
        claimed, rounds = sumcheck.prove(table, n)
        assert sumcheck.verify(table, n, claimed, rounds)


def test_rejects_wrong_sum():
    rng = random.Random(12)
    n = 6
    table = _rand_table(rng, n)
    claimed, rounds = sumcheck.prove(table, n)
    assert not sumcheck.verify(table, n, gl.add(claimed, 1), rounds)


def test_rejects_tampered_round():
    rng = random.Random(13)
    n = 6
    table = _rand_table(rng, n)
    claimed, rounds = sumcheck.prove(table, n)
    for j in range(n):
        for slot in (0, 1):
            bad = list(rounds)
            g0, g1 = bad[j]
            bad[j] = (gl.add(g0, 1), g1) if slot == 0 else (g0, gl.add(g1, 1))
            assert not sumcheck.verify(table, n, claimed, bad)


def _zero_challenge_forgery(table, n, fake):
    """Rounds that pass every sum check if the challenges were all 0 (prover-chosen)."""
    rounds, cur, t = [], fake, list(table)
    for _ in range(n):
        g0 = sum(t[0::2]) % gl.P
        rounds.append((g0, gl.sub(cur, g0)))
        cur, t = g0, t[0::2]
    return rounds


def test_zero_challenge_forgery_is_rejected():
    rng = random.Random(14)
    n = 4
    table = _rand_table(rng, n)
    fake = gl.add(sumcheck.full_sum(table), 12345)
    rounds = _zero_challenge_forgery(table, n, fake)
    # The forgery would pass with r = 0 (what an old, prover-challenge verifier accepted) ...
    cur = fake
    for g0, g1 in rounds:
        assert gl.add(g0, g1) == cur
        cur = g0
    assert sumcheck.eval_mle(table, n, [0] * n) == cur
    # ... but the Fiat-Shamir verifier recomputes the challenges and rejects it.
    assert not sumcheck.verify(table, n, fake, rounds)


def test_eval_mle_bilinear():
    # f(x1, x2) = x1 * x2 -> table (x1 as LSB) = [0, 0, 0, 1]
    table = [0, 0, 0, 1]
    assert sumcheck.eval_mle(table, 2, [2, 3]) == 6
    assert sumcheck.full_sum(table) == 1
