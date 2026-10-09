# CPPminer backend v2 — `--algo abacus` on CUDA (encoding v2)

Date: 2026-10-09. Status: **current**. Supersedes `cppminer-backend-v1`. Repo `danifest751/CPPminer`,
branch `feat/abacus-backend`, commits `289cd11` (encoding v2) and `d98139b` (A' mock timing fix).

## Scope

The Abacus work function as a CPPminer algorithm, to exercise the chain prototype with a real GPU
miner. It is a laboratory tool, not a production miner.

- `src/abacus/cp_abacus.cu`: host and device SHA-256, instance expansion on the device, the A'
  dataset (device, one thread, sequential) and gather, the Goldilocks matmul kernel, the host score
  hash, a mock loop, an A' mock loop and a solo/pool client (`JOB`/`SUB`).
- `src/abacus/cp_abacus.cpp`: argument handling.

## Encoding (byte-identical to `crates/abacus-chain`)

```
preheader = "abacus/ph" || chain_id(32) || version(u32) || prev(32) || height(u64) || timestamp(u64)
            || bits(u32) || nonce(u64)                                   (little-endian)
seed      = SHA256("abacus/instance" || preheader)
A, B      = expand(seed, 2 n^2)          SHA256("abacus/expand" || seed || ctr), 4 x (LE64 mod P) per hash
C         = A * B over Goldilocks
score     = SHA256("abacus/score" || preheader || C)                     accept: leading zero bits >= bits
A'        = blk[u] = SHA256("abacus/ds" || seed || u || blk[u-1] || blk[ref(u)]),
            ref(u) = LE64(blk[u-1][0..8]) mod u; entry = LE64(blk[idx][0..8]) mod P
nonce     = (extranonce << 32) | counter
```

`--dataset N` is a count of 32-byte blocks. The kernel reads one 8-byte word per entry; there is no
segment fold, so the prefix-sum attack of ADR 0010 does not apply to this miner.

## Mock rates (CMP 50HX, warm, three 6 s runs each, spread < 2%)

| mode | n | attempts/s |
|---|---:|---:|
| mock | 64 | 5012–5092 |
| mock | 128 | 1483–1494 |
| mock | 256 | 365–366 |
| A' mock, 8000 blocks | 64 | 5028–5040 (dataset build 0.04 s) |
| A' mock, 1,048,576 blocks | 64 | 5004–5014 (dataset build 5.93 s) |

At these sizes the rate is **host-bound**: `C` is copied back and the score hash runs on the CPU for
every attempt (the matmul itself takes ~0.1 ms at `n = 256`, `gpu-suite-v1.md`). A single-thread
device score hash was tried earlier and was ~20x slower; a parallel device hash is not implemented.
The sequential dataset build runs at ~177k blocks/s on one GPU thread, so a 1 GiB dataset (33.5M
blocks) would take ~3 minutes per epoch.

## End to end against the node (`chain-prototype-v2.md`)

| mode | miner | node |
|---|---|---|
| solo, 10 s | jobs 97, found 96 | 96 accepted, 0 stale, 0 rejected |
| A' solo, 8000 blocks, 10 s | jobs 95, found 94 | 94 accepted, 0 stale, 0 rejected |
| pool, two miners, 8 s | found 52 + 32 | accepted 52 + 32, stale 32 + 48 (not verified), rejected 0 |

Self-test (`--selftest`): SHA-256 known vectors and host/device expansion parity pass.

## Reproduce (on the CUDA host)

```
cppminer --algo abacus --backend cuda -d 0 --mock --n 64 --bits 4 --seconds 6
cppminer --algo abacus --backend cuda -d 0 --mock --n 64 --bits 4 --dataset 1048576 --seconds 6
abacus-node --listen 9401 --n 64 --bits 4 --serve 16 &
cppminer --algo abacus --backend cuda -d 0 --node 127.0.0.1:9401 --n 64 --seconds 10
```
