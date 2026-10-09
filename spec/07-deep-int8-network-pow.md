# ABACUS-LAB-v1 — deep requantized int8 network PoW (candidate T)

Status: candidate construction (2026-10-09, ADR 0015); reference implementation
`crates/abacus-chain/src/tnet.rs`, GPU attempt `cuda/tnet_bench.cu`. Not yet wired into the chain or
CPPminer. No parameters adopted.

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
block:   carries (nonce, i, c); the verifier recomputes row i of X_L (L n^2 multiply-adds) and h
```

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
- **Verification.** One row through `L` layers plus one SHA-256: 11–24 ms (`n = 4096, L = 8`) and
  31–103 ms (`n = 8192, L = 8`) on a laptop CPU (8 threads / 1 thread). Nodes hold the epoch weights
  (`L n^2` bytes: 128 MiB at `n = 4096`, 512 MiB at `n = 8192`), regenerated per epoch (1.3–5 s CPU).
- **Hardware.** The work is int8 GEMM + requantization, the workload of AI inference accelerators.
  Weights change every epoch, so they cannot be fixed in silicon.

## 4. Open questions (falsifiers)

1. **Precomputation on epoch weights.** Four-Russians-style tables (`x W` via per-value row tables)
   trade multiplies for additions and `~4 n^2` bytes of table reads per row; argued memory-bound, not
   measured. Any sub-`L n w` route per ticket falsifies the work model.
2. **Fused requantization.** The separate requant pass costs ~12% at `n = 4096`; fusing it into the GEMM
   epilogue must reproduce the integer rounding of §2 bit for bit across implementations.
3. **Fixed-function hardware.** Whether an int8-GEMM ASIC without general AI features beats AI
   accelerators by a large factor (the hardware claim of ADR 0012) is not measured.
4. **Epoch transitions and verifier memory** for light clients (weights must be regenerated or
   fetched).
