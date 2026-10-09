# ABACUS-LAB-v1 — sumcheck / NTT specification

Status: current (revised 2026-10-09). Defines the Goldilocks field, the NTT object and the
non-interactive sumcheck used by candidate B (a secondary study, ADR 0003). No consensus is specified.

## 1. Field

Goldilocks: residues modulo `P = 2**64 - 2**32 + 1`, 2-adicity 32. `7` generates the multiplicative
group; `7 ** ((P - 1) >> k)` is a primitive `2**k`-th root of unity for `k <= 32`.

## 2. The NTT object

`NTT_n(a)_j = sum_i a_i * w**(i*j)` for `n = 2**k` and a primitive `n`-th root `w`; the inverse uses
`w**-1` and scales by `n**-1`. The transform is a linear map (the linear layer of STARK/FRI provers).

## 3. Sumcheck

A multilinear `f(x_1..x_n)` is given by `table[i] = f(bits of i)`, bit `j` of `i` = `x_{j+1}`
(LSB = variable 1). The claim is `S = sum_{b in {0,1}^n} f(b)`.

- Round `j`: the prover sends `(g_j(0), g_j(1))` of the degree-1 round polynomial; the verifier checks
  `g_j(0) + g_j(1) = cur` and sets `cur = g_j(r_j)`.
- Final: the verifier checks `f(r_1..r_n) = cur`, computing `f(r)` from the table by the multilinear
  extension.

**Challenges are Fiat–Shamir, derived by the verifier:**

```
state_0 = SHA256("abacus/sumcheck" || LE64(n) || LE64(len) || table || LE64(S))
state_j = SHA256(state_{j-1} || LE64(g_j(0)) || LE64(g_j(1))),   r_j = LE64(state_j[0..8]) mod P
```

A verifier that accepts prover-supplied challenges is forgeable: with all `r_j = 0` any claimed sum
passes (found and fixed, ADR 0010).

**Soundness.** Each round polynomial has degree 1, so a false claim survives a round with
probability at most `1/P` per round (`<= 2^-63` with the derivation above), at most `n * 2^-63`
overall for a fixed transcript.

**Cost.** The rounds are `O(n)` field elements, but the final evaluation from the full table is
`O(2^n)`. This verifier is a correctness building block, **not** a succinct verifier: sublinear
verification needs `f(r)` from a commitment opening or an independently computable function, which is
not implemented.

## 4. Encoding and canonicality

Field elements are LE64 in `[0, P)`; `n` is checked before allocation; round values `>= P` and a
claimed sum `>= P` are rejected.

## 5. Known limitations

- NTT is linear: block/butterfly decomposition, precomputed twiddle plans and transcript reuse are
  not bounded by any anchor here.
- Sumcheck certifies the claimed sum, not that the prover did any particular work.
- For an NTT of size `n`, the work is `O(n log n)` and recomputation is only a `log n` factor more
  expensive than any linear-time check (ADR 0003): the cheap-verification premise is weak for B.

## 6. Implementations

`reference/{goldilocks,ntt,sumcheck}.py` and `crates/abacus-verifier/src/{goldilocks,ntt,sumcheck}.rs`;
transcript parity in `tests/test_chain_parity.py`.

## Revision history

- 2026-10-09: Fiat–Shamir challenges specified and implemented; "O(log n) verification" corrected to
  the actual `O(2^n)` table evaluation; soundness stated per round (ADR 0010).
