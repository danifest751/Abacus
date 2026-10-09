import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import a8_proof_cost as pc  # noqa: E402


def test_ideal_commit_exceeds_gemm_at_moderate_n_and_shrinks_with_n():
    # The recorded test-1 conclusion: even the ideal per-attempt commitment costs more than the int8
    # GEMM up to n = 4096 and only falls below it for larger n.
    for n in (1024, 2048, 4096):
        assert pc.row(n, "babybear", 2)["overhead_ideal"] > 1.0
    assert pc.row(8192, "babybear", 2)["overhead_ideal"] < 1.0
    ratios = [pc.row(n, "babybear", 2)["overhead_ideal"] for n in (4096, 8192, 16384, 32768)]
    assert ratios == sorted(ratios, reverse=True)


def test_measured_overhead_and_proof_size_bounds():
    r = pc.row(4096, "babybear", 2)
    assert r["overhead_measured"] > 10
    assert 500 < pc.row(4096, "babybear", 4)["proof_KiB"] < pc.row(4096, "babybear", 2)["proof_KiB"] < 1024
