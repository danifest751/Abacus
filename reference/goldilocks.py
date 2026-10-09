"""Goldilocks field: arithmetic modulo ``P = 2**64 - 2**32 + 1``.

This is the field used by modern STARK/FRI provers (and by Poseidon2 over Goldilocks). Its prime
has 2-adicity 32, so it supports NTT/FFT sizes up to ``2**32`` — the reason the NTT/STARK
candidate in this laboratory, and by the matmul/Freivalds laboratory (candidate A).

Generator: ``7``. ``P - 1 = 2**32 * (2**32 - 1)``, so ``7 ** ((P - 1) >> k)`` is a primitive
``2**k``-th root of unity.
"""

from __future__ import annotations

P = 0xFFFFFFFF00000001  # 2**64 - 2**32 + 1
TWO_ADICITY = 32
GENERATOR = 7


def add(a: int, b: int) -> int:
    return (a + b) % P


def sub(a: int, b: int) -> int:
    return (a - b) % P


def neg(a: int) -> int:
    return (-a) % P


def mul(a: int, b: int) -> int:
    return (a * b) % P


def pow_mod(a: int, e: int) -> int:
    return pow(a, e, P)


def inv(a: int) -> int:
    if a % P == 0:
        raise ZeroDivisionError("inverse of 0")
    return pow(a, P - 2, P)


def root_of_unity(k: int) -> int:
    """A primitive ``2**k``-th root of unity (``k <= 32``)."""
    if not 0 <= k <= TWO_ADICITY:
        raise ValueError("k out of range")
    return pow(GENERATOR, (P - 1) >> k, P)
