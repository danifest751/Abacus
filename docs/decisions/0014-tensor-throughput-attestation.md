# ADR 0014 — Second track: interactive proof of tensor throughput; analysis write-up

Status: accepted (2026-10-09). Extends the scope of the laboratory beyond the PoW question of
`docs/CRITICAL-PATH.md` §1, by decision of the project owner.

## Context

The PoW line ended with: candidate A sound but adding nothing a hash PoW lacks (ASSESSMENT); A8's
per-attempt proof costs more than the tensor work (ADR 0013); its escape E1 needs minutes of proving
(`e1-hash-proof-v1`); and the int8-GEMM PoW design point is already deployed by Pearl
(`prior-art-v1`). What failed in A8 was **grinding**: a non-interactive lottery lets the miner vary
uncommitted data, so cheap sampled checks are unsound and a full succinct commitment is required on
every attempt. In an interactive setting the verifier picks the samples after the commitment, and that
failure mode disappears.

## Decision

1. Build an **interactive proof of tensor throughput** (spec/06, `crates/abacus-attest`,
   `cuda/attest_prover.cu`): commit-then-sample over the rows of fresh int8 products, verified with a
   secret Freivalds vector in `O(k n)` online. Use: compute marketplaces and customers that need a
   cryptographic lower bound on a provider's tensor throughput.
2. Write up the PoW results as an **analysis note** (`docs/papers/tensor-pow-limits.md`): what a
   tensor-core PoW can and cannot do with proofs, measured.

## Results so far (`docs/research/attest-v1.md`)

- GPU prover byte-compatible with the Rust verifier; accepted on loopback and over the LAN.
- Proven throughput 40–49 TMAC/s for `n >= 8192` on a CMP 50HX (the GEMM alone runs at 74–80 TMAC/s;
  the rest of the response time is SHA-256 expansion and row hashing).
- Cheating provers that skip 1% / 5% / 10% of rows passed 56/80, 16/80, 0/80 challenges against an
  expected 58, 15.5, 2.7.

## Consequences

- Prior art to position against: matmul + Freivalds utilisation puzzles for AI-governance telemetry
  (arXiv 2602.09369) and TEE-based GPU attestation. Novelty claims must be limited to the measured
  design (no transfer of `C`, exact int8 on tensor cores, sampled rows + secret Freivalds, measured
  detection), pending a closer literature check.
- The PoW chain prototype stays frozen; CPPminer stays parked.
