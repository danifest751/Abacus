"""Memory-hard epoch dataset for candidate A' (spec/04).

The dataset is a **sequential, data-dependent** chain of 32-byte blocks: block ``u`` depends on
block ``u-1`` and on an earlier block ``ref(u)`` whose index is read from the content of block
``u-1`` (Argon2d-style), so the access pattern is not known before the data. The derivation is the
one in ``reference.chain`` (identical to the Rust chain); this module re-exports it and keeps the
dataset-specific helpers.

Caveats (ADR 0010): only the first 8 bytes of a block are consumed by the gather, so a miner must
store ``8 * nblocks`` bytes, not ``32 * nblocks``; and no time-memory trade-off analysis
(checkpointing, pebbling) has been done. ``recompute_vs_store`` is the naive no-checkpoint cost,
not a lower bound.
"""

from __future__ import annotations

from typing import List

from reference.chain import (  # noqa: F401  (re-exported)
    build_dataset as build,
    dataset_ref,
    expand_indices,
    field_from_block,
)

BLOCK = 32  # bytes per dataset block
CONSUMED = 8  # bytes of each block actually read by the gather


def ref(blocks: List[bytes], u: int) -> int:
    """``ref(u)`` for an already built dataset."""
    return dataset_ref(blocks[u - 1], u)


def gather_field(blocks: List[bytes], idx: int) -> int:
    return field_from_block(blocks[idx])


def recompute_vs_store(u: int) -> dict:
    """Naive cost to obtain block ``u`` with no stored checkpoints (``u+1`` hashes) vs a stored read.

    Not a lower bound: a miner storing every c-th block plus the references it needs pays far less.
    """
    return {"block": u, "recompute_hashes_naive": u + 1, "store_read": 1, "ratio_naive": u + 1}


def storage_bytes(nblocks: int) -> int:
    """Bytes a miner must keep to gather from the dataset (only the consumed prefix of each block)."""
    return CONSUMED * nblocks
