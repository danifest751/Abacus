# Candidate T v2 — frozen parameters, fast verifier, approximation and precomputation

Date: 2026-10-09. Status: **current**. spec/07, ADR 0016. Follows `tnet-v1`.

## Setup

- Rust reference `crates/abacus-chain/src/tnet.rs` (sha256 `e634c376…`): `EpochVerifier` keeps the
  epoch weights transposed so every output entry is one contiguous int8 dot product (vectorised by the
  compiler). `abacus-tnet bench` times it (`TNET_VERIFIER=reference` times the row-major reference).
  AMD Ryzen 7 8745HS, release build, baseline x86-64 or `RUSTFLAGS="-C target-cpu=native"`; median of 7.
- GPU: `cuda/tnet_bench.cu` (sha256 `0b010e05…`) on the CMP 50HX as in `tnet-v1`, 1 s warm-up, 3
  attempts.
- Robustness: `scripts/tnet_robustness_probe.py` (sha256 `7718f83d…`), numpy, uniform int8 weights,
  `n = 8192, L = 8`, 8 rows, seed 1. Raw: `artifacts/tnet-run2/` (not in Git).
- Python reference `reference/tnet.py`, differential test `tests/test_tnet_parity.py`.

## Verification on a CPU (`n = 8192, L = 8`)

| build | verifier | 1 thread | 8 threads |
|---|---|---:|---:|
| baseline x86-64 | row-major reference | 109.1 ms | 29.6 ms |
| baseline x86-64 | transposed | 81.8 ms | 17.6 ms |
| `target-cpu=native` | row-major reference | 46.1 ms | 15.5 ms |
| `target-cpu=native` | transposed | **16.8 ms** | **11.5 ms** |

At `n = 4096`: 4.8 ms (native, 1 thread). Per epoch: weight derivation 5.0 s (one thread, SHA-256
counter mode, parallelisable), transpose 2.1 s (tiled; 8.3 s untiled). A node should dispatch on CPU
features at run time; the baseline build lacks the AVX2 int8 dot product.

## GPU at the frozen parameters (`n = 8192, b = 65536, L = 8, w = 256, M = 2505`)

| attempt ms | GEMM | requant | expand | hash | tensor share | attempt TMAC/s | ns / ticket |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 611.6 | 539.6 | 41.2 | 20.1 | 10.7 | **88.2%** | 57.5 | 292 |

Lottery at 14 bits: 388 tickets against 384 expected. `X_L`: 0.7% zeros, 2.1% saturated, mean `|x|`
43. Parity: the GPU's sample ticket `(nonce 0, i 255, c 23)` is recomputed by `abacus-tnet check`:
identical 256 bytes, 15 leading zero bits.

## Approximation

Error propagation, one activation changed by ±1 after layer 1 (median of 8 rows):

| after layer | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---:|---:|---:|---:|---:|---:|---:|
| entries that differ | 1.0% | 7.4% | 21% | 35% | 44% | 50% | 55% |

A piece is 256 entries, so any error before the last layers destroys every ticket of the row.

Approximate last layer (256 pieces per variant):

| variant | exact entries | exact pieces |
|---|---:|---:|
| drop the 1 or 8 smallest terms (zeros) | 100% | 100% |
| drop the 64 smallest terms | 98.4% | 25.8% |
| drop the 512 smallest terms | 53.2% | 0% |
| int7 weights (`w` rounded to even) | 53.7% | 0% |

One requant step is 6,697 accumulator units against an accumulator spread of 372,000, so a piece is
exact only if the accumulators are exact to about `6697 / 256`; skipping exact zeros (0.7% of terms) is
the only free saving, and it is not available to tensor cores (their structured sparsity needs 50%).

## Precomputation on fixed epoch weights (bound, not measured)

For a fixed `W` (`n x n`) and int8 rows `x`:

- **Per-value tables** `T[k][v] = v W[k]`: `1024 n^2` bytes per layer (64 GiB at `n = 8192`); a row costs
  `n^2` int32 additions instead of `n^2` multiply-adds — no saving, and memory-bound. Tables over pairs
  of values need `2^16`x more.
- **Bit-plane tables** (T-MAC / LUT-GEMM style, group of `g` binary coordinates): `4 · 2^g / g · n^2`
  bytes per layer and `8 / g` int32 additions per original multiply-add. `g = 8`: 8 GiB per layer
  (64 GiB per epoch), one addition per multiply-add; `g = 16`: 1 TiB per layer, 0.5 addition. On the
  measured GPU (int8 tensor 65 TMAC/s; 56 SMs x 64 int32 lanes, 5.4–7.5 T additions/s at 1.5–2.1 GHz)
  this is 9–12x slower at `g = 8` and 4–6x at `g = 16` than the honest path from ALU throughput alone, before table bandwidth. In custom silicon it replaces
  an 8x8 multiplier by a 32-bit adder (a ≤2–3x area saving) at the cost of ≥128x more weight storage
  and bandwidth — the resource that bounds GEMM throughput. Published LUT methods target ≤4-bit
  weights on CPUs, not int8 x int8 on accelerators.

## Denial of service

Without the piece in the block, a forged header costs nothing to make and ~11 ms to reject. With the
piece (ADR 0016), the cheap check (one SHA-256) rejects it unless its author ground SHA-256 to the
target; peer banning handles the rest.

## Findings

1. A node verifies a TNet v1 block in ~11–17 ms on a laptop CPU with a SIMD build, after 7 s of
   per-epoch preparation and with 512 MiB of weights.
2. The GPU numbers of `tnet-v1` hold at the frozen parameters with `B = 65536` (88.2% tensor share).
3. No approximate computation yields valid tickets at useful rates; the network's rounding amplifies
   any error.
4. Precomputation is bounded analytically to a loss on GPUs and a storage-for-multiplier trade in
   silicon. A measured LUT kernel and an ASIC cost model remain open.

## Caveats

- One CPU, one GPU (Turing, sm_75). The probe uses numpy weights, not the consensus derivation (the
  statistics, not the bytes, matter there).
- The precomputation bound is a roofline argument; an optimised LUT kernel was not written.
