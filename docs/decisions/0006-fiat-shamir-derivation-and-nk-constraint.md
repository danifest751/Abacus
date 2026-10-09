# ADR 0006 — Fiat–Shamir derivation: commit-then-expand, and the n > 2k constraint

Status: accepted. Corrects the challenge derivation and records a parameter constraint for candidate A.
Amended by ADR 0009: the `n > 2k` constraint stands, but with `k = 2..3` it no longer forces large
`n`. The commit-then-expand derivation stands.

## Context

ADR 0004 requires `k` Fiat–Shamir challenges bound to the committed `C`. The first implementation
derived each of the `k*n` challenge elements by hashing the whole `C` (an `O(n^2)`-byte buffer) every
time, making derivation `O(k*n^3)` **bytes**. The verifier-throughput bench exposed it: at `n=256`,
`k=128` verification took **62.8 s** versus **0.088 s** to compute the product — a 714x overhead that
destroys the "cheap verification" premise.

## Decision

1. **Commit-then-expand.** Hash `C` **once** to a root, `root = HASH(domain_check || preheader ||
   encode(C))` (`O(n^2)` bytes), then expand the `k*n` field elements from `root` with a cheap counter
   mode (`HASH(root || counter)` yielding four elements per hash). Derivation is then
   `O(n^2 + k*n)` — cheap. Implemented in Python and Rust; parity kept
   (`scripts/freivalds_forgery_probe.py`, `crates/abacus-verifier/src/freivalds_fs.rs`).
2. **Parameter constraint `n > 2k`.** Verification is one commit `O(n^2)` plus `k` challenge checks,
   each two matrix-vector products `O(n^2)`: `verify ~ 2k*n^2`. Work is `~ n^omega` (`omega ~ 3` for a
   naive product). Verification is cheaper than the work only when **`n > 2k`** (for `omega = 3`);
   choose `n` comfortably larger (e.g. `n = 1024..2048` with `k = 128`, a 4-8x margin).

## Measured (CPU, release; `crates/abacus-verifier/src/bin/abacus-verify-bench.rs`)

| n | k | build s | verify s | verify/build |
|---:|---:|---:|---:|---:|
| 128 | 32 | 0.0120 | 0.0085 | 0.71 |
| 128 | 128 | 0.0110 | 0.0316 | 2.87 |
| 256 | 32 | 0.0882 | 0.0330 | 0.37 |
| 256 | 128 | 0.0886 | 0.1298 | 1.47 |

The `n=128, k=128` row confirms the constraint (verifier 2.87x the work — unacceptable); larger `n`
turns it into an advantage.

## Consequences

- The parameter profile is `(n, D, k)` with the hard constraint `n > 2k`; the verifier-throughput
  bench must be re-run for any chosen profile.
- The forgery probe still holds (single-challenge forgeable; FS rejects; honest accepts).
- The work model of ADR 0005 is unchanged; only the challenge derivation and the `n > 2k` constraint
  are added.
