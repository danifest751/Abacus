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
- **B. Sumcheck/GKR NTT.** Header → field element / domain; miner computes an NTT + sumcheck
  transcript; verifier checks in `O(log n)`.
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
- Status: `docs/STATUS.md`.
- Use "experimental Verifiable Algebra PoW"; make no security or usefulness claims.

## 8. Status (2026-10-09)

- **Candidate A falsifiers resolved** (see ADR 0004, ADR 0005): screening and decomposition are
  excluded by header-derived uniform `A, B`; the single-challenge forgery is fixed by `k`
  Fiat–Shamir-bound Freivalds challenges; algorithmic speedup is accounted as `n^omega`. Recorded
  downsides: ASIC-friendly, not useful.
- **D2/D3/D4 done on candidate A**; probes: `instance_probe.py`, `omega_probe.py`,
  `freivalds_forgery_probe.py`. A toy CPU mine+verify loop works (`mine_sim.py`).
- **GPU baseline (E5 start)**: Goldilocks matmul GPU vs CPU on the CMP 50HX (`gpu-baseline-v1.md`);
  the GPU advantage is a ~constant factor, i.e. **work is portable**, not superlinear.
- **Next**: (1) Rust parity for the Fiat–Shamir Freivalds binding; (2) choose a parameter profile
  `(n, D, k)` and measure verifier throughput; (3) matched CPU/GPU mine-scope (D5) with the parity
  kernel.
