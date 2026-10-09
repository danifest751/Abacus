"""Candidate T (spec/07): deep requantized int8 network PoW with row-piece tickets.

Independent mirror of ``crates/abacus-chain/src/tnet.rs``; ``tests/test_tnet_parity.py`` compares it
with the Rust ``abacus-tnet vectors`` output. Pure Python, meant for small instances.
"""

from __future__ import annotations

from typing import List, Sequence

from reference.chain import le32, score_lead, sha256
from reference.int8 import expand_bytes

DOM_W = b"abacus/tnet-w"
DOM_X0 = b"abacus/tnet-x0"
TICKET_TAG = 0x54
REQ_SHIFT = 24


def le64(x: int) -> bytes:
    return x.to_bytes(8, "little")


def i8(b: int) -> int:
    return b - 256 if b >= 128 else b


def default_mult(n: int) -> int:
    return round((1 << REQ_SHIFT) / (74.0 * n ** 0.5))


def layer_weights(epoch_seed: bytes, n: int, l: int) -> List[int]:
    return [i8(x) for x in expand_bytes(sha256(DOM_W, epoch_seed, le32(l)), n * n)]


def x0_seed(header_digest: bytes, nonce: int) -> bytes:
    return sha256(DOM_X0, header_digest, le64(nonce))


def x0_row(seed: bytes, n: int, i: int) -> List[int]:
    return [i8(x) for x in expand_bytes(seed, (i + 1) * n)[i * n:]]


def requant(y: int, mult: int) -> int:
    v = (y * mult + (1 << (REQ_SHIFT - 1))) >> REQ_SHIFT  # arithmetic shift, as in Rust i64
    return max(-128, min(127, v))


def layer_row(x: Sequence[int], w: Sequence[int], n: int, mult: int) -> List[int]:
    return [requant(sum(x[k] * w[k * n + j] for k in range(n)), mult) for j in range(n)]


def forward_row(weights: Sequence[Sequence[int]], n: int, mult: int, seed: bytes, i: int) -> List[int]:
    x = x0_row(seed, n, i)
    for w in weights:
        x = layer_row(x, w, n, mult)
    return x


def encode(v: Sequence[int]) -> bytes:
    return bytes(x & 0xFF for x in v)


def ticket_hash(piece: Sequence[int], header_digest: bytes, nonce: int, i: int, c: int) -> bytes:
    return sha256(encode(piece), bytes([TICKET_TAG]), header_digest, le64(nonce), le32(i), le32(c))


def verify_ticket(weights, n, b, w, mult, header_digest, nonce, i, c, bits) -> bool:
    if i >= b or c >= n // w:
        return False
    row = forward_row(weights, n, mult, x0_seed(header_digest, nonce), i)
    return score_lead(ticket_hash(row[c * w:(c + 1) * w], header_digest, nonce, i, c)) >= bits
