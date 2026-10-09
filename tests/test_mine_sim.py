import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import mine_sim as ms  # noqa: E402


def test_toy_mine_and_verify_roundtrip():
    n = 8
    k = 8
    res = ms.mine(n, target_bits=4, max_attempts=5000)
    assert res["found"], "target 4 bits should be found within 5000 attempts"
    assert ms.verify(res["A"], res["B"], res["C"], n, k)


def test_toy_verify_rejects_wrong_product():
    n = 8
    k = 8
    res = ms.mine(n, target_bits=4, max_attempts=5000)
    C = list(res["C"])
    C[0] = (C[0] + 1) % ms.P
    assert not ms.verify(res["A"], res["B"], C, n, k)
