# E1 v1 — winner-only STARK of the hash of `C` (A8 escape E1)

Date: 2026-10-09. Status: **current**. ADR 0013, escape E1.

## Question

E1 keeps the per-attempt cost low (int8 GEMM + a plain hash of `C`) and moves the proof to the winner:
a STARK that the hashed `C` equals `A * B`. Is the winner's proving time acceptable?

## Method

- Prover: Plonky3 (`github.com/Plonky3/Plonky3`, commit `eab7f0e3`), example `prove_hash_binary`
  (binary-field STARK, GF(2^128) commitments, folding PCS, 100-bit target), built with
  `--features parallel` and run with `RAYON_NUM_THREADS=8` on the `me4me` host (AMD Ryzen 5 5500,
  31 GiB RAM). CPU proving only; no GPU prover was available.
- Objectives: SHA-256 compressions and BLAKE3 compressions, trace length `2^16` (65,536 compressions =
  4 MiB of input). `2^18` is refused by the prover ("security level 100 exceeds the 98 bits the field
  leaves at this domain size"), so larger inputs need several proofs plus aggregation.
- A first single-threaded local build (Ryzen 7 8745HS) is reported for reference only.

## Results

| hash | compressions | prove s (8 threads) | verify s | proof | peak RSS | compressions/s |
|---|---:|---:|---:|---:|---:|---:|
| SHA-256 | 65,536 | 34.6 | 0.054 | 1.20 MB | 8.0 GB | 1,896 |
| BLAKE3 | 65,536 | 15.4 | 0.031 | 0.89 MB | 4.3 GB | 4,262 |

(single-threaded local run: SHA-256 158.9 s, BLAKE3 74.4 s.)

Extrapolated to the hash of `C` alone (64 bytes per compression, linear scaling, ignoring aggregation and
the in-circuit Freivalds/range checks):

| n | `C` | compressions | SHA-256, 8 CPU threads | BLAKE3, 8 CPU threads |
|---:|---:|---:|---:|---:|
| 4096 | 64 MiB | 1.05M | ~9 min | ~4 min |
| 8192 | 256 MiB | 4.19M | ~37 min | ~16 min |

## Findings

- On a CPU, E1's winner proof takes minutes to tens of minutes and tens of GB of memory before
  aggregation — not acceptable for a block interval of minutes.
- A GPU STARK prover would need to be ~10–50x faster to bring this to tens of seconds; that is plausible
  for production provers but **not measured** here. Even then the proving delay works like a propagation
  delay and favours large miners.
- E1 is therefore **conditionally viable at best** (GPU prover, block interval >= ~10 min) and is not
  pursued further without a measured GPU prover.

## Caveats

- Binary-field STARK for hash compressions only; the full E1 statement adds the Freivalds check over
  `C` and the binding of hashed bytes to field elements.
- Linear extrapolation from one size; aggregation of many `2^16` proofs is not included.
