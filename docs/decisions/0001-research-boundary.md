# ADR 0001 — Research boundary

Status: accepted.

## Context

Abacus investigates whether a linear, GPU-optimal computation can support standalone proof of
work. It is easy for such a project to drift into consensus, tokens or "useful AI" marketing
before the central question is answered.

## Decision

- Abacus is a **research laboratory**. There is no currency, production network, GPU miner or
  consensus, and none is added until the work-function gate in `docs/CRITICAL-PATH.md` passes.
- The single objective is the open question in `docs/CRITICAL-PATH.md` §1: a permissionless PoW
  from a linear GPU-optimal computation with cheap probabilistic verification, resistant to
  decomposition, precomputation and reuse.
- A negative result is a valid completion and must be published, not hidden.
- The work gate is decided by evidence and one external cryptographic review, not by volume of
  code, tests or documents.

## Consequences

- Experiments are bounded and timeboxed with explicit stop criteria.
- Verifiers and reference code stay independent (Python and Rust) and dependency-free in the
  ordinary gate.
- No self-reported work, device counters or output size becomes a work metric.
- "Experimental Verifiable Algebra PoW" is used until specific claims have evidence.
