# A8 proof cost v1 — can a succinct proof of `C = A * B` keep up with the int8 GEMM?

Date: 2026-10-09. Status: **current**. Test 1 of ADR 0012; decision in ADR 0013.

## Question

A8 cannot ship `C` (16–256 MiB per block where the GEMM dominates). A succinct proof needs a
polynomial commitment to `C`, and because the score must bind all of `C` and must not be grindable, the
commitment the score uses is built on **every attempt**. Is that commitment cheaper than the GEMM?

## Measurements (CMP 50HX, warm, median of 7)

`cuda/ntt_bench.cu` (sha256 `734b950d…`): radix-2 DIF NTT, naive (one launch per stage) and fused (last
10 stages in shared memory), Goldilocks and BabyBear (Montgomery); exact against a CPU DFT at
`L = 1024` for both fields. Raw: `artifacts/ntt-run1.txt`.

| log2 L | Goldilocks fused ms | BabyBear fused ms | BabyBear Melem/s |
|---:|---:|---:|---:|
| 20 | 0.604 | 0.262 | 4000 |
| 22 | 3.171 | 1.954 | 2147 |
| 24 | 16.398 | 10.523 | 1594 |
| 25 | 36.873 | 24.828 | 1352 |
| 26 | 82.479 | 56.922 | 1179 |
| 27 | 183.339 | 129.890 | 1033 |

Other inputs: cuBLAS int8 GEMM and SHA-256 over `C` from `int8-matmul-v1`; DRAM 538 GB/s and SHA-256
~190–195 GB/s from `gpu-suite-v1`.

## Model (`scripts/a8_proof_cost.py`, JSON in `artifacts/a8-proof-cost.json`)

Per attempt: `GEMM(n) + commit`, commit = Reed–Solomon encoding of the `N = n^2` entries at rate `rho`
plus a SHA-256 Merkle hash of the codeword. Two commit estimates: **measured** (our fused NTT + hashing)
and an **ideal lower bound** for any implementation (3 DRAM passes over the codeword at the sequential
rate + hashing it; FRI folding, transposes and twiddles ignored). Proof size: FRI-style, 100-bit,
32-byte digests, no path pruning. The matmul sumcheck itself (`O(n^2)`) is ignored, which favours A8.

| n | rho | GEMM ms | commit measured ms | commit ideal ms | overhead measured | overhead ideal | tensor share (ideal) | proof KiB |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2048 | 1/2 | 0.196 | 4.74 | 0.36 | 24x | 1.83x | 35% | 797 |
| 4096 | 1/2 | 1.023 | 25.5 | 1.44 | 25x | 1.40x | 42% | 956 |
| 4096 | 1/4 | 1.023 | 58.3 | 2.87 | 57x | 2.81x | 26% | 520 |
| 8192 | 1/2 | 7.080 | 132.7 | 5.75 | 19x | 0.81x | 55% | 1128 |
| 16384 | 1/4 | 56.4 (extrap.) | — | 46.0 | — | 0.82x | 55% | 705 |
| 32768 | 1/2 | 451 (extrap.) | — | 92.0 | — | 0.20x | 83% | 1509 |

(BabyBear rows; Goldilocks doubles the commit cost. BabyBear's two-adicity limits a single NTT to
`2^27`, so `n >= 16384` needs another field, e.g. a circle-FFT field.)

## Findings

1. **Single-GEMM A8 with a per-attempt commitment fails the ADR 0012 stop criterion.** Our kernels make
   the commitment 19–57x the GEMM. Even the ideal lower bound is above the GEMM for `n <= 4096` and only
   reaches ~0.8x at `n = 8192` with a proof just over 1 MiB, or at `n = 16384` (1 GiB `C`, >= 10 GiB of GPU
   memory, a non-BabyBear field).
2. **Where it does pass, the work is no longer mostly tensor-core work.** The tensor-core share of an
   attempt is 42% at `n = 4096` and 55% at `n = 8192` even in the ideal case; the rest is NTT and hashing,
   which favours a commitment ASIC — the opposite of the A8 hypothesis.
3. **Root cause:** tensor cores make the `n^3` product ~440x cheaper per operation than field arithmetic,
   so any `O(n^2 log n)` field-side step per attempt costs as much as the product until `n` is very large.

## Escapes not covered by this test (hypotheses, unmeasured)

- **E1 — prove the hash once.** Score = a plain hash of `C` (measured 0.36x the GEMM at `n = 4096`, 0.20x
  at `8192`: tensor share 73–84%); only the winner proves, with a STARK, that the hashed `C` equals `A * B`
  (the in-circuit check is Freivalds/sumcheck, `O(n^2)`, plus the hash of `4 n^2` bytes). Grinding gives
  nothing because a wrong `C` cannot be proven. Unknown: winner proving time for SHA-256 (or a faster
  hash) over 64–256 MiB, and the orphan risk it creates.
- **E2 — deep chain.** `L` int8 GEMMs with nonlinear requantization between layers (like int8 inference);
  only the final `n^2` output is committed per attempt and GKR proves the layers for the winner. The
  commitment cost is divided by `L` (e.g. ideal 1.40x / 16 ~ 0.09x at `n = 4096`). Unknown: GKR with
  lookup-based requantization, verifier cost for header-derived weights, proof size.

## Caveats

- One GPU; a tuned NTT (e.g. a production prover library) is several times faster than ours, which is
  why the ideal bound is also reported; no FRI folding, transpose or twiddle costs are included.
- The proof-size model omits path pruning and grinding, which can shrink proofs by roughly 30–50%.
