# GPU suite v1 — matmul, gathered reads and A' attempt folds (CMP 50HX)

Date: 2026-10-09. Status: **current**. Supersedes `gpu-baseline-v1`, `gather-bandwidth-v1`,
`attempt-rate-v1` and `attempt-rate-v2`.

## Environment and method

- Lab GPU host: NVIDIA CMP 50HX (Turing, sm_75, 20 GiB), driver 610.43.03, CUDA 13.3. During the
  runs: SM 1950–1995 MHz, memory 7000 MHz, 87–229 W, 39–59 °C (sampled after each bench).
- Command: `bash scripts/gpu_suite.sh` builds the three benches (`nvcc -O3 -arch=sm_75`) and runs them.
  Every bench keeps the GPU busy for 3 s before timing (the `cmp-idle-governor` otherwise under-clocks
  the first launches 4–20x) and reports the **median of 7 CUDA-event timings** (min/max in the raw
  data); for the attempt bench each timing is a batch of 16–64 attempts.
- The suite was run **twice back to back** (runs 3 and 4) with identical sources. Tables show run 3;
  run 4 differs by at most 3.6% (matmul), 2.0% (gather) and 2.5% (attempt rate).
- Source hashes (sha256): `attempt_bench.cu e9bcbe6a…`, `gather_bench.cu ce6ecdd7…`,
  `goldilocks_matmul_bench.cu f7112e0e…`, `gpu_suite.sh 7d86b782…`. Raw JSON lines:
  `artifacts/gpu-run3/`, `artifacts/gpu-run4/` (not in Git).

## 1. Goldilocks matmul (naive 16x16 tiled kernel)

| n | GPU GMAC/s (run 3 / run 4) | CPU GMAC/s (naive, 1 thread) | GPU / CPU | exact match |
|---:|---:|---:|---:|---|
| 256 | 166.7 / 162.7 | 0.153 | 1091x | yes |
| 512 | 178.8 / 174.1 | 0.151 | 1185x | yes |
| 1024 | 175.2 / 169.0 | — | — | — |
| 2048 | 177.0 / 172.2 | — | — | — |

The kernel sustains **~175 GMAC/s** of 64-bit modular multiply-add for `n >= 512` (163–167 at
`n = 256`). Tensor cores do not apply to this arithmetic. The CPU column is a naive single-threaded
loop on the same host and serves only as the exactness check; it is **not** a matched D5 comparison
(no multi-threaded or tuned CPU kernel, no energy measurement).

## 2. Random gathered reads by segment size (2 GiB buffer)

| segment | threads per segment | gather GB/s | of sequential | segments/s |
|---:|---:|---:|---:|---:|
| 8 B | 1 | 25.0 | 4.6% | 3.12e9 |
| 32 B | 4 | 99.2 | 18.4% | 3.10e9 |
| 256 B | 32 | 477.0 | 88.6% | 1.86e9 |
| 2560 B | 32 | 527.3 | 97.9% | 2.06e8 |
| 8 KiB | 32 | 530.7 | 98.4% | 6.5e7 |
| 64 KiB | 32 | 532.6 | 98.8% | 8.1e6 |

Sequential stream: 538–539 GB/s. Small reads are bound by the **access rate (~3.1e9 random reads/s)**,
not by bandwidth; from 256 B a warp-cooperative read reaches 87–99% of the measured sequential rate.

## 3. A' attempt: gather fold vs matmul

One attempt gathers the `n^2` entries of **`A` only** (each folded from a random `seg`-byte segment of
a 2 GiB pseudo-random dataset; `B` is a constant fill) and computes the `n x n` product. Folds:

- **mix**: sequential nonlinear fold, one thread per entry (naive honest miner);
- **sum**: linear `u64` sum, one thread per entry (the v1 design);
- **prefix**: the attack on `sum` — a per-epoch prefix table answers any segment sum with two reads;
- **coop**: nonlinear fold with one warp per entry, coalesced reads, per-lane nonlinear chains and a
  nonlinear butterfly combine (tuned honest miner; not prefix-decomposable).

| n | seg | fold | attempts/s | gather ms | matmul ms | gather / matmul | read MB | read GB/s |
|---:|---:|---|---:|---:|---:|---:|---:|---:|
| 256 | 2560 | mix | 354.4 | 2.713 | 0.094 | 28.8 | 167.8 | 61.8 |
| 256 | 2560 | sum | 352.3 | 2.727 | 0.094 | 28.9 | 167.8 | 61.5 |
| 256 | 2560 | prefix | 6727.8 | 0.050 | 0.096 | 0.52 | 1.0 | 21.1 |
| 256 | 2560 | **coop** | **2321.3** | 0.326 | 0.102 | **3.19** | 167.8 | 515.0 |
| 256 | 65536 | mix | 14.5 | 69.20 | 0.095 | 729 | 4295 | 62.1 |
| 256 | 65536 | sum | 14.1 | 70.67 | 0.095 | 743 | 4295 | 60.8 |
| 256 | 65536 | prefix | 6697.4 | 0.050 | 0.097 | 0.51 | 1.0 | 21.1 |
| 256 | 65536 | **coop** | **126.0** | 7.832 | 0.099 | **79.4** | 4295 | 548.4 |
| 512 | 2560 | mix | 77.6 | 12.37 | 0.688 | 18.0 | 671.1 | 54.3 |
| 512 | 2560 | sum | 79.8 | 12.09 | 0.688 | 17.6 | 671.1 | 55.5 |
| 512 | 2560 | prefix | 1061.7 | 0.187 | 0.747 | 0.25 | 4.2 | 22.4 |
| 512 | 2560 | **coop** | **488.2** | 1.289 | 0.749 | **1.72** | 671.1 | 520.7 |

## Findings

1. **A linear fold is broken.** Against the best honest miner (`coop`), the prefix-sum attacker is
   **2.9x** (`n=256, seg=2560`), **53x** (`n=256, seg=64 KiB`) and **2.2x** (`n=512, seg=2560`) faster,
   and its attempt becomes matmul-bound. Any A' fold must be nonlinear (ADR 0010).
2. **With a nonlinear fold and large segments, a tuned miner is bandwidth-bound.** `coop` reads at
   515–548 GB/s, about the measured sequential rate (96–102%), and gathering `A` alone takes 1.7–79x
   the matmul time; per attempt it reads 168 MB (`n=256, seg=2560`). Gathering `B` as well doubles the
   gather (`memhard-balance-v2`).
3. **The prototype A' (one 8-byte word per entry) is compute-bound** for `n >= 256`: `2 n^2` random
   reads at ~3.1e9/s take 0.45x (n=256) to 0.11x (n=1024) of the matmul (`memhard-balance-v2`).
4. The naive one-thread-per-entry gather (`mix`, `sum`) reaches only ~54–63 GB/s; it overstates the
   gather share ~9x and must not be used to argue memory-hardness.
5. The model of `memhard-balance-v2`, restricted to `A`, predicts 3.34 at `n=256, seg=2560` (measured
   3.19) and 1.67 at `n=512` (measured 1.72).

## Caveats

- One GPU model. The ratios move with a device's bandwidth/compute balance; an HBM GPU or an ASIC with
  a different balance changes them.
- `B` is a constant fill and the attempt loop has no instance expansion and no score hash; only gather
  and matmul are timed.
- The segment-fold design exists only in this bench; the prototype chain and CPPminer implement the
  one-word gather.
- No energy and no tuned CPU: not a D5 result.

## Reproduce

```
bash scripts/gpu_suite.sh            # on the CUDA host; writes artifacts/gpu-<UTC time>/
```
