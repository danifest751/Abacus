import random

import pytest

from reference import goldilocks as gl
from reference import ntt


def test_round_trip():
    rng = random.Random(3)
    for k in range(1, 11):
        n = 1 << k
        a = [rng.randrange(gl.P) for _ in range(n)]
        assert ntt.inverse(ntt.forward(a)) == a


def test_matches_naive_dft():
    rng = random.Random(4)
    for k in range(1, 8):
        n = 1 << k
        a = [rng.randrange(gl.P) for _ in range(n)]
        assert ntt.forward(a) == ntt.naive_dft(a)


def test_is_linear():
    rng = random.Random(5)
    n = 16
    a = [rng.randrange(gl.P) for _ in range(n)]
    b = [rng.randrange(gl.P) for _ in range(n)]
    fa = ntt.forward(a)
    fb = ntt.forward(b)
    fab = ntt.forward([gl.add(x, y) for x, y in zip(a, b)])
    assert fab == [gl.add(x, y) for x, y in zip(fa, fb)]


def test_bad_length():
    with pytest.raises(ValueError):
        ntt.forward([1, 2, 3])
