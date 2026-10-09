# Local prototype v1 — candidate A (header-bound Freivalds matmul PoW)

> **Superseded in part (2026-10-09).** This note describes the first prototype. Since then: retarget,
> fork choice and P2P exist (`multi-node-testnet-v1.md`); the preheader also commits `timestamp` and
> `bits` (encoding v2), difficulty is enforced and timestamps follow median-time-past (ADR 0010). The
> output format below (`target0`) is historical.


Date: 2026-10-09. Crate: `crates/abacus-chain`. A **minimal local prototype** of the candidate A PoW:
mine and verify a chain of blocks. No P2P, mempool, transactions or coin.

## What it does

- **Block**: preheader (chain id, version, prev id, height, nonce) + product `C` + score; block id =
  `SHA-256(domain_block || preheader || encode(C))`.
- **PoW**: `seed = SHA-256(domain_instance || preheader)`; `A, B` expanded from the seed; `C = A*B`;
  `score = SHA-256(domain_score || preheader || encode(C))`; a block is valid when `score < target`
  and the `k` Fiat–Shamir-bound Freivalds challenges accept `C` (ADR 0004).
- **Chain**: append blocks; validate height, prev id, block id, score/target and Freivalds.
- **Optional memory-hard A'**: when a dataset is attached (`Chain::with_dataset`), `A, B` are
  **gathered** from the epoch dataset (spec/04) instead of expanded from the seed; the verifier must
  hold the dataset. The same block does not verify against the plain instance (tested).

## Prototype profile

`n = 8`, `k = 8`, target first byte `0x10` (≈ 1/16 attempts). Small for CPU speed; the parameters are
not a production profile.

## Result

```
$ cargo run --release --bin abacus-mine 5
{"profile": {"n": 8, "k": 8, "target0": 16}, "blocks": [
  {"height": 0, "nonce": 6,  "attempts": 7,  "seconds": 0.0002, "verified": true},
  {"height": 1, "nonce": 23, "attempts": 24, "seconds": 0.0005, "verified": true},
  {"height": 2, "nonce": 20, "attempts": 21, "seconds": 0.0004, "verified": true},
  {"height": 3, "nonce": 31, "attempts": 32, "seconds": 0.0006, "verified": true},
  {"height": 4, "nonce": 12, "attempts": 13, "seconds": 0.0002, "verified": true}
]}
```

Mined a 5-block local chain; every block verified (`verify` = score/target + Freivalds). Unit tests
(5) cover mine+verify, tampered `C`, wrong score, a bad-prev block, and determinism.

## Not included (by scope)

- **P2P / multi-node**, mempool, transactions, rewards, signatures — none.
- **Retarget**: difficulty is fixed in the prototype (the plan requires an ancestor-derived retarget;
  the work model is `(2^D/T) * c*n^omega`).
- **Fork choice**: single chain here (greatest-cumulative-work is specified, not exercised).
- **GPU miner**: only benches (`cuda/`), not a production miner.

## Next

1. Ancestor-derived retarget and a fork-choice test.
2. Add the candidate A' memory-hard gather to the prototype (small dataset for tests).
3. Then a genuine multi-node local testnet (separate processes), which is the E6 gate.
