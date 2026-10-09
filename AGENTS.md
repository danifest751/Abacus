# Abacus Network working rules

- Read `docs/CRITICAL-PATH.md` first. It names the single open decision (a portable, monotone,
  reuse-resistant work function over a GPU-optimal linear-algebra computation) and the decisive
  experiment sequence. Every task must map to it or be explicitly parked.
- English is primary for code, documentation, issues and commits.
- This is a research laboratory, not a production PoW or monetary blockchain. Distinguish
  mathematical validity, measured cost and security claims.
- Preserve source/configuration hashes, seeds and limitations with every experiment. Prefer raw
  data and uncertainty; keep failures and censored observations.
- Run `python scripts/check.py` locally before committing code changes.
- Do not introduce paid CI, cloud GPU jobs or GitHub Actions by default.
- Keep artifacts and indexes outside Git; commit compact reproducible research summaries.
- Never weaken a check or silently swap a consensus candidate for another task.
- Use "experimental Verifiable Algebra PoW"; make no security or usefulness claims without evidence.
