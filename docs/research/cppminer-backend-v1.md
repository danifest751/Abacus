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

## Solo mining (implemented and verified)

The Abacus node gained a job/submit protocol (`p2p.rs`: `JOB` hands out a preheader template +
bits; `SUB <hex>` carries nonce + timestamp + C, validated and appended), and `cp_abacus.cu` gained
a solo client:

```
$ abacus-node --listen 9201 --n 64 --mine 0 --bits 4 --serve 20 &
$ cppminer --algo abacus --backend cuda -d 0 --node 127.0.0.1:9201 --n 64 --seconds 6
[abacus] solo n=64 node=127.0.0.1:9201 jobs=54 found=53 time=6.0s
# node: {"height": 53, "work": 2432, ...}
```

CPPminer mined **53 blocks** into the node's chain in 6 s; every accepted block passed the node's
`k`-challenge Freivalds check. A bug was found and fixed on the way: the client must expand `2*n*n`
elements once and split into `A, B` (expanding twice produced `A == B`, so `C != A*B`).

## GPU instance expansion

`A, B` expansion moved off the host to a device SHA-256 (`seed_kernel`, `expand_kernel`), verified
bit-identical to the host reference (`--selftest`). Rates on the CMP 50HX:

| n | mock attempts/s | note |
|---:|---:|---|
| 64 | 1156 -> **5075** | host expand was the bound |
| 128 | 1528 | |
| 64 solo | 53 -> **63 blocks/6 s** | node accepts all |

The remaining host cost is the **score hash** (SHA-256 over `n^2` elements per attempt), which bounds
larger `n`; moving it to the GPU is the next step.

## Multi-miner (pool)

The node handles concurrent connections (`serve_multi`), so it acts as a simple pool: each client
loops `JOB`/`SUB`. Two CPPminer solo clients against one node:

```
$ abacus-node --listen 9202 --n 64 --bits 4 &
$ cppminer ... --node 127.0.0.1:9202 --n 64 --seconds 8 &   # client 1
$ cppminer ... --node 127.0.0.1:9202 --n 64 --seconds 8 &   # client 2
client1: found 47 ; client2: found 44
node:    {"height": 91, "work": 13568, ...}
```

Two clients grew one chain to 91 blocks. Some found blocks are stale (both clients may mine the same
height before either submits, and the second `SUB` is rejected on a prev-id mismatch) — expected
prototype behaviour; a real pool needs per-client difficulty and stale handling. No share accounting
or payout exists.

## Candidate A' memory-hard path (mock)

`--dataset N [--seg BYTES]` gathers `A, B` from an on-GPU dataset instead of expanding (device
`gather_kernel`). n=64, dataset 1 GiB on the CMP 50HX:

| seg | attempts/s | gather GB/s |
|---:|---:|---:|
| 4 KiB | 2159 | 72.4 |
| 64 KiB | 322 | 173.0 |
| (plain expand, ref) | 5298 | — |

Gather bytes/attempt at seg=4 KiB are 2*n^2*4096 = 33.5 MB, ~0.46 ms at 72 GB/s, versus a ~0.04 ms
matmul — so the gather **dominates**: the memory-hard path works on the GPU. The node-side dataset
(generation + verification) is the next step; the mock is self-contained.

### A' into the chain (solo, verified)

`--node HOST:PORT --dataset NBLOCKS` runs memory-hard solo: CPPminer builds the same host dataset as
the node (`build_dataset`, matching the Rust chain), gathers `A, B` on the GPU, and submits. With
`abacus-node --n 64 --dataset 8000`:

```
cppminer --algo abacus --backend cuda --node 127.0.0.1:9210 --n 64 --dataset 8000 --seconds 6
# node accepted 61 memory-hard blocks; verify=true, ab0 == c0
```

A gather launch-bound bug was found and fixed (the gather used the expand kernel's quarter-block
count instead of one thread per element, leaving most of `A`, `B` unwritten).

## Not implemented

- **Pool mining**: only a single-node solo protocol exists (no share accounting, difficulty
  negotiation or multi-client pool).
- GPU-side instance expansion and score hashing (host-side today, which bounds the attempt rate at
  small `n`).
- The A' memory-hard gather is not wired into the CPPminer path yet.

## Next

1. Pool: share accounting and many miners against one node.
2. GPU-side expand/score; larger `n`.
3. A' memory-hard gather in the CPPminer backend.
