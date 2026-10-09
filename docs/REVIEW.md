# Review guide — Abacus (experimental Verifiable Algebra PoW)

This is a **research laboratory**, not a protocol, coin or security claim. The goal of a review is to
find where the research is wrong, incomplete or overclaimed. A negative result is a valid outcome.

## What to review (in order)

1. `docs/ASSESSMENT.md` — the current answer to the central question and its limits.
2. `docs/CRITICAL-PATH.md` — the question, the decisive experiments and their status.
3. `spec/03` (candidate A) and `spec/04` (candidate A'); `spec/01`, `spec/02` for the verifiers.
4. `docs/decisions/` — ADR 0001–0011. ADR 0009–0011 record the corrections from the first review
   (soundness bound, consensus fixes, A' measurements).
5. Code: `reference/` (Python) and `crates/` (Rust). Correctness evidence: unit tests, the Python/Rust
   differential and parity tests, and the CUDA miner mining into the Rust node.
6. Empirical: `docs/research/README.md` lists the current notes; superseded and withdrawn notes are
   kept with their reasons.

## Claims under test

> **Candidate A.** A dense matmul `C = A*B` over Goldilocks, with `A, B` derived from the preheader,
> a score `SHA256("abacus/score" || preheader || C)` against a target and `k` Fiat–Shamir Freivalds
> challenges bound to `(preheader, C)`, is a sound permissionless PoW.

> **Assessment.** It is sound at the construction level, but the linear algebra adds cost, not
> security or usefulness (`ASSESSMENT.md`).

Attack these specifically:

- **Soundness.** Per-challenge error `<= 2^-63` and `<= Q * 2^-63k` for a `Q`-query forger (ADR 0009):
  is the non-uniformity of `LE64 mod P` handled correctly, and is binding the root to `(preheader, C)`
  enough?
- **Work accounting.** Work is `n^omega` per attempt and `2^bits` attempts per block (ADR 0005). Is there
  any field that yields attempts without a new product, or any way to obtain `C` cheaper than a
  product of two fresh uniform matrices?
- **Consensus rules of the prototype** (spec/03 §4): committed and enforced difficulty, median-time-past
  timestamps, canonical `C`. Are there remaining inflated-weight or malformed-input paths?
- **A'.** Is the large-slice bandwidth result (ADR 0011) sound, and what do dataset time–memory
  trade-offs do to it? Is "Ethash-class" the right comparison?
- **The assessment.** Is anything overclaimed or underclaimed? Is a stronger differentiating property
  available that we missed?

## What we do *not* claim

No currency, production network or consensus design; no usefulness; no ASIC resistance; no
post-quantum security; no mainnet parameters. GPU numbers are for one device (CMP 50HX) and are not a
matched CPU/GPU comparison.

## How to reproduce

```
python scripts/check.py        # tests, differential/parity, rustfmt, clippy, self-test
bash scripts/gpu_suite.sh      # GPU measurements on a CUDA host
```

Probes are in `scripts/`; raw outputs go to an ignored `artifacts/`.

## Prior art we consulted

Pearl (matmul proofs of useful work), Tenero (matmulhash), kHeavyHash, Ethash/ProgPoW. Please
challenge any place where we claim novelty — we prefer to be told we are wrong.
