import random

from reference import goldilocks as gl
from reference import sumcheck


def _rand_table(rng, n):
    return [rng.randrange(gl.P) for _ in range(1 << n)]


def test_completeness():
    rng = random.Random(11)
    for n in range(1, 9):
        table = _rand_table(rng, n)
        claimed, rounds, rs = sumcheck.prove(table, n, rng)
        assert sumcheck.verify(table, n, claimed, rounds, rs)


def test_rejects_wrong_sum():
    rng = random.Random(12)
    n = 6
    table = _rand_table(rng, n)
    claimed, rounds, rs = sumcheck.prove(table, n, rng)
    assert not sumcheck.verify(table, n, gl.add(claimed, 1), rounds, rs)


def test_rejects_tampered_round():
    rng = random.Random(13)
    n = 6
    table = _rand_table(rng, n)
    claimed, rounds, rs = sumcheck.prove(table, n, rng)
    for j in range(n):
        for slot in (0, 1):
            bad = list(rounds)
            g0, g1 = bad[j]
            bad[j] = (gl.add(g0, 1), g1) if slot == 0 else (g0, gl.add(g1, 1))
            assert not sumcheck.verify(table, n, claimed, bad, rs)


def test_eval_mle_bilinear():
    # f(x1, x2) = x1 * x2 -> table (x1 as LSB) = [0, 0, 0, 1]
    table = [0, 0, 0, 1]
    assert sumcheck.eval_mle(table, 2, [2, 3]) == 6
    assert sumcheck.full_sum(table) == 1
