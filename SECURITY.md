# Security policy

## Scope

Abacus is a **research laboratory**. There is **no production network, no coin and no deployed
system**; the chain prototype and the CPPminer backend are test harnesses. Nothing here carries a
security or usefulness guarantee. Do not use this
code where security, consensus or funds depend on it.

## What is in scope for a report

- Correctness or soundness bugs in the verifier (`reference/`, `crates/abacus-verifier/`), e.g. a
  flawed Freivalds acceptance rule, an overflow, or an encoding that admits a non-canonical case.
- A concrete **work-model break**: a cheap way to obtain inflated weight from a stated candidate
  work function (decomposition, precomputation, reuse, screening, outsourced-work claim).
- Consensus bugs in the chain prototype (`crates/abacus-chain`): inflated weight, accepted invalid
  blocks, crashes or unbounded resource use from peer or miner input.
- Resource-exhaustion in the command-line adapters (unbounded parsing/allocations).

## What is out of scope

- Any claim about a mainnet, coin value, ASIC resistance, post-quantum security, or "useful work"
  — none are established.
- Denial-of-service on infrastructure that does not exist.

## How to report

- Preferred: open a **private** security advisory via GitHub
  (`Security` -> `Report a vulnerability`) on `danifest751/Abacus`.
- Alternatively open a regular issue if the finding is not sensitive.
- Include: affected file/commit, minimal reproduction, expected vs observed, and any raw evidence.

## Handling

- Findings are triaged against the scope above and recorded in the repository research record
  (report + decision where relevant). Because the project is pre-consensus, most work-model breaks
  are published as research results rather than embargoed.
