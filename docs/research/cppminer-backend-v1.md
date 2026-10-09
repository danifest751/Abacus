# CPPminer backend v1 — `--algo abacus` (candidate A) on CUDA

Date: 2026-10-09. Repo: `danifest751/CPPminer`, branch `feat/abacus-backend` (from `ca259ff`).

The Abacus PoW (candidate A) is integrated into CPPminer as a new algorithm:

- `include/cp_abacus.h`, `include/cp_algo.h` (adds `CP_ALGO_ABACUS`), `src/common/cp_algo.cpp`
  (parse/name/supports), `src/common/main.cpp` (an early `--algo abacus` dispatch, since the shared
  parser rejects abacus flags), `CMakeLists.txt`.
- `src/abacus/cp_abacus.{cpp,cu}`: argument handling and a CUDA **mock** nonce search.

## Encoding (matches the Abacus prototype byte-for-byte)

```
preheader = "abacus/ph" || chain_id(32) || version(u32 LE) || prev(32) || height(u64 LE)
            || timestamp(u64 LE) || nonce(u64 LE)
seed      = SHA256("abacus/instance" || preheader)
A, B      = expand(seed, 2*n*n)        # SHA256 counter mode, 4 Goldilocks elements per hash
C         = A * B over Goldilocks (P = 2^64 - 2^32 + 1)
score     = SHA256("abacus/score" || preheader || C_le)
accept    = leading_zero_bits(score) >= bits
```

Because the encoding matches `crates/abacus-chain`, a found nonce/block can be submitted to the
Abacus node once a job/submit protocol exists.

## Result (CMP 50HX, sm_75)

```
$ cppminer --algo abacus --backend cuda -d 0 --mock --n 64 --bits 4 --seconds 6
[abacus] n=64 bits=4 device=0 attempts=6935 found=420 attempts/s=1155.8
```

~1156 attempts/s at `n=64` (found ≈ 1/16, as expected for `bits=4`). The rate is **host-bound**
(per-attempt SHA-256 expansion and score on the CPU at this small `n`); the GPU matmul is a small
part. Larger `n` or GPU-side expansion/score would shift the balance.

## Not implemented

- **Solo / pool mining.** The Abacus node prototype exposes only a pull-sync protocol
  (`p2p.rs`), not a job/submit protocol. A solo miner needs the node to hand out a header template
  + target and accept `(nonce, C)`.
- GPU-side instance expansion and score hashing (host-side today).

## Next

1. Add a **job/submit** message to `abacus-node` (hand out a preheader template + bits; accept a
   submitted nonce+C and validate).
2. Add a **solo client** to `cp_abacus.cu` that connects, mines, and submits.
3. Then the A' memory-hard gather (dataset sync/regen) and a pool.
