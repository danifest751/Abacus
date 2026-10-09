# ABACUS-LAB-v1 — deep requantized int8 network PoW (candidate T)

Status: **frozen as TNet v1** (2026-10-09, ADR 0016; construction ADR 0015). Reference
`crates/abacus-chain/src/tnet.rs` (research copy), Python mirror `reference/tnet.py`, GPU attempt
`cuda/tnet_bench.cu`, test vectors `spec/vectors/`. Node and miner work continues in a separate coin
repository; this file stays the research specification.

## 1. Idea

The work of an attempt is the forward pass of a small header-seeded int8 network on tensor cores; the
lottery tickets are short pieces of the output rows; a block is verified by recomputing **one row**
through the network. No product is shipped, nothing is committed per attempt, and every ticket costs
the same work whatever the miner's strategy. It keeps the measured lessons: exact integer arithmetic,
no per-attempt proof (ADR 0013), no grindable freedom, a nonlinearity between linear layers
(ADR 0010), and hashing kept to a few percent of an attempt (`int8-matmul-v1`). The structure of
per-piece tickets verified by recomputation follows Pearl's tile lottery (ePrint 2025/685) without
noise and without a usefulness claim.

## 2. Construction

```
params: n (width), b (rows per attempt), L (layers), w (ticket bytes, w | n), M (requant multiplier)

epoch:   W_l = int8( expand(SHA256("abacus/tnet-w" || epoch_seed || LE32(l)), n^2) ),  l = 0..L-1   (row-major)
attempt: s = SHA256("abacus/tnet-x0" || header_digest || LE64(nonce))
         X_0 = int8( expand(s, b n) )                                                      (b x n, row-major)
         X_l = requant(X_{l-1} * W_{l-1}),  requant(y) = clamp((y M + 2^23) >> 24, -128, 127)
ticket:  (i, c), 0 <= i < b, 0 <= c < n / w
         h = SHA256( X_L[i, c w .. (c + 1) w] || 0x54 || header_digest || LE64(nonce) || LE32(i) || LE32(c) )
         accept iff leading_zero_bits(h) >= bits
block:   carries (nonce, i, c, piece); the verifier checks h(piece) against the target (one hash),
         then recomputes row i of X_L (L n^2 multiply-adds) and compares the piece
```

Frozen parameters (TNet v1, ADR 0016): `n = 8192`, `b = B = 2^16` (row index bound; rows are
independent, so a miner uses any batch size), `L = 8`, `w = 256`, `M = 2505`. Blocks carry 8 + 4 + 4 +
256 bytes of work data. The coin defines the acceptance rule (a 256-bit target in place of `bits`) and
the epoch schedule.

`expand` is the SHA-256 counter stream of spec/03 (`"abacus/expand"`). `M = round(2^24 / (74 sqrt(n)))`
keeps the activation spread constant (74 ~ the standard deviation of a uniform int8); a power-of-two
scale makes activations saturate or collapse (measured).

## 3. Properties

- **Equal cost per ticket.** A batch of `b` rows costs `L b n^2` multiply-adds for `b n / w` tickets;
  computing a single row costs `L n^2` for `n / w` tickets. Both are `L n w` per ticket; on a GPU the
  batched path is 42–45x cheaper per ticket than single rows (`tnet-v1`), so batching (inference-style)
  is the optimal strategy.
- **No linear shortcut.** A ticket needs exact int8 values of a row of `X_L`, which need whole rows of
  every previous layer at full precision (the requantization is nonlinear and rounds at every layer);
  layers cannot be collapsed into one matrix.
- **No grinding.** Tickets are deterministic in `(header_digest, nonce, i, c)`; the miner has no free
  data to vary.
- **Verification.** One row through `L` layers plus one SHA-256: 11.5 ms (8 threads) / 16.8 ms
  (1 thread) at the frozen parameters on a laptop CPU with a SIMD build, weights stored transposed
  (`tnet-v2`). Nodes hold the epoch weights (`L n^2` = 512 MiB), derived per epoch (5 s single-threaded
  + 2 s transpose).
- **No approximation.** One ±1 error after layer 1 changes 55% of the final row; approximate last
  layers lose pieces faster than they save work (`tnet-v2`).
- **Hardware.** The work is int8 GEMM + requantization, the workload of AI inference accelerators.
  Weights change every epoch, so they cannot be fixed in silicon.

## 4. Open questions (falsifiers)

1. **Precomputation on epoch weights.** Bounded, not measured (`tnet-v2`): per-value tables save
   nothing; bit-plane tables cost `8 / g` int32 additions per multiply-add and `4 · 2^g / g · n^2` bytes
   per layer (4–12x slower than tensor cores on the measured GPU; a multiplier-for-storage trade in
   silicon). A measured LUT kernel would settle it. Any sub-`L n w` route per ticket falsifies the work
   model.
2. **Fused requantization** is a miner optimisation (6.7% of an attempt at the frozen parameters); it
   must reproduce the integer rounding of §2 bit for bit. Consensus keeps the separate definition.
3. **Fixed-function hardware.** Whether an int8-GEMM ASIC without general AI features beats AI
   accelerators by a large factor is not measured; the work is the int8 inference kernel, so such a
   chip is an inference chip.
4. **Light clients**: 512 MiB of weights and ~11 ms per header, or trust in a full node.
