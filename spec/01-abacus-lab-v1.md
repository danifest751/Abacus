# ABACUS-LAB-v1 — laboratory specification

Status: current (revised 2026-10-09). Defines the objects and the verification problem studied in
this repository. Consensus rules of the prototype are in spec/03; nothing here is a protocol.

## 1. Field and objects

- Field: residues modulo Goldilocks `P = 2**64 - 2**32 + 1`, a prime. All arithmetic is modulo `P`.
- Matrix: row-major `n x n` array of residues. Vector: length-`n` array of residues.
- A **claim** is a tuple `(A, B, C)` where the prover asserts `C = A * B (mod P)`.

## 2. The verification problem

**Freivalds' check.** Given `(A, B, C)` and a challenge vector `r`, the verifier accepts iff

```
A * (B * r) == C * r   (mod P)
```

Cost: two matrix-vector products and one comparison, `O(n^2)`, versus `O(n^omega)` to recompute.

**Soundness.** If `C != A*B`, some row `e` of `E = A*B - C` is nonzero, and the check passes only if
`e . r = 0`. For `r` uniform over `F_P^n` this has probability `1/P` (a nonzero linear form vanishing
at a random point). Challenges derived as `LE64(hash) mod P` are slightly non-uniform (each coordinate
takes a value with probability at most `2^-63`), so one challenge errs with probability at most
`2^-63`, `k` independent challenges with at most `2^-63k`, and a forger who tries `Q` commitments
succeeds with probability at most `Q * 2^-63k` (ADR 0009). The often-quoted `1/2` applies only to
`r in {0,1}^n`.

**Binding.** A challenge known before `C` is chosen is forgeable: the prover picks `C' = A*B + M` with
`M r = 0` (ADR 0004). Challenges must therefore be derived by Fiat–Shamir from the committed `C` and
everything that defines the instance (spec/03 §3).

## 3. Encoding and canonicality

- Residues are little-endian unsigned 64-bit integers in `[0, P)`; an encoding `>= P` is rejected
  (otherwise one product has two encodings).
- Fixed `n` per profile; lengths are checked before allocation or hashing; no trailing bytes.
- Verifiers return "reject" on malformed input; they never abort.

## 4. Scope

The field, the objects, the verification predicate and canonical encoding of the matrix family. How
an instance is bound to a block, difficulty and fork choice are in spec/03; the memory-hard variant in
spec/04; the NTT/sumcheck family in spec/02.

## 5. Known limitations

- Linear: `(A1 + A2) B = A1 B + A2 B`. The verifier certifies the result, not that the prover did the
  work; work accounting is a separate question (`docs/CRITICAL-PATH.md`).
- The best multiplication algorithm is `O(n^omega)`, `omega < 3`; work must be priced accordingly.

## 6. Implementations

- `reference/freivalds.py`, `reference/chain.py` — Python reference.
- `crates/abacus-verifier` — independent Rust implementation (unit tests, self-test, adapters).
- Cross-checked by `tests/test_differential.py`, `tests/test_fs_parity.py`, `tests/test_chain_parity.py`.

## Revision history

- 2026-10-09: soundness bound `1/2` -> `<= 2^-63` per challenge (ADR 0009); challenges bound to
  `(preheader, C)` (ADR 0004, 0006, 0010); non-canonical residues rejected (ADR 0010).
