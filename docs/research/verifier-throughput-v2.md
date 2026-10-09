# Verifier throughput v2 — candidate A (CPU, release)

Date: 2026-10-09. Status: **current**. Supersedes `verifier-throughput-v1`.

## Method

- CPU: AMD Ryzen 7 8745HS, Windows 10, Rust 1.98.1, release build, one thread. Every timing is the
  **median of 7** repetitions (min/max in the raw data).
- `abacus-verify-bench` (crate `abacus-verifier`): `verify_fs` alone — hash of `C`, challenge
  expansion, `k` Freivalds checks — against one naive `n^3` CPU product. Raw:
  `artifacts/verify-bench-v2.json`.
- `abacus-block-bench` (crate `abacus-chain`): the **complete block check** a node runs — shape and
  canonicality, score hash, instance expansion from the preheader (`n^2 / 2` SHA-256 calls), challenge
  derivation, `k` Freivalds checks — and the instance expansion alone. Raw: `artifacts/block-bench.json`.
- Challenges are bound to `(preheader, C)` (ADR 0006, 0010).

## Freivalds part (`verify_fs`)

| n | k | product s | verify s | verify / product |
|---:|---:|---:|---:|---:|
| 256 | 2 | 0.0890 | 0.0041 | 0.046 |
| 256 | 3 | 0.0890 | 0.0052 | 0.058 |
| 256 | 32 | 0.0890 | 0.0329 | 0.370 |
| 256 | 128 | 0.0890 | 0.1263 | 1.418 |
| 512 | 2 | 0.7042 | 0.0152 | 0.022 |
| 512 | 3 | 0.7042 | 0.0192 | 0.027 |
| 512 | 32 | 0.7042 | 0.1305 | 0.185 |
| 512 | 128 | 0.7042 | 0.5004 | 0.711 |
| 1024 | 2 | 5.6326 | 0.0608 | 0.011 |
| 1024 | 3 | 5.6326 | 0.0763 | 0.014 |
| 1024 | 32 | 5.6326 | 0.5243 | 0.093 |
| 1024 | 128 | 5.6326 | 1.9980 | 0.355 |

## Complete block check

| n | k | instance expansion s | full block check s |
|---:|---:|---:|---:|
| 64 | 2 | 0.0008 | 0.0012 |
| 64 | 3 | 0.0008 | 0.0013 |
| 256 | 2 | 0.0131 | 0.0191 |
| 256 | 3 | 0.0131 | 0.0202 |
| 512 | 2 | 0.0525 | 0.0757 |
| 512 | 3 | 0.0525 | 0.0797 |

## Findings

- With the bound of ADR 0009, `k = 2` (~`2^-126` per `C`) suffices; the Freivalds part then costs 2–5%
  of a naive CPU product (`k = 3`: 3–6%). The earlier `k = 128` profile made it 1.4x *more* expensive
  than the product at `n = 256`.
- **The complete block check is dominated by instance expansion**, not by Freivalds: at `n = 256` it
  takes 19 ms, of which 13 ms is expanding `A, B` from the seed with the in-house SHA-256. That is
  ~21% of a naive single-threaded CPU product, and ~190 GPU products (0.1 ms each, `gpu-suite-v1`).
- Verification cost is bounded and linear in `n^2`, so it is not a DoS vector at these sizes, but
  "cheap verification" means "~5x cheaper than a naive CPU product", not "negligible". A faster
  expansion (hardware SHA-256 or a cheaper PRF) is the obvious lever.

## Caveats

- One CPU, single thread, unoptimised SHA-256 and matrix-vector loops; a tuned verifier is faster and a
  tuned CPU product much faster.
- Block size is `8 n^2` bytes of `C` (512 KiB at `n = 256`), received and hashed by every node; the
  network cost is not measured here.
