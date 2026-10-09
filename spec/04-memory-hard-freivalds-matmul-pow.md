# ABACUS-LAB-v1 — memory-hard Freivalds matmul PoW (candidate A')

Status: candidate construction (research). Extends candidate A (`spec/03`) with a memory-hard,
data-dependent operand layer to raise the ASIC barrier (ADR 0007). No consensus is adopted.

## 1. Idea

Dense matmul is GPU-optimal and ASIC-optimal. A' keeps header binding and Freivalds verification but
makes the operands come from a **large, non-recomputable epoch dataset** via a **header-random
gather**, so the bottleneck is memory access pattern, not multiply throughput, and a fixed ASIC is
obsoleted by per-epoch dataset regeneration.

## 2. Epoch dataset `D`

- Size `S` (e.g. 1-4 GiB), divided into `N` fixed blocks (32 bytes each in the prototype).
- **Sequentially and data-dependently constructed** from `epoch_seed`:
  ```
  blk[0]   = H(domain_ds || epoch_seed || 0)
  blk[u]   = H(domain_ds || epoch_seed || u || blk[u-1] || blk[ref(u)])
  ref(u)   = LE64(blk[u-1][0..8]) mod u                       # data-dependent (ADR 0010)
  epoch_seed = H(domain_epoch || epoch_index)                  # changes each epoch
  ```
  The chain makes a random block **not cheaply recomputable** without the prefix: to read block `u`
  you either built and stored the chain or pay `O(u)`. A miner therefore must **hold `D`** to access
  it randomly, which is the memory-hard property. No time-memory trade-off (checkpointing,
  pebbling) analysis has been done; the `O(u)` figure is the naive no-checkpoint cost.
- **Effective storage.** The prototype gather consumes only the first 8 bytes of a block
  (`field_from_block`), so a miner needs `8 * N` bytes, not `S = 32 * N`. Either state `S` as
  `8 * N` or make the gather consume whole blocks.
- Regenerated every `E` blocks (epoch); regeneration must be cheap for honest miners but hostile to a
  fixed ASIC (algorithm agility).

## 3. Attempt

```
seed     = HASH(domain_instance || preheader)
idx      = expand_indices(seed, n*n)          # n*n positions in [0, N)
A[i][j]  = load_block(D, idx[i*n+j])          # header-random gather  (O(n^2) reads)
B        = fixed small matrix from seed (or a second gather)
C        = A * B                               # O(n^omega) multiplies
r_i      = HASH(domain_check || preheader || encode(C) || i), i=1..k   # FS-bound to C
accept   = ( all_i Freivalds_verify(A, B, C, r_i) )
           and ( HASH(domain_score || preheader || encode(C)) <= T )
```

The work has two components: `n^2` random dataset reads plus `n^omega` multiplies. Parameters must be
chosen so the two are **comparable** (both matter); otherwise the memory layer is cosmetic.

## 4. Verification

- Nodes keep the epoch dataset `D` (Ethash-style) and derive `idx` from the header, so they can
  reproduce `A` exactly.
- Verification = gather `O(n^2)` reads + `k` Freivalds checks `O(k n^2)` + one hash. It must remain
  below the work (`O(n^2 + n^omega)`); with `n > 2k` and a matmul-dominated work, this holds.
- Open: whether nodes can avoid storing `D` (e.g. miner supplies `A` with a Merkle proof against a
  root vs. the gather indices). Spec TBD; the simple version requires nodes to hold `D`.

## 5. Parameters and open questions (falsifiers)

- `(n, S, E, D_bits, k)` with `n > 2k`; the gather (bytes) and matmul (multiplies) must be balanced.
- **Gather bandwidth**: warm, block-cooperative 64 KiB gathered reads reach ~416 GB/s, ~87% of
  sequential on the CMP (`docs/research/gather-bandwidth-v1.md`; the earlier "~6%" was a cold-clock
  artifact and is withdrawn) — so a memory-hard layer needs a very large per-attempt gather.
- **Nonlinear fold**: any per-entry fold of a gathered segment must be nonlinear and sequential. A
  linear fold (e.g. a sum) is answered by two prefix-sum reads and is not memory-hard (ADR 0010).
- **No cross-attempt caching**: `idx` is header/seed derived and must change every attempt.
- **Dataset not recomputable**: the data-dependent chain must actually force storage (measure the cost
  of recomputing a random block vs reading it).
- **Verifier dataset cost**: whether holding `D` on nodes is acceptable.
- **Agility**: epoch regeneration cost vs ASIC-hostility.
- **Balance**: if matmul dominates, the memory layer does not change the ASIC story; if gather
  dominates, it is closer to a memory-hard PoW with a matmul wrapper. **ADR 0008: with one 32-byte
  block per entry the matmul dominates on the CMP (not memory-hard); memory-hardness requires a
  large-slice gather (`G > n^3 * BW / rate`) and/or an int8-representable modulus with tensor cores,
  and makes the PoW bandwidth-bound.**

## 6. Scope

Research candidate; no signatures, rewards, mempool or P2P. `A` and `C` are `n^2` elements on the wire
in the simple version (bounded encoding required). No security, usefulness or ASIC-immunity claim.
