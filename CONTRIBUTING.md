# Contributing

Thanks for looking. Abacus is a research laboratory; contributions that sharpen the **critical
path** are the most valuable.

## Ground rules

- **Read `docs/CRITICAL-PATH.md` first.** Every change should advance the single open decision (a
  portable, monotone, reuse-resistant work function over a GPU-optimal linear computation) or be
  explicitly parked.
- **English** is the primary language for code, docs, issues and commits. Russian is secondary,
  under `docs/ru/`, clearly labelled.
- This is **not** a coin project. Do not add wallets, consensus, mining or monetary framing.
- **Falsification first.** Prefer experiments that break a candidate work function to ones that
  confirm it. Keep failures and censored observations.

## Before you commit

Run the local gate and make sure it passes:

```sh
python scripts/check.py
```

This runs the Python tests (including the Python/Rust differential corpus), the Rust workspace
tests and the self-test binary.

## Style and layout

- Rust: `cargo fmt` and `cargo clippy` clean; no third-party crates in the ordinary gate.
- Python: standard library only in the gate; keep the reference implementation transparent.
- Keep research summaries compact; put raw output under an ignored `artifacts/`, not in Git.
- No paid CI, cloud GPU jobs or GitHub Actions by default.

## Commits, issues and PRs

- Commit messages: imperative subject, short body explaining the decision value.
- Open an issue before a large experiment; state the hypothesis, the falsifier, the budget and the
  stop criterion.
- PRs should reference the critical-path item they advance and include the gate result.
- Preserve source/config hashes and seeds with any experiment.

## Repository hygiene (avoid GitHub quotas)

The repository stays **private** for now and must not consume GitHub metered features:

- **No GitHub Actions / workflows** (no CI minutes). Checks run locally via `python scripts/check.py`.
- **No Git LFS** (no LFS storage/bandwidth). Keep binaries out of Git; `.gitignore` excludes CUDA
  artifacts, build output and research artifacts.
- **No Packages/registry, releases or cloud jobs** by default.
- Commit compact, reproducible **summaries** with hashes/seeds; keep raw data and large results
  under an ignored `artifacts/`.

## Reporting security issues

See [SECURITY.md](SECURITY.md). Use private vulnerability reporting for sensitive findings.
