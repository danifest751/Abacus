import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

import memhard_balance_probe as mb  # noqa: E402

from reference import memhard_dataset as md  # noqa: E402


def test_dataset_is_deterministic_and_dependent():
    seed = b"epoch-0"
    blocks = md.build(seed, 16)
    assert len(blocks) == 16
    assert blocks == md.build(seed, 16)          # deterministic
    assert blocks != md.build(b"epoch-1", 16)    # seed-dependent
    for u in range(1, 16):
        assert 0 <= md.ref(seed, u) < u          # data-dependent earlier reference


def test_gather_and_recompute_costs():
    blocks = md.build(b"s", 32)
    idx = md.expand_indices(b"x", 10, 32)
    assert len(idx) == 10 and all(0 <= i < 32 for i in idx)
    assert isinstance(md.gather_field(blocks, idx[0]), int)
    rc = md.recompute_vs_store(1000)
    assert rc["recompute_hashes"] == 1001 and rc["store_read"] == 1


def test_balance_regime():
    # With one 32-byte block per A entry, the matmul dominates (memory layer cosmetic).
    r = mb.row(512, mb.RATE_FIELD)
    assert r["gather_over_matmul"] < 1.0
    # To make the gather dominate, the per-attempt gather must be much larger than one block/entry.
    assert r["required_gather_bytes_for_balance"] > r["gathered_bytes_one_block_per_entry"]
    n, rate = 256, mb.RATE_FIELD
    assert abs(mb.required_gather_bytes_for_balance(n, rate) - (n**3) * mb.BW_G / rate) < 1
