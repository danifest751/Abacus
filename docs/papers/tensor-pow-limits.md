# Proofs next to tensor cores: what an int8-matmul proof of work can and cannot verify

*Abacus laboratory note, 2026-10-09. Draft for external review. All numbers are measured on one GPU
(NVIDIA CMP 50HX, Turing sm_75) unless stated; raw data and sources are referenced per claim.*

## Abstract

Dense int8 matrix multiplication is the workload GPUs and AI accelerators are built for, which makes it
an attractive proof-of-work primitive; one such chain (Pearl) is live and another (Nockchain) has
announced a matrix-multiplication puzzle. We measure what happens when a tensor-core product has to be
*verified*. Exact int8 products are verifiable over a 64-bit prime field with per-challenge error
`<= 2^-63`. But a tensor core performs ~440x more multiply-adds per second than the same GPU performs
64-bit field operations, so any field-side step per attempt — committing to the product, encoding it,
proving it — costs as much as the product unless matrices are very large: a per-attempt polynomial
commitment costs 19–57x the GEMM with our kernels and still 1.4x at an ideal lower bound for
`n = 4096`. Proving only the winning attempt is possible but costs minutes of STARK proving per block on
CPUs. Linear reductions of gathered memory are broken by prefix sums. In an **interactive** setting the
same machinery works well: a commit-then-sample protocol certifies 40–49 TMAC/s of exact int8 work with
a 32-byte commitment and 7–26 ms of online verification, and catches provers that skip work at the
predicted rate.

## 1. Setting

An attempt computes `C = A * B` for `n x n` int8 matrices derived from the block header (or, in useful
variants, supplied by a customer) and enters a lottery on a hash of the result. A verifier must be
convinced that the published result is the product without recomputing it.

## 2. Exact int8 products verify cheaply over a 64-bit field

For `n <= 2^16`, `|(A B)_ij| <= 2^30`, so any int32 claim differs from the product by an integer matrix
with entries `< 2^32`, which is below the Goldilocks prime `P = 2^64 - 2^32 + 1`. Mapping to `F_P` keeps
the error nonzero, and a Freivalds challenge `r` uniform over `F_P^n` misses it with probability
`<= 1/P` (`<= 2^-63` for `LE64 mod P` challenges). Two Fiat–Shamir challenges bound to the header and
the product give `~2^-126`; the often-quoted `1/2` per challenge holds only for `r in {0,1}^n` and leads
to needlessly large `k` (Abacus ADR 0009). Measured cost: the Freivalds part is 2–5% of a naive CPU
product at `n = 256..512`; expanding the instance from the header with SHA-256 dominates a full block
check (19 ms at `n = 256`).

## 3. Tensor cores make field-side steps expensive

| quantity (CMP 50HX, warm, median of 7) | rate |
|---|---:|
| int8 GEMM, cuBLAS, tensor cores | 68 TMAC/s (`n = 4096`), 78 TMAC/s (`n = 8192`) |
| int8 GEMM, CUDA cores | 0.61 TMAC/s |
| Goldilocks multiply-add, tiled kernel | 0.175 TMAC/s |
| SHA-256 over the product | ~190 GB/s |
| radix-2 NTT, BabyBear, fused kernel | 1.0–4.0 Gelem/s (`2^27`–`2^20`) |

The product has `4 n^2` bytes while the work is `n^3`: hashing it costs 5.4x the GEMM at `n = 256`, 0.67x
at `n = 2048`, 0.20x at `n = 8192`. Shipping it is out of the question where the GEMM dominates
(16–256 MiB).

## 4. Non-interactive: per-attempt proofs cost more than the work

A lottery score must bind all of `C`. A linear score (`C r` for a header-known `r`) is computable as
`A (B r)` in `O(n^2)` without the product; a score on a sampled subset is ground by varying unsampled
entries for the price of a hash. A sound score therefore commits to `C` with the same commitment the
proof uses, on every attempt.

| n | GEMM | commitment, our NTT + SHA-256 | commitment, ideal lower bound | tensor share of the attempt (ideal) | FRI-style proof |
|---:|---:|---:|---:|---:|---:|
| 4096 | 1.02 ms | 25.5 ms (25x) | 1.44 ms (1.4x) | 42% | ~0.95 MiB |
| 8192 | 7.08 ms | 133 ms (19x) | 5.75 ms (0.81x) | 55% | ~1.1 MiB |
| 16384 | 56 ms (extrapolated) | — | 46 ms (0.82x, rate 1/4) | 55% | ~0.7 MiB |

(BabyBear, rate 1/2 unless noted; the ideal bound counts three DRAM passes and one hash of the codeword
and ignores FRI folding, so real costs are higher.) The commitment reaches the GEMM's cost only around
`n = 8192–16384`, with gigabytes of GPU memory, and even then about half of each attempt is NTT and
hashing — work for which the best hardware is not an AI accelerator. A tensor-core PoW with
per-attempt succinct proofs therefore does not keep its "AI hardware is the best miner" property.

**Prove only the winner.** Hashing `C` per attempt (0.2–0.36x the GEMM) and proving, for the winning
attempt only, that the hashed `C` equals `A B` is sound — a wrong `C` cannot be proven — but the proof
covers the hash of `4 n^2` bytes. Plonky3 on 8 CPU threads proves 65,536 SHA-256 compressions in 34.6 s
(BLAKE3: 15.4 s), i.e. ~9 min (~4 min) for `n = 4096` before aggregation, and refuses larger traces at
100-bit security. Viable only with a much faster (GPU) prover and long block intervals; the proving
delay then acts as a propagation delay that favours large miners.

**What Pearl does instead.** Pearl turns each tile of the noised product into its own lottery ticket and
lets the verifier recompute one tile, so no commitment to all of `C` is needed per attempt
(Komargodski–Schen–Weinstein, ePrint 2025/685). That is the design point that survives the analysis
above; its open issue is usefulness, not verification.

## 5. Memory-hard variants: linear folds are free to bypass

Gathering operands from a large dataset makes an attempt bandwidth-bound only with large segments per
entry, and only if each segment is folded **nonlinearly**: a sum-folded segment is answered from a
per-epoch prefix table with two reads. Measured against a warp-cooperative honest miner, the
prefix-sum attacker is 2.2–53x faster and becomes matmul-bound; with a nonlinear fold the honest miner
reads at ~98% of the sequential DRAM rate. Small random reads (8–32 B) are bound by the access rate
(~3.1e9/s), so one-word gathers leave the attempt compute-bound.

## 6. Interactive verification works

Without a lottery there is nothing to grind. A verifier sends a fresh seed; the prover computes `m`
products, returns a Merkle root over their rows, and only then learns which `k` rows to open; the
verifier checks each opened row in `O(n)` against a secret Freivalds vector prepared offline.

| n | m | certified TMAC/s | GEMM-only TMAC/s | verify | traffic |
|---:|---:|---:|---:|---:|---:|
| 8192 | 4 | 40–42 | 76–80 | 12–14 ms | ~1 MiB |
| 16384 | 2 | 48–49 | 66–69 | 23–26 ms | ~2 MiB |

Provers that skip 1% / 5% / 10% of rows passed 56/80, 16/80 and 0/80 challenges (expected 58, 15.5,
2.7 from the sampling bound `(1-f)^k`, `k = 32`). The certified rate is a lower bound on exact int8 work
in the measured time; it does not identify the machine that did it.

## 7. Design rules

1. Verify exact integer products over a field larger than the error bound; size `k` with the `1/P`
   bound, not `1/2`.
2. Never let a lottery score be a linear function of the product, or depend on a sampled subset the
   miner can vary.
3. Budget every per-attempt field or hash step against tensor-core throughput (~400x a field multiply);
   it must be `o(n^3 / 400)` to stay negligible.
4. Fold gathered memory nonlinearly, and benchmark against a cooperative (tuned) honest miner.
5. Prefer interactive commit-then-sample checks where the application allows them.

## Limitations

One GPU model; naive or simple kernels for NTT and hashing (hence the ideal bounds); no energy
measurements; the E1 prover cost is CPU-only; no independent replication yet.

## Artefacts

Abacus repository: `docs/research/` (gpu-suite-v1, int8-matmul-v1, a8-proof-cost-v1, e1-hash-proof-v1,
attest-v1, prior-art-v1), `docs/decisions/` (ADR 0009–0014), `scripts/gpu_suite.sh`,
`crates/abacus-attest`, `cuda/`.
