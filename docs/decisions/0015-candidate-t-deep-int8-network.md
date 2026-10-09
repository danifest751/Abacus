# ADR 0015 — Candidate T: deep requantized int8 network PoW with row-piece tickets

Status: accepted as the primary PoW candidate (2026-10-09), by decision of the project owner to pursue
a PoW coin. Supersedes A8 (ADR 0012/0013) as the PoW line; ADR 0014 (attestation) remains a side track.

## Context

Measured so far: an int8 tensor-core product is ~440x cheaper per operation than field arithmetic, so a
PoW cannot afford a per-attempt commitment or proof of the product (ADR 0013); sampled checks of a
committed product are grindable; linear reductions are bypassable (ADR 0010); a winner-only proof takes
minutes (E1). Pearl's per-tile lottery shows the structure that survives: each small piece of the
result is a ticket verified by recomputation.

## Decision

Pursue **candidate T** (spec/07): per attempt, `L` layers of `X_l = requant(X_{l-1} W_l)` on int8 with
epoch weights and a header/nonce-seeded input; tickets are `w`-byte pieces of output rows; a block
carries `(nonce, i, c)` and is verified by recomputing row `i` through the layers.

Test criteria (set before the run): tensor-core share of an attempt >= 85%, CPU verification <= ~100 ms,
no cheaper ticket path, fair lottery.

## Result (`docs/research/tnet-v1.md`)

Met at `n = 8192, L = 8`: 87.8% tensor share (59 TMAC/s per attempt), verification 31 ms on 8 CPU
threads (103 ms on one), single-row mining 42–45x more expensive per ticket, ticket counts as expected,
activations using the full int8 range after replacing a power-of-two scale by a fixed-point multiplier.
At `n = 4096` the tensor share is 79–83% because of the separate requantization pass.

## Next

1. Fuse requantization into the GEMM epilogue with bit-exact integer rounding (lifts `n = 4096`).
2. Analyse precomputation on fixed epoch weights (Four-Russians tables) and fixed-function hardware.
3. Wire candidate T into the chain prototype (block fields, epoch seed from the chain, weights cache)
   and into CPPminer as the GPU miner.

Follow-up: ADR 0016 (frozen as TNet v1; item 1 settled as a miner optimisation, item 2 bounded,
item 3 handed to the coin repository).
