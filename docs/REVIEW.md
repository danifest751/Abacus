# Review guide — Abacus (experimental Verifiable Algebra PoW)

This is a **research laboratory**, not a protocol, coin or security claim. The goal of a review is to
find where the research is wrong, incomplete or overclaimed. Everything here is falsification-first;
a negative result is a valid outcome.

## Changes since the first review (2026-10-09)

The first review of `bb6e532` is answered in ADR 0009 (the per-challenge Freivalds error is
`<= 2^-63`, not `1/2`; `k = 2..3` suffices) and ADR 0010 (difficulty enforced and committed,
timestamp rules, malformed-input panics, data-dependent A' dataset, the withdrawn A' attempt-rate
result, sumcheck Fiat–Shamir, full Python/Rust parity). Questions 1 and 5 below are partly answered
there; please re-check them against the new code.

## What to review (in order)

1. `01-ABACUS-RESEARCH-PLAN.md` — the objective, candidates A/B/C, test plan, gates.
2. `docs/CRITICAL-PATH.md` — the single open question and the decisive experiments.
3. `docs/THREAT-MODEL.md` — linearity, precomputation, reuse, resources, quantum and ASIC posture.
4. `spec/01`-`spec/04` — the lab, sumcheck/NTT, candidate A (header-bound Freivalds matmul), candidate
   A' (memory-hard).
5. `docs/decisions/0001`-`0010` — the decisions, including two bugs we found ourselves (ADR 0004,
   0006), two corrections (ADR 0008 plus the warm-bandwidth note) and the review fixes (ADR 0009,
   0010).
6. Code: `reference/` (Python) and `crates/abacus-verifier` + `crates/abacus-chain` (Rust); the
   differential/parity tests are the correctness evidence.
7. Empirical: `docs/research/*` (GPU baselines, verifier throughput, memory-hard balance/attempt rate,
   local prototype, multi-node testnet, CPPminer backend).

## The central claim under test (candidate A)

> A permissionless PoW can be built from a dense matmul `C = A*B` over Goldilocks, where `A, B` derive
> from the header (so no screening is possible), the score is `SHA256(domain || preheader || C)` against
> a target, and verification uses `k` **Fiat-Shamir-bound Freivalds** challenges.

Attack these specifically:

- **Forgery / soundness.** Is the `k`-challenge Fiat-Shamir binding (ADR 0004) actually sound? Can a
  wrong `C` pass with non-negligible probability, or be ground without the matmul? ADR 0009 states
  `<= Q * 2^-63k` for a `Q`-query forger; check it and the revised `n > 2k` constraint.
- **Work accounting.** The work is `n^omega`, not `n^3`. Is the difficulty monotone in the best-known
  multiplication cost, and is a uniform algorithmic constant absorbed by retarget (ADR 0005)?
- **Linearity.** With header-derived `A, B`, is screening/decomposition/precompute really excluded?
  Look for any field that yields new attempts, or any way to obtain `C` without the `n^omega` product.
- **Verifier cost / DoS.** Every node verifies every block at `O(k n^2)`; is the worst-case cost
  bounded before variable-length parsing?
- **Post-quantum and ASIC posture.** A/B are hash-based (Grover caveat); C is not PQ. A is ASIC-friendly
  and A' only raises the barrier (ADR 0007/0008). Are these stated precisely, without overclaiming?

## What we do *not* claim

- No currency, network, production miner or consensus.
- No "usefulness" (a random matmul is not a consumer computation).
- No ASIC resistance, no post-quantum security, no mainnet parameters.
- The GPU advantage is a **measured constant factor** (CMP 50HX), not a proof of work-model soundness.

## How to reproduce

```
python scripts/check.py      # Python + Rust tests + self-test + differential corpus
```

Experiments live in `scripts/` and `cuda/`; raw outputs go to an ignored `artifacts/`.

## Specific questions we want answered

1. Does the Fiat-Shamir Freivalds construction (ADR 0004/0006/0009) give `<= Q * 2^-63k` soundness
   under a miner who commits to `C` first, and is `k = 2..3` a sensible profile?
2. Is candidate A genuinely a *permissionless* puzzle (no free attempt fields, no cheaper route to a
   valid `C`), or does linearity still leave a gap we have not closed?
3. For A' (memory-hard gather), is the gathered instance sound, and does the dataset-before-instance
   ordering really bind (a miner must hold the dataset)?
4. Are the empirical claims (`docs/research/*`) reproducible from the given commands, and are the
   caveats honest?
5. What is the strongest attack you can mount on the work model of candidate A or A'?

## A note on prior art we already consulted

Pearl (matmul PoW), Tenero (matmulhash v2), and the closed RGminer/PeakMiner were studied separately
(`handoff-gpu-opt-20261007/`); the header-binding + no-noising distinction from Pearl is in
`spec/03`. Please challenge any place where we claim novelty — we prefer to be told we are wrong.