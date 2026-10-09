import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import memhard_balance_probe as mb  # noqa: E402

from reference import memhard_dataset as md  # noqa: E402


def test_dataset_is_deterministic_and_seed_dependent():
    seed = b"epoch-0"
    blocks = md.build(seed, 16)
    assert len(blocks) == 16
    assert blocks == md.build(seed, 16)          # deterministic
    assert blocks != md.build(b"epoch-1", 16)    # seed-dependent
    for u in range(1, 16):
        assert 0 <= md.ref(blocks, u) < u        # an earlier block


def test_reference_is_read_from_data():
    blocks = md.build(b"s", 64)
    prev = blocks[10]
    tweaked = bytes([prev[0] ^ 1]) + prev[1:]
    assert any(md.dataset_ref(prev, u) != md.dataset_ref(tweaked, u) for u in range(11, 64))


def test_gather_storage_and_naive_recompute_costs():
    blocks = md.build(b"s", 32)
    idx = md.expand_indices(b"x", 10, 32)
    assert len(idx) == 10 and all(0 <= i < 32 for i in idx)
    assert isinstance(md.gather_field(blocks, idx[0]), int)
    assert md.storage_bytes(1000) == 8 * 1000  # only the consumed 8 bytes per block need storing
    rc = md.recompute_vs_store(1000)
    assert rc["recompute_hashes_naive"] == 1001 and rc["store_read"] == 1


def test_balance_regime():
    # Prototype (one 8-byte word per entry): the matmul dominates, so it is not memory-hard.
    for n in (256, 512, 1024):
        assert mb.row(n)["prototype_gather_over_matmul"] < 1.0
    # Large nonlinear slices: the cooperative gather dominates once seg exceeds the balance size.
    r = mb.row(256, 2560)
    assert r["balance_seg_bytes"] < 2560 and r["slice_gather_over_matmul"] > 1.0
    # The balance segment grows linearly with n (matmul n^3 vs gather n^2).
    assert abs(mb.balance_segment_bytes(512) / mb.balance_segment_bytes(256) - 2.0) < 1e-9


def test_linear_segment_fold_collapses_with_prefix_sums():
    # ADR 0010: a gather that folds a segment by a plain sum is answered by two prefix-sum reads, so
    # it is not memory-hard. The fold must be nonlinear.
    import random

    rng = random.Random(3)
    m = 1 << 64
    data = [rng.getrandbits(64) for _ in range(5000)]
    pre = [0]
    for x in data:
        pre.append((pre[-1] + x) % m)
    for off, seg in ((0, 320), (1234, 320), (4000, 999)):
        assert sum(data[off:off + seg]) % m == (pre[off + seg] - pre[off]) % m
