# Abacus Network — verifiable GPU-algebra proof of work

Private research into whether a **linear-algebra computation that is intrinsically GPU-optimal**
can support standalone proof of work. This repository currently implements a reference
verifier (Freivalds and sumcheck) and studies candidate PoW constructions. **There is no
currency, production network or GPU miner.**

**English is the primary repository language.**

## The one open question

Can a permissionless PoW be built from a linear, GPU-optimal computation (matmul, NTT, MSM)
with **cheap probabilistic verification**, while resisting decomposition, precomputation and
reuse? Linearity is the enemy, so a **nonlinear anchor** is the central research problem. A
negative result is a valid outcome.

## Implemented

- Exact Python reference verifier: `Freivalds` matrix-product verification over a prime field.
- Independent Rust verifier with explicit bounds and overflow analysis.
- Deterministic differential corpus (Python vs Rust) and adversarial tamper cases.
- One local check command (`python scripts/check.py`).

The ordinary check gate uses standard libraries only. Cargo builds offline without third-party
crates. GitHub Actions are intentionally absent; checks run locally.

## Run locally

Tested with Python 3.12.10 and Rust 1.98.1 on Windows.

```sh
python scripts/check.py
```

This runs formatting-independent Python tests and the Rust workspace tests, plus the
deterministic Python/Rust differential corpus. Experiments write to an ignored `artifacts/`.

## Documentation

- [Research plan](01-ABACUS-RESEARCH-PLAN.md) — objective, candidates, test plan, gates.
- [Critical path](docs/CRITICAL-PATH.md) — the single open decision and decisive experiments.
- [Status](docs/STATUS.md) — what is implemented.
- [Threat model](docs/THREAT-MODEL.md) — linearity, precomputation, reuse, resource attacks.
- [Language policy](docs/LANGUAGE.md).

## Scope boundary

Abacus is a research laboratory, not a production PoW or monetary blockchain. Freivalds and
sumcheck **verify a claimed result**, not that a miner did the work; that gap is the research.
Small-instance results do not establish GPU advantage, ASIC resistance, post-quantum security
or mainnet parameters. Use "experimental Verifiable Algebra PoW" until specific claims have
adequate evidence.

## Credits and license

MIT. See [LICENSE](LICENSE).
