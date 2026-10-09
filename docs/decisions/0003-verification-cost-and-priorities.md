# ADR 0003 — Verification cost and candidate prioritisation

Status: accepted research constraint; no consensus parameters selected.

## Context

The laboratory premise is "expensive search, cheap verification". That premise is not equally true
for the three candidates, and the difference decides where effort belongs. This decision records the
cost analysis and the resulting priority, and what the decisive threats are for each.

## Analysis

Let `n` be the instance size.

- **A. Freivalds matmul.** Work: computing `C = A*B` costs `O(n^omega)` field operations, where
  `omega` is the matrix-multiplication exponent (Strassen `~2.81` practical; best known `~2.37`).
  Verification: Freivalds is `O(n^2)`. The **verification advantage is `n^(omega-2)`** — genuinely
  cheap. This is the strongest "cheap verification" candidate.
- **B. Sumcheck / NTT.** Work: a size-`n` NTT costs `O(n log n)`. Verification with sumcheck costs
  `O(n)` field elements plus one evaluation. The advantage over simply recomputing the NTT is only a
  **`log n` factor**. Unless the input is committed/private or far larger than the verifier can hold,
  the "cheap verification" premise is **weak** for B.
- **C. MSM / KZG.** Work: an MSM costs `O(n)` group operations; verification is a constant number of
  pairings — a real advantage. But ECDLP/pairings are **not post-quantum** (see
  `docs/THREAT-MODEL.md` section 9).

## Screening is not the main threat here

Unlike lattice/SIS instances (whose hardness varies per instance, so a miner screens), header-derived
matmul/NTT instances are **uniform**: random `A, B` are full-rank and dense with overwhelming
probability, so "screening the header/nonce" yields no cheaper instance. The decisive threats for a
linear candidate are therefore:

1. **Algorithmic speedup**: a miner using Strassen/better `omega` does the same result with
   asymptotically less work; the work model must use the cost of the **best known algorithm**, not
   `n^3`.
2. **Decomposition / precomputation**: `(A1+A2)B = A1B + A2B`, low-rank structure, cached plans.
   The instance must be verified dense/full-rank, and any structure a miner can influence must be
   excluded or anchored.
3. **Reuse across attempts** (score construction): partial products or twiddle plans reused across
   attempts of the *same* instance; header binding must change the whole instance per attempt.

## Decision

- **Prioritise candidate A (matmul with Freivalds verification)**. It has the clearest cheap-
  verification advantage and the least cryptographic baggage.
- **Treat B (NTT/sumcheck) as a secondary study**: it is useful for demonstrating sumcheck/GKR and for
  committed-input settings, but its verification advantage over recomputation is only `log n`, so it
  is not the primary path to a work function.
- **C stays a reference point only** (not PQ; pairings are a separate trust surface).
- The work model for A must be expressed against the **best known multiplication cost**, and the
  nonlinear anchor must bound decomposition/precompute (the linearity falsifier), not screening.

## Consequences

- The decisive CPU experiments are: instance-structure collapse (decomposition), algorithmic-speedup
  accounting (`omega`), and reuse. Screening is documented but not the blocker.
- D5 (matched CPU/GPU) is needed only after a work model exists; GPU is not required for the analysis
  above.
