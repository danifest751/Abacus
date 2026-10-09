"""Number-theoretic transform over the Goldilocks field (iterative Cooley-Tukey).

The transform size is a power of two ``n = 2**k`` with ``k <= TWO_ADICITY``. ``forward`` computes
``y_j = sum_i a_i * w**(i*j)`` for a primitive ``n``-th root ``w``; ``inverse`` undoes it. This is
the GPU-optimal primitive studied by the NTT candidate work function.
"""

from __future__ import annotations

from typing import List

from reference import goldilocks as gl


def _bit_reverse(a: List[int]) -> None:
    n = len(a)
    j = 0
    for i in range(1, n):
        bit = n >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j |= bit
        if i < j:
            a[i], a[j] = a[j], a[i]


def _transform(a: List[int], inverse: bool) -> None:
    n = len(a)
    if n == 0 or (n & (n - 1)):
        raise ValueError("length must be a power of two")
    k = n.bit_length() - 1
    if k > gl.TWO_ADICITY:
        raise ValueError("length exceeds 2-adicity")
    _bit_reverse(a)
    length = 2
    while length <= n:
        wlen = gl.pow_mod(gl.GENERATOR, (gl.P - 1) // length)
        if inverse:
            wlen = gl.inv(wlen)
        half = length >> 1
        for start in range(0, n, length):
            w = 1
            for j in range(start, start + half):
                u = a[j]
                v = gl.mul(a[j + half], w)
                a[j] = gl.add(u, v)
                a[j + half] = gl.sub(u, v)
                w = gl.mul(w, wlen)
        length <<= 1
    if inverse:
        n_inv = gl.inv(n % gl.P)
        for i in range(n):
            a[i] = gl.mul(a[i], n_inv)


def forward(a: List[int]) -> List[int]:
    out = [x % gl.P for x in a]
    _transform(out, inverse=False)
    return out


def inverse(a: List[int]) -> List[int]:
    out = [x % gl.P for x in a]
    _transform(out, inverse=True)
    return out


def naive_dft(a: List[int]) -> List[int]:
    """Definition-order DFT, for tests only."""
    n = len(a)
    k = n.bit_length() - 1
    w = gl.root_of_unity(k)
    return [sum(gl.mul(a[i], gl.pow_mod(w, (i * j) % n)) for i in range(n)) % gl.P for j in range(n)]
