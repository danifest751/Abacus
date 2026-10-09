# Critical path to understanding

Date: 2026-10-09. Status: working directive. This file narrows the programme to the single open
decision and the minimum experiments that resolve it. Read it before starting any task; every task
must map to it or be explicitly parked.

## 1. The one open question

**Can a permissionless PoW be built from a linear, GPU-optimal computation (matmul, NTT, MSM) with
cheap probabilistic verification, while resisting decomposition, precomputation and reuse?**

Everything else is secondary. If a defensible construction exists, the project can proceed to a
chain. If not, the correct outcome is an **explicit, published negative result**.

## 2. Rules

1. **One critical path.** Every proposed task states which experiment below it advances; tasks that
   do not map are parked in §6.
2. **Decision value, not volume.** Before any milestone ask: *does it change the answer to §1?* If
   no, do not build it.
3. **Falsification first.** Try to break candidate work functions (cheap inflated weight, reuse,
   screening, precompute), not to confirm them.
4. **Timebox and kill.** Each experiment has a budget and a stop criterion; on failure, record and
   stop.
5. **Negative result in parallel.** Draft the negative-result note while running.
6. **External review early**, before large investment.
7. **Scale only where it matters**, at the smallest profile where the effects appear.
8. **No consensus/monetary surface** until the work function passes review.

## 3. Candidate work functions and falsifiers

- **A. Freivalds matmul.** Header → `A,B`; miner computes `C`; target on a transcript hash;
  verifier checks with Freivalds.
  - Falsifiers: linear decomposition/precompute; plan/basis reuse; screening; whole-result
    outsourcing; challenge chosen by the miner.
- **A'. Memory-hard Freivalds matmul (ADR 0007).** As A, but the operands are **gathered** from a
  large, per-epoch, data-dependent dataset with a header-random index, so the bottleneck is **memory
  bandwidth** (ASIC-harder).
  - Additional falsifiers: gathered access stays bandwidth-bound and GPU-favourable; work model
    monotone in bytes fetched; operands not cacheable across attempts; verifier dataset cost; epoch
    regeneration cheap yet ASIC-hostile.
  - **Status (ADR 0011):** one gathered word per entry is compute-bound; a nonlinear large-slice
    gather is bandwidth-bound for a tuned miner, which makes A' an Ethash-class bandwidth PoW; a linear
    fold is broken by prefix sums (ADR 0010).
- **A8. int8 tensor-core matmul (ADR 0012, spec/05).** As A, but `A, B` are int8 and the work is the
  exact int32 product on tensor cores; Freivalds over Goldilocks verifies it.
  - Falsifiers: commitment/proof cost of `C` vs the product; block size; a single-purpose design
    beating AI hardware; any sub-`n^3` route for random int8 inputs; linear or sampled scores.
- **B. Sumcheck/GKR NTT.** Header → field element / domain; miner computes an NTT + sumcheck
  transcript; the verifier checks `O(n)` round values plus one evaluation of the polynomial (sublinear
  only with a commitment opening; the implemented verifier reads the full table).
  - Falsifiers: NTT linearity (block/butterfly reuse, precomputed twiddle plans); transcript
    reuse; domain selection; sublinear verification does not imply sublinear *work*.
- **C. MSM with KZG.** Header → scalars/points; miner computes an MSM + commitment; pairing checks.
  - Falsifiers: MSM linearity in scalars/points; batched/precomputed bases; pairing-check scope
    (cheap verification vs which work it certifies).

Each weight is defined by component (instance generation, heavy compute, anchor, proof, failed
attempts) and shown **monotone** in the difficulty knob and **non-transferable**.

## 4. Decisive experiment sequence

- **D1 — This file**: agreed experiment list, profiles, scopes, thresholds.
- **D2 — Formal candidate work functions + falsifiers** (write §3 in full).
- **D3 — Screening attack.** Does *selecting* header/nonce/instance give **> 2x**? If yes, the
  construction is likely unsound.
- **D4 — Precomputation / amortisation attack.** `cost(first)/cost(subsequent)` for cached
  products/plans/bases; does reuse break monotonicity?
- **D5 — Matched CPU/GPU scope.** One CPU and one GPU at matched domain/caps/output/timer: is the
  weight (energy x time) portable, or does a device win superlinearly?
- **D6 — Anchor design and proof.** Define and justify the minimal nonlinear anchor; only then a
  difficulty/fork simulator (E4).

## 5. Success metric

A written, defensible answer to §1, backed by reproducible evidence and an external review note —
**not** a larger codebase, more tests, or a miner counter.

## 6. Parked until the path resolves

- GPU kernels beyond what D3–D5 need; any production miner.
- chain/consensus/retarget beyond the minimal D6 simulator.
- wallets, pools, payments, monetary framing.
- new document families unless they resolve a fork in §4.

## 7. Relationship to existing documents

- Plan and gates: `01-ABACUS-RESEARCH-PLAN.md` (§12 E0–E7, §13 stop/revise).
- Threats: `docs/THREAT-MODEL.md` (linearity, precompute, reuse).
- Status: `docs/STATUS.md`; current answer to §1: `docs/ASSESSMENT.md`; research record:
  `docs/research/README.md`.
- Use "experimental Verifiable Algebra PoW"; make no security or usefulness claims.

## 8. Status (2026-10-09)

- **D1–D4 done for candidate A.** Screening and reuse are excluded by the header-derived instance;
  the single-challenge forgery is fixed by Fiat–Shamir challenges bound to `(preheader, C)` (ADR 0004,
  0010); work is priced as `n^omega` (ADR 0005); per-challenge error `<= 2^-63`, `k = 2..3` (ADR 0009).
- **D5 not done.** Only GPU throughput is measured (~175 GMAC/s warm, `gpu-suite-v1`); the matched
  CPU/GPU energy x time comparison has not been run.
- **D6 not done.** Candidate A avoids the anchor by never accepting external data; the anchor problem
  remains open for useful-work variants.
- **E6 prototype done** (beyond the minimal D6 simulator of §6): chain, sync and a GPU miner, used to
  validate the construction end to end (`chain-prototype-v2`). It found and fixed consensus bugs
  (ADR 0010) and is frozen.
- **A8** (ADR 0012, 0013): ~78 TMAC/s exact int8 on tensor cores. Test 1: a per-attempt succinct
  commitment to `C` costs more than the product, so the single-GEMM form is stopped; escapes E1/E2 are
  open hypotheses.
- **Candidate T (ADR 0015, spec/07)** is the primary PoW candidate: deep requantized int8 network,
  row-piece tickets verified by recomputing one row; first test passed (`tnet-v1`).
- **Second track (ADR 0014)**: interactive proof of tensor throughput (spec/06), outside the PoW
  question by owner decision; the PoW results are written up in `docs/papers/tensor-pow-limits.md`.
- **Answer to §1**: `docs/ASSESSMENT.md` — yes at the construction level, but the linear algebra adds
  cost, not security or usefulness; three directions could change that, otherwise publish the neutral
  result.
