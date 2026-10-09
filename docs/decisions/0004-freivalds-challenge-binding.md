# ADR 0004 — Freivalds challenges must be multiple and bound to the committed C

Status: accepted. This corrects the naive challenge in `spec/03-header-bound-freivalds-matmul-pow.md`.

## Context

Freivalds' check `A*(B*r) == C*r` has soundness `1/2` per independent random vector `r`. In the naive
candidate A, `r` was derived from the header (i.e. known **before** the miner chooses `C`). That is
forgeable.

## The flaw

If `r` is known before `C` is chosen, a cheater picks any `C'` with `C' r = (A*B) r`. Since `M = C' - A*B`
need only satisfy `M r = 0`, the solution set is an affine subspace of dimension `n*(n-1)` in the
`n x n` entries of `C`. The cheater then grinds `HASH(C')` in that subspace to beat the score target,
**without doing the matmul**. A single challenge also passes with probability `1/2` even for an
arbitrary wrong `C`. Freivalds is only meaningful for a prover who is **committed to `C` before `r`**,
with enough independent challenges.

## Decision

- Challenges are **multiple**: `k` independent vectors, `k` = the security parameter (use `k = 128`
  for `~2^-128` soundness).
- Challenges are **Fiat–Shamir bound to the committed C**: `r_i = HASH(domain_check || preheader ||
  encode(C) || i)`, `i = 1..k`. The miner fixes `C` first; `r_i` then depend on `C`, so the
  null-space construction above does not apply and a wrong `C` passes all `k` checks with probability
  at most `2^-k`.
- The verifier cost becomes `O(k * n^2)` field operations (two matrix-vector products per challenge),
  still below the `O(n^omega)` work for `n` well above `k`.
- `spec/03` is updated accordingly; the honest miner passes all `k` checks (completeness).

## Consequences

- The work model must use `k` as a protocol parameter; larger `n` (or smaller `k`) trades verifier
  throughput against soundness. This is a real design knob, not a free lunch.
- The reference and the Rust self-test exercise a **multi-challenge** Freivalds
  (`freivalds_verify_multi`), which already exists; the naive single-challenge path must not be used
  for the PoW.
- `scripts/freivalds_forgery_probe.py` demonstrates both the single-challenge forgery and that the
  Fiat–Shamir multi-challenge binding rejects it.
