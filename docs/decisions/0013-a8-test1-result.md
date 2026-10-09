# ADR 0013 — A8 test 1: a per-attempt succinct commitment costs more than the int8 GEMM

Status: accepted research result (2026-10-09). Applies the stop criterion of ADR 0012 to the
single-GEMM form of A8; records two untested escapes. Evidence: `docs/research/a8-proof-cost-v1.md`.

## Result

- A succinct proof of `C = A * B` needs a commitment to `C`, and the score must use it, so it is built on
  every attempt. On the CMP 50HX our NTT-based commitment costs 19–57x the int8 GEMM; an ideal lower
  bound for any implementation is 1.4x at `n = 4096` and 0.8x at `n = 8192` (proof ~1.1 MiB), and drops
  well below the GEMM only for `n >= 16384`.
- In every regime where the bound passes, 45% or more of an attempt is NTT and hashing, not tensor-core
  work, contradicting the A8 hypothesis that the best mining hardware is AI hardware.

## Decision

1. **Single-GEMM A8 with a per-attempt polynomial commitment is stopped** under ADR 0012 criterion (a).
2. The int8 reference implementation stays as the exact-product and verification baseline.
3. Two escapes are recorded as hypotheses, not adopted:
   - **E1 (prove the hash once):** score on a plain hash of `C`; the winner proves with a STARK that the
     hashed `C` equals `A * B`. Per-attempt overhead measured at 0.2–0.36x the GEMM (`n = 8192..4096`).
     Decisive unknown: the winner's proving time for the hash of 64–256 MiB.
   - **E2 (deep chain):** `L` requantized int8 GEMMs per attempt, one commitment to the final output,
     GKR for the layers; overhead divided by `L`. Decisive unknowns: GKR with lookups, verifier cost,
     proof size.

## Consequences

The next A8 experiment, if pursued, is an estimate of the E1 winner proving cost from an existing
STARK/zkVM prover on the same GPU class (SHA-256 or a faster hash over tens of MiB, plus an `O(n^2)`
Freivalds check). If that cost exceeds a block interval, E1 is stopped too; E2 is a larger design
effort and comes after it.
