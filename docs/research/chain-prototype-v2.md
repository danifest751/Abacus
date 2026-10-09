# Chain prototype v2 — candidate A local chain and multi-node sync (encoding v2)

Date: 2026-10-09. Status: **current**. Supersedes `local-prototype-v1` and `multi-node-testnet-v1`.
Crate: `crates/abacus-chain` (no third-party dependencies). This is the E6 gate of the research plan;
it is a test harness for the work function, not a network design.

## What it implements

- **Block**: preheader v2 (`"abacus/ph" || chain_id || version || prev || height || timestamp || bits
  || nonce`), the product `C` (`n^2` canonical residues), the score and the block id
  (`SHA-256("abacus/block" || preheader || C)`).
- **Validity** (`Chain::try_append`): height and prev link; `bits` equal to the ancestor-derived
  `next_bits()`; timestamp above the median of the last 11 blocks; `|C| = n^2` and every entry `< P`
  (checked before hashing); block id; score target; `k` Fiat–Shamir Freivalds challenges bound to
  `(preheader, C)`. Shape and canonicality are checked before anything is hashed (also for pool
  submissions). Every rejection reason (`Height`, `Prev`, `Bits`, `Timestamp`, `Id`, `Pow`) has a test.
- **Difficulty**: leading-zero bits of the score; every 16 blocks the time from the first to the last
  block of the window (15 intervals) is compared with `15 x 10 s`: +1 bit below half, -1 bit above
  twice; bits in `[1, 120]`.
- **Fork choice**: greatest cumulative work `sum 2^bits` over the **required** bits (`u128`).
- **P2P** (`p2p.rs`): pull sync of a peer snapshot, validated into a fresh chain without holding the
  node lock and adopted only if it has strictly more work; bounded line length, read timeouts, a
  64-connection cap, at most `2^20` blocks per snapshot, no synced block more than 120 s ahead of the
  node's clock; `--resync SEC`.
- **Solo/pool protocol**: `JOB` (template with real-time timestamp, at least `MTP + 1`) and `SUB`
  (nonce, timestamp, `C`). A submission must use the miner's range `nonce >> 32 == extranonce`, must
  not be more than 120 s in the future, and is counted **stale** if the tip moved since the
  connection's last `JOB`. Per-miner accepted/stale/rejected counters.
- **Optional A'**: with `--dataset N` the operands are gathered from a data-dependent epoch dataset
  of `N` 32-byte blocks, one 8-byte word per entry (spec/04 §3, prototype gather).

## Results (2026-10-09, release build, AMD Ryzen 7 8745HS)

Local mining with timestamps 2 s apart, 5x faster than the 10 s target (`abacus-mine 40`, `n = 8`,
`k = 8`):
all 40 blocks verified; bits 4 for heights 0–15, 5 for 16–31, 6 from 32 — the retarget raises the
difficulty at every window boundary as specified.

Three-process demo (unchanged commands, `abacus-node`):

```
A: --listen 9311 --mine 3 --bits 6 --serve 10              -> height 3, work 192
B: --listen 9312 --peer :9311 --mine 2 --bits 6 --serve 8 --resync 2
                                                           -> synced 3, mined 2: height 5, work 320
C: --listen 9313 --peer :9312 --serve 0                    -> synced 5: height 5, work 320, same tip as B
```

GPU miner end to end (CPPminer v2 against this node on the CMP 50HX, `n = 64`, start `bits = 4`;
`cppminer-backend-v2.md`):

| mode | node result |
|---|---|
| solo, 10 s | height 96; 96 accepted, 0 stale, 0 rejected |
| A' solo, 8000-block dataset, 10 s | height 94; 94 accepted, 0 stale, 0 rejected |
| pool, two miners, 8 s | height 84; accepted 52 + 32, stale 32 + 48 (not verified), rejected 0 |

Every accepted GPU block passed the full v2 validation, so the CUDA preheader, instance, score and
A' dataset match the Rust chain byte for byte (also covered by `tests/test_chain_parity.py` for
Python). In the pool every losing submission of a height race is counted as stale; stale submissions
are not verified, so "0 rejected" covers only the verified ones.

## Limits (by design of the laboratory)

- No transactions, mempool, rewards, signatures, gossip or peer discovery; a single fixed `chain_id`.
- The prototype profile (`n = 8..64`, `k = 8`) is for tests, not a parameter proposal.
- Sync downloads and re-validates the whole chain; there is no headers-first sync or checkpointing.
- No rate limiting or peer scoring; the bounds above only cap per-connection resource use.

## Reproduce

```
cargo run --release --bin abacus-mine 40
cargo run --release --bin abacus-node -- --listen 9311 --mine 3 --bits 6 --serve 10   # and B, C as above
python scripts/check.py                                                               # all tests
```
