# ADR 0012 — Candidate A8: int8 tensor-core matmul with exact verification

Status: accepted research direction (2026-10-09). Adds candidate A8 (spec/05) as the primary line of
work, following direction 1 of `docs/ASSESSMENT.md`. Candidate A stays the reference construction.
Amended by ADR 0013: test 1 stops the single-GEMM form with a per-attempt commitment; escapes E1/E2
remain open.

## Context

The assessment found that candidate A (Goldilocks matmul) is sound but its linear algebra adds cost,
not a distinguishing property. One concrete weakness: 64-bit modular arithmetic runs on the GPU's
ordinary integer units (~0.18 TMAC/s on the CMP 50HX) and does not use the tensor cores, which are the
part of a GPU — and of every AI accelerator — that dense matmul is built for.

## Decision

Study **A8**: the instance `A, B` is `n x n` **int8** from the header, the work is the **exact** product
`C = A * B` in **int32** (tensor-core IMMA), and verification is Fiat–Shamir Freivalds over Goldilocks
(an integer error is `< 2^32 < P`, so it never vanishes mod `P`; ADR 0009's `2^-63` per challenge holds).
The hypothesis to test is: *the best hardware for A8 is general-purpose AI hardware, so the PoW does not
create a market for single-purpose ASICs.*

## First measurement (CMP 50HX, `docs/research/int8-matmul-v1.md`)

- cuBLAS int8 GEMM: **68 TMAC/s at n = 4096, 78 TMAC/s at n = 8192** (~155 TOPS), exact (CPU-checked
  for n <= 512, GPU result Freivalds-checked for all n; a single corrupted entry is caught).
- That is ~110–128x the same int8 product on CUDA cores and ~440x the Goldilocks kernel.
- **Hashing `C` competes with the matmul.** `C` is `4 n^2` bytes; SHA-256 over it on the GPU runs at
  ~190 GB/s, which is 5.4x the GEMM time at `n = 256`, 0.67x at `n = 2048`, 0.36x at `n = 4096` and 0.20x
  at `n = 8192`. The matmul dominates only for `n >= ~2048`.
- **Block size is then prohibitive**: `C` is 16 MiB (`n = 2048`) to 256 MiB (`n = 8192`) per block.

## Consequences and falsifiers

1. **A8 cannot ship `C`.** It needs a succinct argument that the committed `C` equals `A * B`: commit to
   `C` (the score is then `H(preheader || commitment)`), prove the product with the matmul sumcheck
   (`C~(x, y) = sum_k A~(x, k) B~(k, y)`) and open the commitment at one random point. This merges the
   Freivalds (A) and sumcheck (B) lines; prover overhead and proof size must be measured.
2. **Sampled verification does not work.** Checking a few random rows of a committed `C` is broken by
   grinding: a miner computes almost all of `C` once and then varies garbage in one unchecked row to
   draw new scores at the cost of a hash, not of a product.
3. **No linear score.** A score from `C r` for a header-known `r` is computable as `A (B r)` in `O(n^2)`
   without the product; the score must be a collision-resistant commitment to all of `C`.
4. **Hardware hypothesis.** Requires a matched comparison (D5): tuned multi-threaded CPU int8 GEMM,
   energy per MAC, and published int8 rates of AI accelerators; the claim fails if a cheap
   single-purpose design beats AI hardware by a large factor.
5. **Structure.** Random int8 matrices are full rank with overwhelming probability, but small entries
   make products tabulable; any shortcut that avoids `n^3` multiply-adds for random int8 inputs falsifies
   the work model.

## Stop criteria

Stop A8 and record the result if (a) no succinct argument keeps prover overhead below the GEMM cost at
a block size under ~1 MiB, or (b) the matched comparison shows a single-purpose design with a large
advantage over AI hardware.
