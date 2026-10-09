import os
import random
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import instance_probe as ip  # noqa: E402


def test_structure_collapse_correct_and_rank():
    rng = random.Random(2)
    for n in (8, 12):
        for r in (1, 2):
            res = ip.structure_collapse(n, r, rng)
            assert res["correct"]
            assert res["rank_A"] == r
            assert res["rank_random_B"] == n
            assert abs(res["analytic_ratio"] - n / (2.0 * r)) < 1e-9


def test_dense_random_is_full_rank():
    rng = random.Random(9)
    assert ip.rank_mod_p(ip.random_matrix(rng, 8), 8, 8) == 8
