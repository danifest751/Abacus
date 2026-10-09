# Multi-node testnet v1 — candidate A (E6 gate)

Date: 2026-10-09. Bin: `crates/abacus-chain/src/bin/abacus-node.rs`. A minimal **multi-process** local
testnet: nodes listen, sync from a peer, mine, and serve their chain. No gossip, peer discovery,
mempool or coin.

## Protocol

Line protocol over TCP (no third-party crates). A server writes `B <hex>` for each block (oldest
first) then `E`. A client pulls all blocks, validates them into a fresh chain and **adopts it only if
it has strictly greater cumulative work**. A block is hex-encoded with a fixed layout (height, prev,
timestamp, nonce, bits, c-length, c words, score, id) and re-verified on receipt (id + score + target +
Freivalds).

## Demo (three processes)

```
node A: --listen 9101 --mine 3 --bits 6 --serve 8
node B: --listen 9102 --peer 127.0.0.1:9101 --mine 2 --bits 6 --serve 8
node C: --listen 9103 --peer 127.0.0.1:9102 --serve 0
```

Result:

```
A: {"height": 3, "work": 192, "tip": "1b4b..."}
B: {"height": 5, "work": 320, "tip": "0efa..."}   # synced A's 3, mined 2
C: {"height": 5, "work": 320, "tip": "0efa..."}   # synced B's 5, converged
```

Node B adopted A's chain (3) and extended it to 5; node C adopted B's 5 and converged to the same tip.
Two unit tests cover sync-convergence and ignoring a shorter chain.

## Scope

- **Pull-based** sync only (a client fetches a peer's chain); no gossip/push, no peer discovery.
- Single fixed `chain_id` and profile. `bits` travels inside each block but is **validated** against
  the ancestor-derived retarget and committed in the preheader (ADR 0010); before that fix a peer
  chain could carry any difficulty.
- No mempool, transactions, rewards or signatures.
- Genesis is an all-zero prev id; the first block is height 0.

## This is the E6 gate

Local nodes with two miner processes, sync and convergence pass — the E6 milestone from
`01-ABACUS-RESEARCH-PLAN.md`. E7 (external testnet on separate operators/machines/networks) is not
attempted; it also requires resource rules, transaction rules and independent review, none of which
exist.

## Hardening after the review (ADR 0010)

Bounded line reads, socket timeouts, a 64-connection cap, no lock held while serving or mining,
lock-free fetch/validate, periodic `--resync SEC`, median-time-past timestamps and a 120 s future
bound on submissions. The demo commands above are unchanged; the printed `work` is now an exact
integer and a `miners` array reports per-extranonce accepted / stale / rejected counts.

## Next

- Carry the memory-hard A' dataset across nodes (the dataset is deterministic from the epoch seed, so
  it can be rebuilt rather than transferred — but nodes currently sync only plain-A blocks).
- Reorg test with a genuinely higher-work competing branch and rollback.
