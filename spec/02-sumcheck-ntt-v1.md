# ABACUS-LAB-v1 — sumcheck / NTT specification

Status: initial. Defines the Goldilocks field, the NTT object and the sumcheck verification problem
studied by candidate B. No consensus, difficulty or network is specified here.

## 1. Field

Goldilocks: residues modulo `P = 2**64 - 2**32 + 1`, a prime with 2-adicity 32. A generator is `7`;
`7 ** ((P - 1) >> k)` is a primitive `2**k`-th root of unity for `k <= 32`. Arithmetic uses 128-bit
intermediates; products are reduced modulo `P`.

## 2. The NTT object

`NTT_n(a)_j = sum_i a_i * w**(i*j)`, where `n = 2**k` and `w` is a primitive `n`-th root. The
inverse is `n**-1 * NTT_n(a)` with `w` replaced by `w**-1`. The transform is a linear map; this is
the GPU-optimal primitive (and the linear layer of STARK/FRI provers) being studied.

## 3. The sumcheck verification problem

A multilinear polynomial `f(x_1..x_n)` is given by its Boolean-hypercube evaluations,
`table[i] = f(bits of i)`, bit `j` of `i` = `x_{j+1}` (LSB = variable 1). The protocol proves

```
S = sum_{b in {0,1}^n} f(b)
```

with `O(n)` field elements:

- round `j` (variable `j+1`): the prover sends the degree-1 round polynomial `g_j`, given here by
  `(g_j(0), g_j(1))`; the verifier checks `g_j(0) + g_j(1) = cur` and sets `cur = g_j(r_j)` for a
  random `r_j`;
- after `n` rounds the verifier recomputes `f(r_1..r_n)` from the table via the multilinear
  extension and checks it equals `cur`.

Completeness holds for an honest prover. Soundness: for an incorrect claim, the probability of
acceptance is at most `n / P` per independent challenge (negligible for `P = 2**64` analog and for
the multi-round protocol), and any single inconsistent round value is rejected deterministically by
the `g_j(0) + g_j(1) = cur` check.

## 4. Encoding and canonicality

- Field elements are little-endian unsigned 64-bit integers in `[0, P)`.
- `n` is a power of two, `k = log2(n) <= 32`; the length is checked before any allocation.
- Challenge values `r_j` are derived from the header/transcript (domain-separated) and are never
  chosen by the prover.

## 5. Scope of this specification

This spec covers the field, the NTT object, the sumcheck predicate and canonical encoding. It says
nothing about how an instance is chosen for a block, how work is priced, difficulty, chain
selection or usefulness — those are open and live in `docs/CRITICAL-PATH.md`.

## 6. Known limitations

- NTT is linear: block/butterfly decomposition, precomputed twiddle plans and transcript reuse must
  be bounded by a nonlinear anchor; this is not yet chosen.
- Sumcheck verifies a claimed computation cheaply; it does not certify that the prover performed the
  work (the same work-accounting gap as Freivalds).
- The Fiat-Shamir transform (turning the interaction non-interactive) is not specified here and must
  bind every field that changes the instance.

## 7. Reference and independent implementations

- `reference/goldilocks.py`, `reference/ntt.py`, `reference/sumcheck.py` — Python reference.
- `crates/abacus-verifier/src/{goldilocks,ntt,sumcheck}.rs` — independent Rust implementation with
  unit tests and a self-test that exercises Freivalds + NTT + sumcheck.
