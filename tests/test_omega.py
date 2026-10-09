import math
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import omega_probe as op  # noqa: E402


def test_strassen_beats_naive_multiplies():
    base = 64
    for k in range(7, 13):
        n = 1 << k
        assert op.strassen_mults(n, base) < op.naive_mults(n)


def test_strassen_scales_by_seven():
    base = 64
    # Above the base, a doubling multiplies the multiply count by exactly 7.
    assert op.strassen_mults(2 * 128, base) == 7 * op.strassen_mults(128, base)


def test_work_saved_exponent_is_three_minus_log2_7():
    assert abs((3.0 - math.log2(7)) - 0.1926) < 0.01


def test_strassen_total_ops_fewer_at_scale():
    base = 64
    n = 1 << 12
    strassen_total = op.strassen_mults(n, base) + op.strassen_adds(n, base)
    naive_total = n**3 + n * n * (n - 1)
    assert strassen_total < naive_total
