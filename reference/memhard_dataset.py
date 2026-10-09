"""Memory-hard epoch dataset for candidate A' (spec/04).

The dataset is a **sequential, data-dependent** chain of fixed-size blocks: block ``u`` depends on
block ``u-1`` and on a data-dependent earlier block ``ref(u)``. A random block therefore cannot be
recomputed cheaply without the prefix, so a miner must **store** the dataset to gather from it — the
memory-hard property. Determinstic and independent of any hardware.
"""

from __future__ import annotations

import hashlib
from typing import List

from reference.freivalds import P

DOM_DS = b"abacus/ds"
DOM_REF = b"abacus/ref"
DOM_IDX = b"abacus/idx"
BLOCK = 32  # bytes per dataset block


def _h(*parts: bytes) -> bytes:
    m = hashlib.sha256()
    for p in parts:
        m.update(p)
    return m.digest()


def ref(epoch_seed: bytes, u: int) -> int:
    if u == 0:
        raise ValueError("ref undefined for u=0")
    return int.from_bytes(_h(DOM_REF, epoch_seed, u.to_bytes(8, "little"))[:8], "little") % u


def build(epoch_seed: bytes, nblocks: int) -> List[bytes]:
    blocks = [_h(DOM_DS, epoch_seed, (0).to_bytes(8, "little"))]
    for u in range(1, nblocks):
        r = ref(epoch_seed, u)
        blocks.append(_h(DOM_DS, epoch_seed, u.to_bytes(8, "little"), blocks[u - 1], blocks[r]))
    return blocks


def expand_indices(seed: bytes, count: int, nblocks: int) -> List[int]:
    out = []
    c = 0
    while len(out) < count:
        h = _h(DOM_IDX, seed, c.to_bytes(4, "little"))
        for off in (0, 8, 16, 24):
            out.append(int.from_bytes(h[off:off + 8], "little") % nblocks)
        c += 1
    return out[:count]


def field_from_block(block: bytes) -> int:
    return int.from_bytes(block[:8], "little") % P


def gather_field(blocks: List[bytes], idx: int) -> int:
    return field_from_block(blocks[idx])


def recompute_vs_store(u: int) -> dict:
    """Cost to obtain block u: rebuilding the prefix (u+1 hashes) vs a stored read (1 access)."""
    return {"block": u, "recompute_hashes": u + 1, "store_read": 1, "ratio": u + 1}
