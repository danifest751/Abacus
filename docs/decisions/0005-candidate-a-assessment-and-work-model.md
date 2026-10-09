# ADR 0005 — Candidate A: falsifiers, work model and decision

Status: accepted research assessment; no consensus parameters selected.

## Context

Candidate A (header-bound Freivalds matmul PoW, `spec/03`) has now been attacked with the
critical-path falsifiers. This decision records the outcome and the resulting work model.

## Falsifier outcomes

| Falsifier | Outcome | Evidence |
|---|---|---|
| Screening (easy instance) | **excluded** | `A, B` are header-derived and uniform; random dense matrices are full rank (`scripts/instance_probe.py`). A miner cannot request a structured instance. |
| Decomposition / precompute | **excluded** | one product per header; partial products are not reusable across headers; low-rank structure would be `n/(2r)` cheaper but the miner cannot choose `A, B`. |
| Single-challenge forgery | **fixed** | a header-derived `r` known before `C` is forgeable; fixed by `k` Fiat–Shamir challenges bound to the committed `C` (ADR 0004; `scripts/freivalds_forgery_probe.py`). |
| Algorithmic speedup | **accounted** | work is `n^omega`, not `n^3`; the retarget absorbs a uniform constant (`scripts/omega_probe.py`). |
| ASIC/FPGA friendliness | **not excluded** | dense matmul is ASIC-friendly; GPU advantage exists but ASIC resistance does not. Recorded, not hidden. |
| Usefulness | **none** | a random matmul is not a consumer computation. Do not claim usefulness. |

## Work model (candidate)

- Attempt: one `n x n` product over the field = `c * n^omega` operations (`omega` per the best known
  algorithm).
- A block is found when `HASH(domain_score || preheader || encode(C)) <= T` for a `D`-bit digest.
- Expected attempts per block `= 2^D / T`; expected work `= (2^D / T) * c * n^omega`.
- Difficulty retarget adjusts `T` (and `n` only as a profile change); it is ancestor-derived and does
  not use wall-clock or device-reported work.
- Verification per block: `O(k * n^2)` field operations for the `k` Freivalds challenges plus one
  hash. For `n` well above `k` this stays below the work `n^omega`, but verifier throughput must be
  measured for the chosen `k`.

## Decision

- Candidate A **passes the construction-level falsifiers** (screening, decomposition, forgery) and is
  a **viable, simple PoW** modulo the two recorded downsides (ASIC-friendly; not useful).
- Proceed to (a) pick a parameter profile `(n, D, k)`, (b) build a minimal CPU mine-and-verify loop
  for calibration, then (c) the matched CPU/GPU scope (D5). No consensus, network or coin is adopted.

## Consequences

- The critical path updates: A's construction falsifiers are resolved; the open work is the
  parameter/work-model calibration and verifier throughput, not the primitive.
- B (NTT/sumcheck) remains a secondary study (ADR 0003); C stays a reference point.
