# ADR 0016 — TNet v1: frozen parameters, claimed piece in the block, robustness results

Status: accepted (2026-10-09). Freezes candidate T (ADR 0015, spec/07) as **TNet v1**, the work
function handed over to a separate coin repository. Abacus keeps the research record and the
reference copy `crates/abacus-chain/src/tnet.rs`.

## Context

ADR 0015 met its test criteria at `n = 8192, L = 8` and left four questions: precomputation on epoch
weights, fused requantization, fixed-function hardware, verifier cost and epoch handling. The owner's
goal is a PoW coin, so the construction must be frozen with test vectors before node work starts.

## Decision

1. **Parameters** (`TNET_V1`): `n = 8192`, `L = 8`, `w = 256` (32 tickets per row), `M = 2505`
   (`round(2^24 / (74 sqrt(8192)))`), `B = 2^16` rows per nonce. Rows are independent, so `B` only
   bounds the row index; a miner runs any batch size (the GPU result is the same at `b = 8192` and
   `b = 65536`).
2. **Block carries the piece.** A block carries `(nonce, i, c, piece)`, `piece` being the 256 claimed
   bytes. A verifier first checks that `SHA256(piece || 0x54 || hd || nonce || i || c)` meets the
   target (one hash), then recomputes row `i` and compares the piece. Without the piece a header with
   no work at all would cost the verifier a full recomputation; with it, a forged header costs its
   author a SHA-256 grind to the target.
3. **Requantization stays a separate integer step** in consensus (`clamp((y M + 2^23) >> 24)`); fusing it
   into a GEMM epilogue is a miner optimisation that must reproduce it bit for bit. At `n = 8192` the
   separate pass costs 6.7% of an attempt and the tensor share is 88%; `n = 4096` is not adopted.
4. **Epochs** (recommended for the coin, not fixed here): weights change every `E` blocks, the epoch
   seed is a block id far enough back (e.g. 64 blocks) for nodes to derive the next weights in advance.
   Nodes hold `L n^2 = 512 MiB` per epoch.

## Results (`docs/research/tnet-v2.md`)

- **Verification**: transposed SIMD verifier 11.5 ms (8 threads) and 16.8 ms (1 thread) with
  `target-cpu=native`; 17.6 / 81.8 ms for a baseline x86-64 build. Per epoch: 5.0 s weight derivation
  (single-threaded) + 2.1 s transpose.
- **GPU at frozen parameters**: 88.2% tensor share, 57.5 TMAC/s, 292 ns per ticket, 388 tickets at
  14 bits against 384 expected; the GPU-found ticket is recomputed byte for byte by Rust.
- **Approximation fails**: one ±1 error after layer 1 changes 55% of the final row; dropping 64 of 8192
  product terms in the last layer leaves 26% of pieces exact, int7 weights leave none. Skipping zero
  terms is exact but saves ~0.7%.
- **Precomputation is bounded, not measured**: per-value tables need `1024 n^2` bytes per layer for no
  saving; bit-plane tables (group `g`) need `4 · 2^g / g · n^2` bytes per layer and `8 / g` int32 additions
  per original multiply-add — on the measured GPU 4–12x slower than tensor cores from ALU
  throughput alone; in silicon they trade an int8 multiplier for ≥128x weight storage and bandwidth.
- **Second architecture** (`tnet-ampere-v1`, added the same day): RTX 3090 at the frozen parameters,
  159.5 ns per ticket (1.83x the CMP 50HX), 86.7% tensor share, single rows 51x per ticket, tickets
  accepted by Rust byte for byte.
- **Test vectors**: `spec/vectors/tnet-v1-frozen.jsonl` (frozen parameters, Rust, one row matched to a
  GPU-found ticket) and `tnet-v1-small.jsonl` (`n = 256`, checked against the Python reference).

## Consequences

- TNet v1 is ready to be specified as a coin's work function; the coin is Requant
  (`danifest751/requant`). Open, and stated as such: an int8-GEMM
  ASIC without general AI features (only its advantage over AI accelerators is unknown, not its
  existence), and light clients (512 MiB and ~11 ms per header, or trust in a full node).
- Any change to the parameters, the derivations or the requantization is a new version with new
  vectors.
