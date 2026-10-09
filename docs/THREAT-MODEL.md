# Threat model (initial)

Scope: the research laboratory. No consensus or monetary system exists yet; this lists the
threats the work model must answer before one can.

## 1. Linearity and decomposition (primary)

Matmul, NTT and MSM are **linear** in their inputs. A malicious miner may:
- split the computation and combine partial results (`(A1 + A2) B = A1 B + A2 B`);
- precompute and cache products, bases or transform plans and reuse them across attempts;
- exploit symmetries (scaling, permutation, zero rows/columns, sparse structure).

Consequence: the "work" is not the claimed operation. Any construction must bound this, e.g. with
a nonlinear anchor (a hash of partial results folded back into the computation, a per-block fresh
randomness that must be consumed before the heavy work, a low-rank noise term, or a sequential
dependency). Characterising and minimising this anchor is the central research problem.

## 2. Verification soundness

- Freivalds accepts an incorrect product with probability at most `1/2` per random vector; the
  number of challenge vectors and their independence must be specified, and challenges must not
  be miner-chosen.
- Sumcheck/GKR soundness depends on Fiat–Shamir transcript binding; random-oracle assumptions and
  transcript encoding must be fixed and domain-separated.
- A verifier checks the *result*, not that a miner performed the work. A cheater who found a
  cheaper route is indistinguishable — this is a work-accounting problem, not a verification bug.

## 3. Instance selection (screening)

If any part of the instance (matrix, NTT domain, MSM scalars/points) is derivable from miner-
chosen header fields, the miner can screen for easy instances. Header binding must publish all
fields that change the challenge and forbid free attempt fields.

## 4. Reuse and transfer

- Replaying the same proof must not create additional work.
- Work must not transfer across parents/chains; any change to payout, transactions, timestamp,
  difficulty or version must change the instance.
- `cost(first) / cost(subsequent)` (bases, plans, cached products) must be measured and bounded.

## 5. Asymmetry and outsourcing

- Memory-rich or many-core devices must not gain a superlinear advantage that breaks monotonicity
  of the work function.
- The heavy computation may be outsourced; the protocol must not reward claimed-but-unverified
  work (no self-reported time, device counters or output size).

## 6. Resource attacks on nodes

- Invalid or near-valid proofs must not be able to overload full nodes: proof size, parsing and
  worst-case acceptance/rejection cost must be bounded before variable-length parsing.
- Snapshot/submission message caps and rate limits are required before any networking.

## 7. Cryptographic dependence (if the proving instantiation is pursued)

Coupling the PoW to a real proving market (NTT/MSM for ZK proof systems) introduces external
demand, key/setup trust and upgrade questions. Treat usefulness as an economic opportunity, not a
security property.

## 8. Out of scope (for now)

ASIC/FPGA resistance, post-quantum security, mainnet difficulty and coin value are **not**
established and must not be claimed.
