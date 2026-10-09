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

- Freivalds accepts an incorrect product with probability at most `1/P` per vector uniform over
  `F_P` (`<= 2^-63` with our derivation; `1/2` only for `r in {0,1}^n`, ADR 0009); challenges must be
  Fiat–Shamir bound to `(preheader, C)` and never miner-chosen.
- Sumcheck/GKR soundness depends on Fiat–Shamir transcript binding; random-oracle assumptions and
  transcript encoding must be fixed and domain-separated. A verifier that accepts prover-supplied
  challenges is forgeable (found and fixed, ADR 0010).
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
- Snapshot/submission message caps and rate limits are required before any networking. The
  prototype caps line length, connections and snapshot size and uses read timeouts (ADR 0010); it
  has no rate limiting or peer scoring.

## 6a. Difficulty and time (prototype consensus)

- The difficulty of every block must be the ancestor-derived value and must be committed in the
  header; work is counted from the required target, never from the achieved score (ADR 0010).
- Timestamps feed the retarget, so they must be bounded: greater than the median of the last 11
  blocks, and not far in the future of the validating node. A miner-chosen timestamp is otherwise
  a difficulty knob.

## 7. Cryptographic dependence (if the proving instantiation is pursued)

Coupling the PoW to a real proving market (NTT/MSM for ZK proof systems) introduces external
demand, key/setup trust and upgrade questions. Treat usefulness as an economic opportunity, not a
security property.

## 8. Out of scope (for now)

ASIC/FPGA resistance, post-quantum security, mainnet difficulty and coin value are **not**
established and must not be claimed.

## 9. Quantum posture (precise, not marketing)

"Quantum resistance" is fashionable and mostly premature: no quantum computer breaks deployed
cryptography today, and most projects bolt on ML-DSA for branding. Abacus states what is true per
candidate instead of branding:

- **Matmul (A) and NTT/sumcheck (B)** carry **no classical hardness assumption**. Their soundness is
  hash-based (Freivalds over a random challenge; Fiat–Shamir transcript). They are therefore
  **quantum-neutral**: a quantum computer does not speed up the verifier's task, and the security
  reduces to the hash. Caveat: Grover halves the effective security of hash preimages/collisions, so
  a hash-target rule (construction B) must size the digest for the intended PQ margin.
- **MSM / KZG (C)** is **not** post-quantum: its security rests on ECDLP and pairings, broken by
  Shor. Any deployment of C that claims PQ signatures/keys must be treated as a separate, unproven
  statement.
- A **signature/key layer** is outside the PoW primitive; if Abacus ever needs one, PQ primitives
  (e.g. ML-DSA / SLH-DSA) are the baseline, not a feature of the work function.

Do not label the project "quantum-resistant". State the per-candidate posture above.

## 10. ASIC posture (candidate A is ASIC-friendly; A' adds memory-hardness)

Dense matmul is **GPU-optimal and ASIC-optimal**: a tensor-core/systolic ASIC does matmul better than
any GPU. So "GPU-optimal" does **not** imply ASIC resistance; the property that makes candidate A
attractive for GPUs is the same one that makes it attractive for ASICs.

Mitigation (ADR 0007): **candidate A'** adds a memory-hard, data-dependent layer — a large epoch
dataset `D` with header-random **gather** of the operands — so the bottleneck is **memory bandwidth**,
not multiply throughput, and a fixed ASIC is obsoleted by per-epoch dataset regeneration. Threat
questions this must answer: is the gathered access bandwidth-bound and GPU-favourable; is the work
model monotone in bytes fetched; can gathered operands be precomputed/reused; what is the verifier's
added dataset cost; is epoch regeneration cheap enough yet ASIC-hostile.

Honest caveat: memory-hard algorithms eventually get ASICs (Ethash, scrypt); agility narrows the
window, it does not close it. The goal is a high ASIC barrier, not immunity.

