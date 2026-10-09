import random

import pytest

from reference.freivalds import (
    P,
    add_mod,
    freivalds_verify,
    freivalds_verify_multi,
    matmul,
    mul_mod,
    random_matrix,
    random_vector,
)


def test_field_edges():
    assert mul_mod(P - 1, P - 1) == 1
    assert add_mod(P - 1, 1) == 0
    assert mul_mod(0, 12345) == 0


def test_accepts_true_product():
    rng = random.Random(42)
    n = 12
    a = random_matrix(rng, n)
    b = random_matrix(rng, n)
    c = matmul(a, b, n)
    rs = [random_vector(rng, n) for _ in range(8)]
    assert freivalds_verify_multi(a, b, c, n, rs)


def test_rejects_tampered_product():
    rng = random.Random(7)
    n = 24
    a = random_matrix(rng, n)
    b = random_matrix(rng, n)
    c = matmul(a, b, n)
    c[0] = (c[0] + 1) % P
    rs = [random_vector(rng, n) for _ in range(8)]
    assert not freivalds_verify_multi(a, b, c, n, rs)


def test_empty_challenge_rejected():
    rng = random.Random(1)
    n = 4
    a = random_matrix(rng, n)
    b = random_matrix(rng, n)
    c = matmul(a, b, n)
    assert not freivalds_verify_multi(a, b, c, n, [])


def test_shape_errors():
    with pytest.raises(ValueError):
        matmul([1, 2, 3], [1, 2, 3], 2)
