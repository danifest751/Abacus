# ABACUS-LAB-v1 — laboratory specification

Status: initial. Defines the objects and the verification problem studied in this repository.
No consensus, difficulty or network is specified here.

## 1. Field and objects

- Field: residues modulo Goldilocks `P = 2**64 - 2**32 + 1`, a prime. All arithmetic is modulo `P`.
- Matrix: row-major `n x n` array of residues.
- Vector: length-`n` array of residues.
- A **claim** is a tuple `(A, B, C)` where the prover asserts `C = A * B (mod P)`.

## 2. The verification problem

**Freivalds' algorithm.** Given `(A, B, C)` and a challenge vector `r`, the verifier accepts iff

```
A * (B * r) == C * r   (mod P)
```

Cost: `O(n^2)` field operations (two matrix-vector products plus comparison) versus `O(n^3)` for a
direct recomputation. Soundness: for uniformly random `r`, an incorrect `C` is accepted with
probability at most `1/P` (Schwartz–Zippel; `<= 2^-63` with our `LE64 mod P` derivation), so `k`
independent vectors give at most `2^-63k` (ADR 0009). The classical `1/2` bound applies to
`r in {0,1}^n` only.

## 3. Encoding and canonicality

- Residues are little-endian unsigned 64-bit integers in `[0, P)`.
- Only canonical encodings are accepted: no extra trailing bytes, no non-canonical integers,
  fixed `n` per profile, and dimension bounds checked before allocation.
- The challenge vectors `r_i` are derived by Fiat–Shamir from `(preheader, C)` after `C` is
  committed (ADR 0004, 0006; domain `abacus/check`), never from the header alone and never chosen by
  the prover, and must not be alterable by any field that yields new attempts.

## 4. Scope of this specification

This spec covers the field, the objects, the verification predicate and canonical encoding for the
matrix family. It deliberately says nothing about: how an instance is chosen for a block; how work
is priced; difficulty; chain selection; or usefulness. Those are open and are the subject of
`docs/CRITICAL-PATH.md`.

## 5. Known limitations

- Linear: `(A1 + A2) B = A1 B + A2 B`; the verifier certifies the result, not the work.
- A prover may decompose, precompute and reuse; the anchor that prevents this is not yet chosen.
- The field/primitive is illustrative; NTT (sumcheck-verified) and MSM (pairing-verified) are
  parallel candidate families with their own specifications.

## 6. Reference and independent implementations

- `reference/freivalds.py` — transparent Python reference.
- `crates/abacus-verifier` — independent Rust implementation with unit tests, a self-test binary
  and a stdin/stdout differential adapter.
- The two implementations are compared by `tests/test_differential.py` on a deterministic corpus.
