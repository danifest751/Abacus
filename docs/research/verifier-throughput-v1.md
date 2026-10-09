# Verifier throughput v1 — candidate A (CPU, release)

Date: 2026-10-09. Bench: `crates/abacus-verifier/src/bin/abacus-verify-bench.rs`
(`cargo run --release --bin abacus-verify-bench`). Field: Goldilocks `P = 2**64 - 2**32 + 1` (an earlier draft said `2**61 - 1`; the code has always
used Goldilocks).
Challenge derivation: commit-then-expand (ADR 0006). CPU: desktop x86-64, release build.

Work = `n^3` scalar multiplications for the product; verification = one `O(n^2)` commitment hash plus
`k` Freivalds checks, each two `O(n^2)` matrix-vector products, so `~ 2k*n^2`.

| n | k | build s | verify s | verify / build |
|---:|---:|---:|---:|---:|
| 256 | 32 | 0.0882 | 0.0328 | 0.37 |
| 256 | 128 | 0.0880 | 0.1316 | 1.50 |
| 512 | 32 | 0.7068 | 0.1283 | 0.18 |
| 512 | 128 | 0.7073 | 0.4918 | 0.70 |
| 1024 | 32 | 5.6520 | 0.5153 | 0.09 |
| 1024 | 128 | 5.6380 | 1.9369 | 0.34 |

## Findings

- Verification is **cheaper than the work only when `n > 2k`** (ADR 0006). At `n=256, k=128`
  (`n = 2k`) the verifier costs 1.5x the work — unacceptable; at `n=512, k=128` it is 0.70x; at
  `n=1024, k=128` it is **0.34x** (verifier ~3x cheaper than the work).
- The earlier 714x blow-up was the naive Fiat–Shamir derivation (hashing the whole `C` per challenge
  element), fixed by commit-then-expand (ADR 0006).
- ~~A recommended parameter profile is therefore `k = 128` with `n >= 512`~~ **Superseded by ADR
  0009:** the per-challenge error is `<= 2^-63`, so `k = 2..3` suffices and the verifier costs
  `~4..6 n^2`; re-measured below (v1b). `n` and `k` are protocol parameters; the bench must be re-run for the chosen
  profile.

## v1b — re-run with `k = 2, 3` (ADR 0009)

Same machine and bench (`abacus-verify-bench`, release), challenges now bound to `(preheader, C)`.
Raw: `artifacts/verify-bench-20261009b.json` (not in Git).

| n | k | build s | verify s | verify / build |
|---:|---:|---:|---:|---:|
| 256 | 2 | 0.0878 | 0.0042 | 0.048 |
| 256 | 3 | 0.0878 | 0.0077 | 0.088 |
| 256 | 32 | 0.0889 | 0.0350 | 0.39 |
| 256 | 128 | 0.0872 | 0.1266 | 1.45 |
| 512 | 2 | 0.7105 | 0.0149 | 0.021 |
| 512 | 3 | 0.6985 | 0.0188 | 0.027 |
| 1024 | 2 | 5.6255 | 0.0634 | 0.011 |
| 1024 | 3 | 5.6239 | 0.0793 | 0.014 |
| 1024 | 128 | 5.6271 | 1.9951 | 0.35 |

With `k = 2` the verifier is ~21x cheaper than the (naive CPU) work already at `n = 256`, where
`k = 128` made it 1.45x **more** expensive. The verifier no longer forces large `n`.

## Caveats

- CPU release timings on one machine; the GPU path (miner) and multi-node verification are separate.
- This measures the verifier against a **naive `n^3`** product; a miner using Strassen (`n^2.807`)
  narrows the margin by ~`n^0.19` (see `omega_probe.py`). The `n > 2k` rule assumes `omega ~ 3`; for
  `omega ~ 2.807` the required `n` for a given margin grows modestly.

## Reproduce

```
cargo run --release --bin abacus-verify-bench
```
