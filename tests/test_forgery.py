import os
import random
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import freivalds_forgery_probe as fp  # noqa: E402

from reference.freivalds import freivalds_verify, freivalds_verify_multi, matmul, random_matrix, random_vector  # noqa: E402


def test_single_challenge_is_forgeable():
    rng = random.Random(1)
    n = 8
    A = random_matrix(rng, n)
    B = random_matrix(rng, n)
    C = matmul(A, B, n)
    r = random_vector(rng, n)
    if r[0] == 0:
        r[0] = 1
    Cprime, nonzero = fp.forge(A, B, n, r, rng)
    assert nonzero and Cprime != C
    assert freivalds_verify(A, B, Cprime, n, r)  # forged product passes a single check


def test_fiat_shamir_multichallenge_rejects_forgery():
    rng = random.Random(2)
    n = 8
    k = 8
    A = random_matrix(rng, n)
    B = random_matrix(rng, n)
    C = matmul(A, B, n)
    r = random_vector(rng, n)
    if r[0] == 0:
        r[0] = 1
    Cprime, _ = fp.forge(A, B, n, r, rng)
    assert not freivalds_verify_multi(A, B, Cprime, n, fp.fs_challenges(b"ph", Cprime, n, k))
    assert freivalds_verify_multi(A, B, C, n, fp.fs_challenges(b"ph", C, n, k))
