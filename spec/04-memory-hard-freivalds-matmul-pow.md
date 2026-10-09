# ABACUS-LAB-v1 — memory-hard Freivalds matmul PoW (candidate A')

Status: current candidate construction (revised 2026-10-09). Extends candidate A (spec/03) with
operands gathered from an epoch dataset (ADR 0007). The prototype implements the one-word gather
(§3a); the large-slice gather (§3b) exists only in `cuda/attempt_bench.cu`. No parameters are adopted
and no memory-hardness or ASIC-resistance claim is made.

## 1. Idea

Keep header binding and Freivalds verification, but take the operands from a large per-epoch dataset
at header-random positions, so that the cost of an attempt includes memory traffic and a miner must
hold the dataset.

## 2. Epoch dataset

```
blk[0] = SHA256("abacus/ds" || epoch_seed || LE64(0))
blk[u] = SHA256("abacus/ds" || epoch_seed || LE64(u) || blk[u-1] || blk[ref(u)])
ref(u) = LE64(blk[u-1][0..8]) mod u            # data-dependent (ADR 0010)
```

`N` blocks of 32 bytes. Block `u` depends on its predecessor and on an earlier block whose index is
read from the predecessor's content, so the dataset is built sequentially and the reference pattern
is not known before the data. Recomputing one block without storage costs up to `u` hashes in the
naive case; **no time–memory trade-off analysis (checkpointing, pebbling) has been done.**

## 3. Attempt

```
seed = SHA256("abacus/instance" || preheader)
idx  = expand(seed, 2 n^2) mod N
```

**3a. One-word gather (prototype, `crates/abacus-chain`, CPPminer).** `A[i] = LE64(blk[idx[i]][0..8])
mod P`, likewise `B` from `idx[n^2 ..]`. Only 8 bytes of each block are consumed, so a miner needs
`8 N` bytes of storage. Measured on the CMP 50HX this is **compute-bound** for `n >= 256`: `2 n^2`
random reads at ~3.1e9/s take 0.45x (`n = 256`) to 0.11x (`n = 1024`) of the matmul (ADR 0011).

**3b. Large-slice gather (bench only).** Each entry folds a `seg`-byte segment at a random offset with
a **nonlinear** fold (sequential or tree-structured mixing). A linear fold such as a sum is answered
by a per-epoch prefix table with two reads per entry and is forbidden (ADR 0010). With a nonlinear
fold and `seg` above the balance size `n^3 * BW / (2 n^2 * rate)` the attempt is bandwidth-bound for a
tuned miner (measured gather/matmul 1.7–79 for gathering `A` alone, ADR 0011).

Then `C = A * B`, score, challenges and acceptance exactly as in spec/03.

## 4. Verification

Nodes hold the dataset and reproduce `A, B` from the header; verification adds the gather (`2 n^2`
reads, or `2 n^2 seg` bytes for 3b) to spec/03's cost. Supplying operands with Merkle proofs instead
would remove the dataset from nodes but adds `O(n^2 log N)` proof data; not specified.

## 5. Parameter constraints and open questions

- Nonlinear fold for any slice design; `seg` above the balance size.
- `N >> 2 n^2`: with few distinct operand values, products can be grouped by value and the number of
  multiplications drops (with `N < n` distinct values a row needs `N n` instead of `n^2`).
- Verifier cost: dataset in memory (e.g. 2 GiB) and `2 n^2 seg` bytes re-read per block (336 MB at
  `n = 256, seg = 2560 B`).
- Epoch regeneration: sequential; ~177k blocks/s on one GPU thread in CPPminer (1 GiB ≈ 3 min).
- In the bandwidth-bound regime the work is the gather, not the matmul (ADR 0011 §3): A' is then a
  bandwidth PoW with a matmul attached and must be compared with Ethash-class designs.
- Not measured: other devices, energy, an ASIC model, the int8-modulus path of ADR 0008.

## Revision history

- 2026-10-09: data-dependent `ref(u)`; 32-byte blocks with 8-byte consumption stated; nonlinear-fold
  requirement; measured balance and the `N >> 2 n^2` constraint (ADR 0010, 0011). The earlier "~6% of
  sequential bandwidth" figure was a cold-clock artifact and is removed.
