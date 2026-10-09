# CPPminer backend v1 — `--algo abacus` (candidate A) on CUDA

Date: 2026-10-09. Repo: `danifest751/CPPminer`, branch `feat/abacus-backend` (from `ca259ff`).

> **Compatibility (ADR 0010).** The sections below up to "Encoding v2" were measured against
> encoding **v1**. CPPminer is now on **v2** (see the last section). The "GPU dataset ...
> data-dependent chain" below was index-dependent only in v1.
>
> **Correction to the A' mock table.** `--dataset` was always a **block count** (32-byte blocks), not
> MiB, and `--seg` was parsed but never used by the kernel. The "dataset 1 GiB, seg 4 KiB / 64 KiB"
> rows therefore ran on 1024 blocks (32 KiB) with identical kernels; their difference and the
> "gather dominates" reading are not supported. The `gather GB/s` column also counted 32 bytes per
> element although 8 are read.

The Abacus PoW (candidate A) is integrated into CPPminer as a new algorithm:

- `include/cp_abacus.h`, `include/cp_algo.h` (adds `CP_ALGO_ABACUS`), `src/common/cp_algo.cpp`
  (parse/name/supports), `src/common/main.cpp` (an early `--algo abacus` dispatch, since the shared
  parser rejects abacus flags), `CMakeLists.txt`.
- `src/abacus/cp_abacus.{cpp,cu}`: argument handling and a CUDA **mock** nonce search.

## Encoding v1 (matched the Abacus prototype byte-for-byte before ADR 0010)

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
prototype behaviour; a real pool needs per-client difficulty and stale handling. (Superseded by the
extranonce change below; per-miner accounting now exists in the node, ADR 0010. No payout exists.)

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

## GPU dataset generation and pool extranonce

- **GPU dataset**: `dataset_chain_kernel` builds the epoch dataset on the device (sequential
  data-dependent chain, matching the Rust `build_dataset`); used by the hard mock and A' solo. The
  host no longer builds it.
- **Pool extranonce**: each connection gets a distinct extranonce; the miner sets
  `nonce = (extranonce<<32) | counter`, so two miners do not walk the same nonce space. Two clients
  against one node: **52 + 39 = 91 accepted = node height** — no stale shares (previously many were
  lost to same-height collisions).
- **Device score (reverted)**: a single-thread device SHA-256 over `n^2*8` bytes was ~20x *slower*
  than the host score (269 vs 5075 attempts/s at n=64, measured), because it serialises the whole
  digest onto one GPU thread. It was reverted; the host score is not the bottleneck at these sizes.
  A *parallel* device hash (or a host score pipeline overlapped with the GPU) is the way to help
  larger `n`.

## Current rates (CMP 50HX, n=64)

| mode | attempts/s |
|---|---:|
| plain mock | 4820 |
| hard mock (device dataset) | 3757 |
| A' solo into node | 52+39 blocks / two clients |
| pool (2 clients) | 91 accepted, 0 stale |

## Not implemented

- **Pool mining**: one node serves several miners with per-miner extranonce ranges and per-miner
  accounting of accepted blocks; there is no share target below the block target, no difficulty
  negotiation and no payout.
- GPU-side instance expansion and score hashing (host-side today, which bounds the attempt rate at
  small `n`).
- The A' gather is wired into CPPminer (above) but not yet updated to encoding v2.

## Encoding v2 (2026-10-09, ADR 0010) — verified

Changes in `src/abacus/cp_abacus.{cu,cpp}`, `include/cp_abacus.h`: `bits` (u32 LE) in the preheader
between `timestamp` and `nonce` (mock and solo); data-dependent dataset reference
`ref(u) = LE64(blk[u-1][0..8]) mod u` on the host and in `dataset_chain_kernel`; `--dataset` is
explicitly a block count (`long long`); `--seg` removed (warns if passed); the A' mock reports
8 bytes read per gathered element.

End to end on the CMP 50HX against `abacus-node` at Abacus HEAD (`n = 64`, `bits = 4` start):

| mode | miner | node |
|---|---|---|
| solo, 10 s | jobs=97 found=96 | height 96; 96 accepted, 0 stale, 0 rejected |
| A' solo, dataset 8000 blocks, 10 s | jobs=91 found=90 | height 90; 90 accepted, 0 stale, 0 rejected |
| pool, two miners, 8 s | found 41 + 43 | height 84; accepted 41 + 43, stale 38 + 40, rejected 0 |

Every accepted block passed the node's v2 checks (committed `bits` equal to the retarget, MTP
timestamp, extranonce range, FS Freivalds), so the CUDA preheader, instance, score and A' dataset
match the Rust chain byte for byte. Difficulty rose during the runs (work 16128 for 96 blocks), i.e.
the enforced retarget reacted to fast blocks. In the pool every losing submission is stale (the other
miner won the height); none is invalid. The earlier "0 stale" pool figure counted only accepted
submissions. Script: `~/v2_e2e.sh` on the server.

## Next

1. Nothing on the critical path; CPPminer stays parked (CRITICAL-PATH §6).
2. GPU-side expand/score; larger `n`.
3. A' memory-hard gather in the CPPminer backend.
