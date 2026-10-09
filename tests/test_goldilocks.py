import random

import pytest

from reference import goldilocks as gl


def test_basic_edges():
    assert gl.mul(gl.P - 1, gl.P - 1) == 1
    assert gl.add(gl.P - 1, 1) == 0
    assert gl.sub(0, 1) == gl.P - 1
    assert gl.mul(0, 12345) == 0


def test_inverse():
    rng = random.Random(1)
    for _ in range(64):
        a = rng.randrange(1, gl.P)
        assert gl.mul(a, gl.inv(a)) == 1
    with pytest.raises(ZeroDivisionError):
        gl.inv(0)


def test_roots_of_unity_are_primitive():
    for k in range(1, 21):
        root = gl.root_of_unity(k)
        n = 1 << k
        assert gl.pow_mod(root, n) == 1
        assert gl.pow_mod(root, n >> 1) == gl.P - 1  # primitive: not a smaller order
