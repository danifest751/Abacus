# Prior art v1 — GPU linear-algebra PoW, proving-as-work and verifiable tensor compute

Date: 2026-10-09. Status: **current**. A literature/deployment scan made before choosing the next
direction. Claims from project write-ups and media are marked as such; they are not independently
verified here.

## Deployed or announced systems

- **Pearl (mainnet April 2026).** Proof of useful work from int8 matrix multiplication ("NoisyGEMM";
  entries restricted to [-64, 64]). The miner commits to its matrices, adds structured noise derived from
  the commitment, computes the noised product in tiles, reduces tiles into a "jackpot" hash checked
  against the target, and on a hit gives a Merkle proof; the verifier recomputes one tile. Theory:
  Komargodski–Schen–Weinstein, "Proofs of Useful Work from Arbitrary Matrix Multiplication"
  (ePrint 2025/685, arXiv 2504.09971). An empirical study (arXiv 2606.04819) reports that most mining
  produced no paid AI output ("usefulness gap").
  *Relevance:* Pearl already is a tensor-core int8 PoW, and its per-tile lottery avoids the per-attempt
  commitment to the whole product that stopped A8 (ADR 0013).
- **Nockchain.** Proof-of-work by generating STARK proofs of NockVM execution over Goldilocks; each nonce
  requires a new trace and proof ("single-witness hardness"); a matrix-multiplication puzzle is announced
  but not live (project docs, Blockworks). *Relevance:* our test 1 measures exactly the cost of a
  per-attempt proof next to a tensor-core product.
- **Aleo.** Proof of succinct work: the coinbase puzzle makes provers run MSM/FFT, rewards are shared
  pro rata among solutions above a minimum target (Aleo docs).

## Research

- **Proof of Necessary Work** (Kattis–Bonneau, ePrint 2020/190): mining produces the recursive SNARKs
  that make the chain's own state verifiable in constant time (prototype, ~40 ms light-client check);
  no production deployment found.
- **Verifiable inference with Freivalds over integerised matmul:** Slalom (TEE + Freivalds), Maverick
  (arXiv 2609.10264), VeriAttn (arXiv 2606.16352); sampled layerwise proofs for LLMs (arXiv 2609.27367).
- **GPU attestation:** dense-matmul utilisation puzzles checked with Freivalds for AI-governance telemetry
  (arXiv 2602.09369); TEE-based GPU attestation in marketplaces.
- **Proof of learning / useful AI work:** Coin.AI (arXiv 1903.09800), PoLe, SEDULity (arXiv 2512.13666),
  which also criticises winner-only useful work.

## What this means for Abacus

- "GPU-optimal linear algebra as PoW" is no longer unexplored: Pearl occupies the int8-GEMM design point
  with a simpler verification (per-tile lottery) than our A8/E1 designs.
- Our measured, transferable results are the analysis ones: per-challenge soundness over a field for
  exact integer products (ADR 0009, 0012), the cost of per-attempt proofs versus tensor-core work
  (ADR 0013), the prefix-sum break of linear gathers (ADR 0010) and the gather balance (ADR 0011).

## Sources

- https://eprint.iacr.org/2025/685 · https://arxiv.org/abs/2504.09971v1 · https://arxiv.org/pdf/2606.04819
- https://www.datawallet.com/crypto/pearl-explained · https://simeononsecurity.com/articles/pearl-prl-mining-guide-2026/
- https://www.nockchain.org/writings/puzzle · https://docs.nockchain.org/architecture/proofs ·
  https://blockworks-research.beehiiv.com/p/nockchains-bet-on-useful-work
- https://developer.aleo.org/guides/faqs · https://aleo.org/post/decentralized-proving-advantages/
- https://eprint.iacr.org/2020/190
- https://arxiv.org/pdf/2609.10264 · https://arxiv.org/pdf/2606.16352 · https://arxiv.org/pdf/2609.27367
- https://arxiv.org/pdf/2602.09369 · https://arxiv.org/pdf/2512.13666 · https://arxiv.org/pdf/1903.09800
