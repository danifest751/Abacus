# ADR 0009 — Freivalds soundness over `F_P` is `~1/P` per challenge, not `1/2`

Status: accepted correction. Corrects the soundness bound in ADR 0004 and the `n > 2k` constraint in
ADR 0006. Found in the 2026-10-09 code review.

## Context

ADR 0004 states that a wrong `C` passes one Freivalds challenge with probability `1/2`, so `k`
challenges give `2^-k`, and adopts `k = 128`. ADR 0006 then derives the verifier-cost constraint
`n > 2k` (so `n = 1024..2048`). The `1/2` bound is the classical one for challenges `r in {0,1}^n`.
Our challenges are **uniform over `F_P^n`** (Goldilocks, `P = 2^64 - 2^32 + 1`).

## The correct bound

Let `E = A*B - C != 0` and let `e` be a nonzero row of `E`. A challenge passes only if `e . r = 0`.
For `r` uniform over `F_P^n` this is a nonzero linear form vanishing at a random point:
`Pr = 1/P` (Schwartz–Zippel). Our derivation (`LE64 mod P`) is slightly non-uniform: `2^32 - 1`
residues have probability `2/2^64`, so the per-coordinate maximum is `2^-63` and the per-challenge
error is at most **`2^-63`**. With `k` independent Fiat–Shamir challenges bound to `(preheader, C)`,
a forger making `Q` hash queries succeeds with probability at most **`Q * 2^-63k`**.

| `k` | per-`C` error | forger with `Q = 2^80` queries |
|---:|---:|---:|
| 1 | `2^-63` | not sound |
| 2 | `2^-126` | `2^-46` |
| 3 | `2^-189` | `2^-109` |
| 128 (ADR 0004) | `2^-8064` | — (wildly over-provisioned) |

Evidence: `scripts/freivalds_soundness_probe.py` measures the per-challenge error over a small prime
(`p = 101`, worst-case `E` with one nonzero entry): field challenges err at `0.00975 ~ 1/p`, binary
challenges at `0.498 ~ 1/2` (`tests/test_soundness.py`).

## Decision

1. The soundness statement is **`<= 2^-63` per challenge, `<= Q * 2^-63k` for a `Q`-query
   forger**. ADR 0004's `2^-k` and `k = 128` are superseded.
2. The suggested profile is **`k = 2` (~126-bit per-`C`) or `k = 3`** for a margin against large
   grinding budgets. The prototype keeps `k = 8` (node) and `k = 4` (tests); nothing in the code is
   weakened by this ADR.
3. ADR 0006's constraint becomes `n > 2k` with `k = 2..3`, i.e. **effectively any `n >= 8`**: the
   verifier costs `~2k n^2 = 4..6 n^2` field operations plus one hash of `C`, against `~n^omega` for
   the work. Large `n` is no longer forced by the verifier; `n` is now chosen by the work model,
   block size (`8 n^2` bytes of `C`) and GPU efficiency.

## Consequences

- The `(n, D, k)` profile must be re-derived (`abacus-verify-bench` now also runs `k = 2, 3`).
- Block size drops sharply if smaller `n` is chosen (e.g. `n = 256`: 512 KiB of `C` instead of
  8–32 MiB at `n = 1024..2048`).
- The forgery analysis of ADR 0004 (single **header-derived** challenge is forgeable) is unaffected:
  that attack works because `r` is known before `C`, not because of the per-challenge error rate.
