import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import freivalds_soundness_probe as sp  # noqa: E402


def test_per_challenge_error_is_one_over_p_not_one_half():
    run = sp.measure(n=6, p=101, trials=20000, seed=1)
    # Worst-case E (one nonzero entry): field challenges err ~1/p, binary challenges ~1/2.
    assert abs(run["field_r_worst_E"] - 1 / 101) < 0.004
    assert abs(run["binary_r_worst_E"] - 0.5) < 0.02
    # A random nonzero E almost never passes a field challenge.
    assert run["field_r_random_E"] <= 1 / 101
