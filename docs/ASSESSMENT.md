# Assessment of the central question (2026-10-09)

This is the current written answer to `docs/CRITICAL-PATH.md` §1, as required by its success metric.
It states what the evidence supports, what it does not, and what would change the answer. It is a
research assessment, not a security claim.

> **Question.** Can a permissionless PoW be built from a linear, GPU-optimal computation (matmul,
> NTT, MSM) with cheap probabilistic verification, while resisting decomposition, precomputation and
> reuse?

## Short answer

**Yes at the construction level, but the linear algebra contributes no property that a hash-based PoW
lacks.** Candidate A is a sound, simple PoW. Its resistance to decomposition, precomputation and reuse
comes from deriving the whole instance from the header, not from anything specific to linear algebra,
and the features that would justify linear algebra (usefulness, a lasting GPU advantage, an ASIC
barrier) are not delivered. The idea has a right to exist as a **research result** and as a base for one
of the directions below; it does not yet justify a network.

## What the evidence supports

| Claim | Evidence |
|---|---|
| Candidate A is permissionless: no screening, no reuse across attempts | the instance is uniform and fully re-derived per preheader (spec/03 §3); random dense matrices are full rank (`instance_probe.py`) |
| Verification is sound and cheap | per-challenge error `<= 2^-63`; `k = 2` gives `~2^-126` (ADR 0009, `freivalds_soundness_probe.py`); a full block check takes 19 ms at `n = 256` on one CPU core (~1/5 of a naive CPU product, ~190 GPU products), two thirds of it instance expansion (`verifier-throughput-v2`) |
| The prototype is internally consistent | Rust and Python agree byte for byte on every consensus derivation (`test_chain_parity.py`); the CUDA miner's preheader, instance, score and A' dataset words are accepted by the Rust node with no invalid verified submission (`chain-prototype-v2`) |
| A GPU computes the work efficiently | ~175 GMAC/s of Goldilocks multiply-add on a CMP 50HX with a naive kernel (`gpu-suite-v1`) |
| A' can be made bandwidth-bound | with a nonlinear large-slice fold a tuned miner reads at about the measured sequential rate and gathering `A` alone takes 1.7–79x the matmul (ADR 0011) |

## What it does not support

1. **Linearity is avoided, not solved.** Header-derived `A, B` exclude decomposition because the miner
   never sees useful or reusable structure. The original problem — linear work on *externally given*
   data (useful work) — is untouched. That is the case where an anchor is needed (Pearl's noise), and
   it is where the project's question is genuinely hard.
2. **No usefulness.** The matrices are random. The work is a costly function inside a hash lottery,
   i.e. functionally "a slow hash". Security reduces to SHA-256 as a random oracle plus Freivalds
   soundness; the matmul adds cost, not security.
3. **No demonstrated GPU or ASIC property.** Dense 64-bit modular multiply-add is a regular, simple
   datapath; a dedicated MAC array would beat a GPU by a large constant factor, as for SHA-256. The
   measured 1,100x GPU/CPU ratio is against a naive single-threaded CPU loop; the matched comparison
   (D5: tuned multi-threaded CPU, energy x time) has not been run.
4. **A' moves the work away from linear algebra.** When the gather dominates, the PoW is a memory-
   bandwidth PoW with a matmul attached (Ethash-class). It must then be judged against Ethash/ProgPoW
   on dataset time–memory trade-offs, verifier memory, and ASICs with comparable memory — none of
   which is analysed. In the one-word design it is not memory-hard at all.
5. **Network costs.** Every block carries `C`: `8 n^2` bytes (512 KiB at `n = 256`, 8 MiB at
   `n = 1024`). A' additionally requires every node to hold the dataset and re-read hundreds of MB per
   block.
6. **Candidates B and C are weaker.** For an NTT, verification is only a `log n` factor cheaper than
   recomputation (ADR 0003) and the implemented sumcheck verifier is `O(2^n)`; MSM/KZG is not
   post-quantum and carries pairing trust. Neither was developed beyond verifiers.

## Prior art to compare against (not re-studied here)

- Pearl / matmul proofs of useful work: external matrices with low-rank noise — the useful-work variant.
- kHeavyHash (Kaspa): a header-derived matrix-vector product inside a hash; a small linear-algebra step
  in a hash PoW, later ASIC-mined.
- Ethash / ProgPoW: DAG-based bandwidth PoW — the comparison class for A'.
- Aleo's coinbase puzzle (proof-of-succinct-work): a deployed PoW whose work was built from
  zk-proving primitives. It must be checked before claiming that no live chain uses MSM/NTT-style
  work; its design history should be studied as the closest precedent for the "zk-prover as buyer"
  idea below.

## The "zk provers are the buyer" claim

A recurring motivation (plan §1) is: *no live PoW uses MSM/NTT/Freivalds-matmul as work, and the work
would be exactly what zk provers need, so the PoW gets a real buyer — unlike Pearl's useless matmul.*
Assessed against the evidence here:

1. **Pearl is the useful-work design, not the useless one.** Pearl accepts arbitrary external matrices
   (e.g. AI workloads) and adds low-rank noise so that the useful product can be recovered while the
   PoW stays non-reusable. Candidate A is the one doing useless work: its matrices are random by
   construction.
2. **The novelty is a combination, not a new capability.** "Freivalds-verified matmul as the whole
   work" is, to our knowledge, not deployed, but matrix steps inside PoW (kHeavyHash), useful matmul
   PoW (Pearl) and zk-primitive PoW (Aleo) exist. Novelty alone is not a property.
3. **Usefulness and the soundness we have are in direct conflict.** Candidate A is sound *because* the
   instance comes from the header. A prover's job brings its own inputs (polynomials, scalars, bases,
   witness data). Once inputs are external, every falsifier of `CRITICAL-PATH.md` §3 returns: the job
   author can choose easy or precomputed instances, submit jobs it has already solved, or reuse work
   across nonces. This is the D6 anchor problem, still open.
4. **A PoW needs many attempts; a buyer needs one result.** A block takes `2^bits` independent attempts.
   For the work to be useful, every attempt must compute something the buyer wants on the buyer's
   data, re-randomised per nonce so it is not reusable, with the buyer's result recoverable from the
   randomised one. Pearl achieves this for matmul with low-rank noise; nothing equivalent is designed
   for NTT or MSM here.
5. **Cheap verification does not carry over.** Freivalds verifies a matmul. An MSM or NTT inside a
   proof system is not checked by Freivalds; KZG pairings check an opening, not that an arbitrary MSM
   was computed correctly; for an NTT, verification is only `log n` cheaper than recomputation
   (ADR 0003).
6. **Market fit is unproven.** Proving demand is small, latency-bound and often involves private
   witness data compared with a PoW's continuous, public, embarrassingly parallel lottery; and
   subsidised (useful) work lowers the net cost of attacking the chain, which the security accounting
   would have to absorb.

**Verdict.** As stated, the claim does not hold for anything built here: the working construction
does useless work, and the useful-work variant runs straight into the unsolved anchor problem. It
is a legitimate *hypothesis* for direction 2 below, with the six obstacles above as its falsifiers.

## Update: candidate T (ADR 0015)

Following the owner's goal of a PoW coin, candidate T applies the lessons above: tickets are pieces
of output rows of a header-seeded int8 network (verified by recomputing one row, no proof, no
grindable data, nonlinear requantization between layers). First measurement: 87.8% of an attempt on
tensor cores, 31 ms CPU verification, no cheaper ticket path found (`tnet-v1`). Its distinguishing
claim — the best miner is general AI inference hardware — still needs the precomputation and
fixed-function analyses of spec/07 §4.

## What would change the answer

Pick one distinguishing property and test it; stop if none survives.

1. **Work on general-purpose AI hardware (A8, chosen — ADR 0012).** Use int8 tensor-core matmul so
   that the best mining hardware is AI hardware rather than a single-purpose ASIC. First result: ~78
   TMAC/s exact on the CMP 50HX (~440x the Goldilocks kernel). Test 1 (ADR 0013): a per-attempt
   succinct commitment to `C` costs more than the product and shifts 45%+ of the work to NTT/hashing,
   so the single-GEMM form fails; E1 (winner-only proof of a plain hash) and E2 (deep requantized
   chain) remain untested. The A' bandwidth route (Ethash-class) is not pursued.
2. **Useful work.** Accept external matrices (Pearl-style) and design/verify the anchor that restores
   non-reusability. This is the open research problem the project set out to study.
3. **Proof size.** Replace shipping `C` by a commitment with a sumcheck-based matmul proof; measure
   proof size and verifier time. This would fix the main network cost of candidate A but not points 2–4.

If none of these yields a property that hash-based PoW lacks, the correct outcome is a published
neutral/negative result: "a header-bound Freivalds matmul PoW is sound and implementable, but linear
algebra adds cost, not security or usefulness."
