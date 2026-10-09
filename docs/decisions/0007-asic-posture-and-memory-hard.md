# ADR 0007 — ASIC posture and the memory-hard direction (candidate A')

Status: accepted research direction. Records the ASIC tension and adopts a memory-hard variant as the
Amended by ADR 0008, 0010 and 0011: the dataset references are now data-dependent; the large-slice
gather needs a nonlinear fold; the A' measurements are in ADR 0011.
default candidate.

## The tension

Candidate A is built on dense matmul, and dense matmul is **GPU-optimal and ASIC-optimal**: a tensor
core / systolic array / TPU does matrix multiplication better than any GPU. "Guaranteed GPU advantage"
therefore does **not** imply ASIC resistance; if anything the same property that makes it GPU-optimal
makes it ASIC-friendly. This was recorded as a downside in ADR 0005 and is now a decision point.

## Options

1. **Accept ASIC-friendliness.** Bitcoin (SHA-256), Litecoin (scrypt), and Pearl (matmul) are
   ASIC-dominated and still function. Honest, simple — but decentralisation weakens and the "GPU coin"
   framing is lost.
2. **Add memory-hardness (candidate A').** Make the bottleneck **memory bandwidth** rather than raw
   multiply throughput, so an ASIC must attach comparable high-bandwidth memory (expensive), and
   regenerate the working set per epoch so a fixed ASIC is obsolete quickly (algorithm agility).
3. **Pick a less ASIC-friendly but still GPU-usable primitive.** Hard: the most GPU-optimal primitive
   is the most ASIC-optimal one; anything less GPU-friendly weakens the premise.

## Decision

- Pursue **candidate A' (memory-hard Freivalds matmul PoW)** as the default; keep plain A as the
  measured baseline.
- **Design sketch.** Keep header binding and Freivalds verification, but do not let the operands be
  small and freely computed:
  - an **epoch dataset `D`** (e.g. 1-4 GiB) is built deterministically and **data-dependently** from
    the epoch seed (sequential construction), regenerated per epoch;
  - per attempt, `A` and/or `B` are **assembled by gathering** columns/blocks from `D` using a
    header-derived index (a Tenero-§8.3-style gather), so the work is dominated by **random,
    bandwidth-bound reads**;
  - the product `C = A*B` and the `k`-challenge Freivalds check are unchanged.
- **New falsifiers to test** (append to `docs/CRITICAL-PATH.md`):
  - **GPU advantage under gathered access**: is the gather bandwidth-bound, and does a high-bandwidth
    GPU (HBM) beat a compute-rich ASIC on it?
  - **Work-model monotonicity**: is the work dominated by bytes fetched, not multiplies, and is it
    monotone in the difficulty knob?
  - **Precompute / reuse**: gathering must be header-random per attempt; test that operands cannot be
    cached across attempts.
  - **Verifier cost**: the verifier needs the dataset (or the miner's operands with proofs); quantify
    the added cost and the `O(k n^2)` budget.
  - **Agility**: the epoch regeneration must be cheap enough to run but hostile to a fixed ASIC.
- The **probe** (`cuda/gather_bench.cu`) measures the gathered-read bandwidth on the GPU to check the
  premise before committing to parameters.

## Consequences

- The plain candidate A (ADR 0005) remains the reference and the correctness baseline; A' adds the
  memory-hard layer and its falsifiers.
- Address this in the threat model (ASIC section) and in the critical-path falsifier list.
- Honest caveat: memory-hard algorithms eventually get ASICs (Ethash, scrypt); agility narrows the
  window, it does not close it. The goal is a high ASIC barrier, not immunity.
