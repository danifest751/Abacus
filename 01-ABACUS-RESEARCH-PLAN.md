# Abacus Network — verifiable GPU-algebra proof of work

Initial research plan. Version: 0.1. Date: 2026-10-09.
Status: **research plan — not a completed protocol, network, coin or security claim.**
Working label: "Verifiable Algebra PoW" (VAP). Project name: **Abacus**.

## 1. Objective and sequence

Determine whether a **linear-algebra computation that is intrinsically GPU-optimal** can
support standalone proof of work: costly search, **inexpensive independent verification**,
understandable difficulty adjustment, and resistance to work reuse and precomputation.

The three target primitives are the ones where the GPU advantage is a **fact, not a
hypothesis**:
- dense **matrix multiplication** (tensor cores) — `C = A·B`;
- **NTT/FFT** over a finite field (the transform at the core of STARK/FRI proving);
- **MSM** (multi-scalar multiplication on elliptic curves — the bottleneck of KZG provers).

Verification of a *claimed* result is cheap and provable:
- **Freivalds' algorithm** for matmul: check `A·(B·r) == C·r` for random `r` — `O(n²)` vs
  `O(n³)`, error ≤ 1/2 (amplified with a few vectors);
- **Sumcheck / GKR** for NTT and other layered linear maps — `O(log n)` verification;
- **KZG pairing checks** for MSM-derived commitments — a constant number of pairings.

A useful instantiation exists: if the work is *the proving computation itself*, the PoW has a
**real buyer** (ZK proving markets), unlike "useless" matmul PoW.

Sequence: mathematical specification → reference verifier → tests and simple attacks →
CPU measurements → work model → consensus simulator → GPU miner → local network → external
review → public testnet. Deliver a reproducible laboratory and a **decision to continue,
revise or reject**. A negative result is useful.

Do not fork a monetary blockchain before an acceptable work model exists.

## 2. Why "guaranteed GPU"

For lattice/SIS PoW the GPU advantage is unproven; for Abacus it is not. Dense matmul, NTT and
MSM are the most GPU-bound operations in all of cryptography:
- every zk-SNARK/STARK prover is bottlenecked on **MSM and NTT**, both dominated by memory
  bandwidth and tensor-core throughput;
- int8/FP16 matmul is the canonical tensor-core workload.

Therefore the hardware story is settled a priori: the research is **not** "does the GPU win",
but "**can a linear, GPU-optimal computation be turned into a permissionless puzzle.**"

## 3. Corrections to the naive starting proposal

| Assumption | Correction |
|---|---|
| "Verification is cheap, so it's a PoW" | Cheap verification is necessary, not sufficient. The puzzle must be **permissionless** and its work **countable**. |
| "One random matmul per header" | Matmul (and NTT, and MSM) are **linear**, so a miner can decompose, precompute and reuse — this is the central unsoundness to solve. |
| "Freivalds proves the work" | Freivalds checks the *result*, not that the miner did the work. A cheater who found a cheaper route is indistinguishable. |
| "Difficulty = matrix size" | Size sets a floor, not a searchable lottery; without a target/grinding rule there is no adjustable difficulty. |
| "GPU-native implies ASIC-resistant" | MSM/NTT/matmul are heavily ASIC/FPGA-friendly; memory-hardness must be engineered, not assumed. |
| "Useful work is free" | A real buyer (proving markets) is an opportunity, but it couples the PoW to external demand and adds trust questions. |
| "Just replace CheckProofOfWork" | Header binding, work pricing, difficulty, chainwork, resource limits and proof delivery all change. |

## 4. The central research question

**Can a permissionless PoW be built from a linear, GPU-optimal computation with cheap
probabilistic verification, while resisting decomposition, precomputation and reuse?**

Linearity is the enemy:
- `(A₁ + A₂)·B = A₁B + A₂B` → a miner can split the work and cache partial products;
- NTT is linear over the field → the same holds with block/butterfly decomposition;
- MSM is linear in the scalars and the points.

A permissionless puzzle therefore needs a **nonlinear anchor**: e.g. a hash of partial results
folded back into the computation, per-block fresh randomness that must be consumed *before* the
heavy work, a low-rank noise term (Pearl's approach) that destroys linearity, or a sequential
dependency. **Characterising and minimising this anchor is the core of the project.**

## 5. Candidate constructions

- **A. Freivalds matmul PoW.** Header → random `A`, `B`; miner computes `C`; a header-derived
  target is compared against a hash of the transcript; the verifier checks `C` with Freivalds.
  Open problem: linearity (decomposition/precompute) — must add an anchor.
- **B. Sumcheck/GKR NTT PoW.** Header → random field element / evaluation domain; miner computes
  a large NTT and emits a sumcheck transcript; the verifier checks it in `O(log n)`; a target on
  the transcript hash sets difficulty. Reuse and instance selection must be bounded.
- **C. MSM PoW with KZG verification.** Header → scalars/points; miner computes an MSM and a
  commitment; the verifier runs pairing checks. Closest to "useful proving"; linearity and the
  need for a random challenge are the open problems.

Distinct constructions get distinct `pow_version` identifiers and incompatible vectors. Compare
intended solvers against adversarial minimum cost — honest implementation speed is not the
adversary's cost.

## 6. Binding work to blocks

```
preheader  = encode(chain_id, protocol_version, pow_version, parameter_profile,
                    previous_block_id, height, tx_root, timestamp,
                    difficulty_descriptor, reward_commitment, nonce)
instance   = HASH(domain_instance || preheader)          # derives A/B, NTT domain, or MSM points
work       = compute(instance)                            # GPU-optimal linear algebra
proof      = canonical_encode(work_output, transcript)    # result + (sumcheck) transcript
block_id   = HASH(domain_block || preheader || proof)
```

Nothing that yields new attempts may vary without changing the challenge; replaying a proof must
not create additional work; an old output must not coincidentally satisfy a changed instance
without regeneration.

## 7. Work accounting and difficulty

Define a candidate **work function by component** and show it is **monotone** in the difficulty
knob and **non-transferable** across parents/chains:
- components: instance generation, the heavy computation, any anchor/nonlinear step, proof
  construction, failed attempts;
- do not use wall-clock seconds, device-reported work, raw output size or vector length;
- study reuse: cost(first) / cost(subsequent); cross-header freshness; precomputation and
  memory/time trade-offs (a memory-rich device must not win superlinearly);
- difficulty retarget uses only chain-derived quantities; do not adopt WTEMA/ASERT by name before
  the model is understood.

## 8. Consensus decisions to specify (candidate, not adopted)

Fork choice (validity + weight, equal-work behaviour), retarget (ancestor-derived start, bounds,
rounding), timestamps, block interval (compare 30/60/120 s), proof format and worst-case
acceptance/rejection cost, header sync incl. light clients, profiles and activation, reorg and
no duplicate reward, upgrades. Confirmations are probabilistic, not final.

## 9. Test plan (initial)

| ID | Case | Expected property |
|---|---|---|
| M01 | Manual fixtures (tiny matmul/NTT/MSM) | Exact accept/reject |
| M02 | Malformed/short proof, wrong sizes | Early rejection, bounded allocation |
| M03 | Freivalds/sumcheck boundary | Exact cross-language agreement |
| M04 | Field/curve edge cases | Defined arithmetic or safe refusal |
| M05 | Decomposition/linearity probes | Expose trivial cheap routes |
| M06 | Unknown versions/huge sizes | Reject before large allocation |
| M07 | Alternative encodings, extra bytes | Canonical encoding |
| M08 | Same seed across platforms | Same instance and bytes |
| M09 | Anchor boundaries (noise/hash fold) | Independent reference agreement |

| ID | Attack | Investigate |
|---|---|---|
| A01 | Change header/nonce/root/payout | Regeneration; free attempt fields |
| A02 | Replay proofs/jobs | Duplicate credit |
| A03 | Precompute/decompose (linearity) | Cheap inflated weight |
| A04 | Reuse bases/plans across jobs | Amortisation |
| A05 | Memory/device scaling | Superlinear advantage, hardware barriers |
| A06 | Almost-valid proofs | Expensive rejection / node overload |
| A07 | Target/time manipulation | Inflated weight, retarget bypass |
| A08 | Offload the heavy compute | Outsourcing vs local work |

Compare against an independent reference; GPU search heuristics may use approximate arithmetic
but outputs must pass exact verification. Add a bounded local fuzzing session with saved seeds.

## 10. Measurements

Record source/config hashes, seeds, CPU/GPU/RAM/VRAM, OS/drivers/compiler flags; generation,
compute, anchor, proof and verification time; peak memory and transfers; first vs subsequent
cost; failures and timeouts (as censored observations). Separate cold and steady state. Publish
raw data and uncertainty. Provisional filter: verification including instance generation
p95 ≤ 10 ms on an ordinary CPU; proof ≤ 16 KiB (engineering screens, not promises).

## 11. Project layout

```
spec/                 mathematical and protocol specifications
reference/            transparent Python verifier and bounded probes
crates/               independent Rust verifier
cuda/                 GPU kernels (matmul / NTT / MSM)
reference-solvers/    CPU baselines
simulator/            chain, difficulty and fork experiments
tests/vectors/        fixed fixtures
tests/adversarial/    regression corpus
benchmarks/           pinned experiments
docs/                 status, threat model, decisions
```

## 12. Milestones and gates

| Stage | Deliverable | Gate |
|---|---|---|
| E0 | Math, encoding, header binding, threats, manual cases | Coherent definitions |
| E1 | Reference verifier (Freivalds/sumcheck) + differential corpus | Agreement, bounded parsing |
| E2 | CPU baselines, raw data, linearity/precompute attacks | No trivial cheap bypass in a retained candidate |
| E3 | Construction decision + work model + external review | Defensible, monotone work function |
| E4 | Difficulty/retarget/time-overload simulator | Deterministic rules, no cheap inflated-weight branch |
| E5 | GPU miner and hardware comparison | Exact verification; resource/energy/scaling data |
| E6 | Local nodes and ≥2 miner processes | Sync, forks, restart, reorg pass |
| E7 | External testnet and published artifacts | Blocking findings closed; complete resource rules |

Testnet does not imply coin value. Estimate E2/E3 effort from data, not from this plan.

## 13. Stop or revise when

- Accepted proofs multiply cheaply without corresponding work accounting.
- Linearity (decomposition/precompute) cannot be bounded by an anchor.
- Work cannot be compared after difficulty changes.
- Invalid proofs cheaply overload full nodes.
- GPU advantage turns out unsupported (contradicting §2) — investigate before continuing.
- A changed task retains an obsolete security argument.
- Progress depends on unestablished claims.

Reject or revise explicitly. Switching primitive (matmul → NTT → MSM) or adding an anchor is a
documented decision, not a hidden fallback.

## 14. Open questions

Which primitive gives the cleanest permissionless puzzle; minimal nonlinear anchor; work/difficulty
definition; reuse and precomputation bounds; ASIC/memory-hardness position; light-client proof
delivery; usefulness/demand coupling if the proving instantiation is pursued; external review.

Use "experimental Verifiable Algebra PoW" until specific claims have adequate evidence.

## 15. Sources

- Freivalds, *Fast probabilistic algorithms* (matrix product verification), 1977/1979.
- Sumcheck protocol and GKR (Goldwasser–Kalai–Rothblum) for verifiable layered circuits.
- KZG polynomial commitments (Kate–Zaverucha–Goldberg) — pairing-based verification.
- Pearl / `proofs of useful work from matmul` — prior art on matmul PoW (and its noising anchor).
- CuPOW / Qubic / Bittensor — prior art on "useful work" (for contrast; neither is this design).
