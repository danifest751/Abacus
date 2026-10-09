# ADR 0002 — Candidate work functions and their falsifiers

Status: accepted research constraint; no consensus parameters selected.

## Context

`docs/CRITICAL-PATH.md` fixes one question: can a permissionless PoW be built from a **linear,
GPU-optimal** computation with cheap probabilistic verification, resisting decomposition,
precomputation and reuse? The laboratory now has working verifiers for the two basic tools:
Freivalds (matmul) and sumcheck/NTT (Goldilocks). This decision writes down the candidate work
functions, how their weight is defined, and what would kill each one.

## Decision

Define weight **by component** and require monotonicity and non-transferability for every candidate.

- **A. Freivalds matmul.** Instance: header -> `A, B`; work: compute `C = A*B`; proof: `(C, r)` with
  a header-derived challenge `r`; verifier: Freivalds, `O(n^2)`.
  - Falsifiers: linear decomposition and precompute (`(A1+A2)B = A1B + A2B`); plan/basis reuse;
    header/nonce screening for easy instances; whole-result outsourcing; miner-chosen `r`.
- **B. Sumcheck / GKR NTT over Goldilocks.** Instance: header -> a random field element / evaluation
  domain; work: compute an NTT and a sumcheck transcript; verifier: sumcheck, `O(n)` field elements
  and one evaluation, no `O(n log n)` recomputation.
  - Falsifiers: NTT linearity (block/butterfly reuse, precomputed twiddle plans); transcript reuse;
    domain selection; sublinear *verification* does not imply sublinear *work*; challenge derivation
    must be bound to the header (Fiat-Shamir) and not miner-choosable.
- **C. MSM with KZG.** Instance: header -> scalars/points; work: an MSM + commitment; verifier:
  pairing checks.
  - Falsifiers: MSM linearity in scalars/points; batched/precomputed bases; pairing scope (cheap
    verification vs which work it certifies).
  - Note: ECDLP/pairings are **not** post-quantum (see `docs/THREAT-MODEL.md` section 9).

Weight components to be defined and measured for any candidate: instance generation; the heavy
computation; the nonlinear anchor; proof construction; failed attempts. Do **not** use wall-clock
seconds, device-reported work, output size or vector length. A candidate is rejected at the gate if
any falsifier fires at the smallest profile where it can.

## Consequences

- The linearity/precompute anchor is the common blocker for A and B; it must be designed before a
  difficulty/fork simulator is built.
- The verifier additions (Goldilocks, NTT, sumcheck) are the measurement tools for B; they make no
  claim about the *existence* of a work function.
- Screening, precomputation/amortisation and matched CPU/GPU scope (D3-D5) are the next experiments.
