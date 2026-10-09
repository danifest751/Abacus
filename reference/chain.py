"""Consensus derivations of the prototype, mirroring ``crates/abacus-chain`` byte for byte.

This is the single Python reference for the encodings used by the Rust chain and by CPPminer:
preheader (v2, ``bits`` committed), instance expansion, score, block id, the Fiat-Shamir Freivalds
challenges bound to ``(preheader, C)``, the candidate A' dataset (data-dependent references) and the
gathered instance. ``tests/test_chain_parity.py`` checks every function against the Rust
``abacus-vectors`` binary.
"""

from __future__ import annotations

import hashlib
from typing import List, Sequence, Tuple

from reference.freivalds import P, freivalds_verify_multi

DOM_INSTANCE = b"abacus/instance"
DOM_SCORE = b"abacus/score"
DOM_BLOCK = b"abacus/block"
DOM_EXPAND = b"abacus/expand"
DOM_PH = b"abacus/ph"
DOM_CHECK = b"abacus/check"
DOM_DS = b"abacus/ds"


def sha256(*parts: bytes) -> bytes:
    m = hashlib.sha256()
    for p in parts:
        m.update(p)
    return m.digest()


def le64(x: int) -> bytes:
    return int(x).to_bytes(8, "little")


def le32(x: int) -> bytes:
    return int(x).to_bytes(4, "little")


def expand(seed: bytes, count: int) -> List[int]:
    """SHA-256 counter mode, four Goldilocks elements (LE64 mod P) per hash."""
    out: List[int] = []
    c = 0
    while len(out) < count:
        h = sha256(DOM_EXPAND, seed, le32(c))
        for off in (0, 8, 16, 24):
            out.append(int.from_bytes(h[off:off + 8], "little") % P)
        c += 1
    return out[:count]


def preheader(chain_id: bytes, version: int, height: int, prev: bytes, timestamp: int, bits: int,
              nonce: int) -> bytes:
    return DOM_PH + chain_id + le32(version) + prev + le64(height) + le64(timestamp) + le32(bits) + le64(nonce)


def instance_seed(ph: bytes) -> bytes:
    return sha256(DOM_INSTANCE, ph)


def instance(ph: bytes, n: int) -> Tuple[List[int], List[int]]:
    v = expand(instance_seed(ph), 2 * n * n)
    return v[:n * n], v[n * n:]


def encode_c(c: Sequence[int]) -> bytes:
    return b"".join(le64(x) for x in c)


def score(ph: bytes, c: Sequence[int]) -> bytes:
    return sha256(DOM_SCORE, ph, encode_c(c))


def block_id(ph: bytes, c: Sequence[int]) -> bytes:
    return sha256(DOM_BLOCK, ph, encode_c(c))


def score_lead(sc: bytes) -> int:
    lead = 0
    for b in sc:
        if b == 0:
            lead += 8
        else:
            lead += 8 - b.bit_length()
            break
    return lead


def fs_challenges(ph: bytes, c: Sequence[int], n: int, k: int) -> List[List[int]]:
    """``k`` challenge vectors bound to ``(preheader, C)``: commit-then-expand (ADR 0006)."""
    root = sha256(DOM_CHECK, ph, encode_c(c))
    vals: List[int] = []
    counter = 0
    while len(vals) < k * n:
        h = sha256(root, le32(counter))
        for off in (0, 8, 16, 24):
            vals.append(int.from_bytes(h[off:off + 8], "little") % P)
        counter += 1
    return [vals[i * n:(i + 1) * n] for i in range(k)]


def verify_fs(ph: bytes, a, b, c, n: int, k: int) -> bool:
    if k <= 0 or len(a) != n * n or len(b) != n * n or len(c) != n * n or any(x >= P for x in c):
        return False
    return freivalds_verify_multi(a, b, c, n, fs_challenges(ph, c, n, k))


# ---- candidate A' (spec/04) ----

def dataset_ref(prev_block: bytes, u: int) -> int:
    """Data-dependent reference of block ``u >= 1``: ``LE64(blk[u-1][:8]) mod u``."""
    if u <= 0:
        raise ValueError("ref undefined for u=0")
    return int.from_bytes(prev_block[:8], "little") % u


def build_dataset(epoch_seed: bytes, nblocks: int) -> List[bytes]:
    if nblocks <= 0:
        return []
    blocks = [sha256(DOM_DS, epoch_seed, le64(0))]
    for u in range(1, nblocks):
        r = dataset_ref(blocks[u - 1], u)
        blocks.append(sha256(DOM_DS, epoch_seed, le64(u), blocks[u - 1], blocks[r]))
    return blocks


def field_from_block(block: bytes) -> int:
    """Only the first 8 bytes of a block are consumed (spec/04 section 2: storage is 8 B per block)."""
    return int.from_bytes(block[:8], "little") % P


def expand_indices(seed: bytes, count: int, nblocks: int) -> List[int]:
    return [x % nblocks for x in expand(seed, count)]


def instance_hard(ph: bytes, n: int, dataset: Sequence[bytes]) -> Tuple[List[int], List[int]]:
    total = n * n
    idx = expand_indices(instance_seed(ph), 2 * total, len(dataset))
    a = [field_from_block(dataset[idx[i]]) for i in range(total)]
    b = [field_from_block(dataset[idx[i]]) for i in range(total, 2 * total)]
    return a, b
