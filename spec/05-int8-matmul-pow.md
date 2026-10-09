# ABACUS-LAB-v1 — int8 tensor-core matmul PoW (candidate A8)

Status: candidate construction (2026-10-09, ADR 0012). Reference implementation of the instance, the
product and the verification exists (`crates/abacus-verifier/src/int8.rs`, `crates/abacus-chain`
`instance_i8` / `score_i8` / `verify_i8`, `reference/int8.py`); it is **not** wired into the chain, and
the score/proof format below is the open part.

## 1. Construction (current reference)

```
preheader = as spec/03 (encoding v2)
seed      = SHA256("abacus/instance-i8" || preheader)
bytes     = SHA256("abacus/expand" || seed || LE32(i)), i = 0, 1, ...     # 32 bytes per hash
A, B      = first n^2 and next n^2 bytes, read as two's-complement int8 (row-major)
C         = A * B exactly, int32                                          # tensor-core IMMA
score     = SHA256("abacus/score-i8" || preheader || C as LE int32)
root      = SHA256("abacus/check-i8" || preheader || C as LE int32)
r_i       = expand(root) over F_P, P = 2^64 - 2^32 + 1 (as spec/03)
accept    = n <= 2^16, |C| = n^2, leading_zero_bits(score) >= bits,
            all_i A*(B*r_i) == C*r_i  (mod P, entries mapped to residues)
```

**Soundness.** For `n <= 2^16`, `|(A*B)_ij| <= n * 128 * 128 <= 2^30` and any int32 claim has
`|c| <= 2^31`, so a wrong `C` differs by an integer matrix `E` with `0 < |e_ij| < 2^32 < P`; `E mod P` is
nonzero and the per-challenge error is `<= 2^-63` as in spec/01.

**Instance expansion** is 8x cheaper than candidate A (32 int8 entries per hash instead of 4 field
elements).

## 2. Measured properties (`docs/research/int8-matmul-v1.md`)

- GEMM on tensor cores: up to ~78 TMAC/s on a CMP 50HX, ~110–128x CUDA cores, ~440x Goldilocks.
- Hashing `C` (`4 n^2` bytes) costs 0.2x (`n = 8192`) to 5.4x (`n = 256`) of the GEMM.
- `C` per block: 16 MiB at `n = 2048`, 64 MiB at `n = 4096`.

## 3. Open: score and proof format

Shipping `C` is infeasible at the sizes where the matmul dominates. Required instead (ADR 0012):

- `score = H(preheader || commit(C))` with a binding commitment to all of `C`;
- a succinct proof that the committed `C` equals `A * B`: the matmul sumcheck
  `C~(x, y) = sum_k A~(x, k) B~(k, y)` at a Fiat–Shamir point, with `A~, B~` evaluated by the verifier from
  the seed (`O(n^2)`) and `C~` opened from the commitment;
- no linear score (computable from `A (B r)` without the product) and no sampled-row verification
  (grindable).

## 3a. Test 1 result (ADR 0013)

A per-attempt polynomial commitment to `C` costs 19–57x the GEMM with our kernels and, even at an ideal
lower bound, more than the GEMM for `n <= 4096`; the single-GEMM form above is stopped. Open escapes:
**E1** — score on a plain hash of `C`, winner-only STARK that the hashed `C` equals `A * B`; **E2** — a
deep chain of requantized int8 GEMMs with one output commitment and GKR (`a8-proof-cost-v1`).

## 4. Falsifiers

Listed in ADR 0012: prover overhead of the argument vs the GEMM; proof size; matched CPU/GPU and
AI-accelerator comparison; any sub-`n^3` route for random int8 inputs.
