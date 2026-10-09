# ADR 0010 — Review fixes: consensus validation, encoding v2, A' dataset and fold, sumcheck

Status: accepted (2026-10-09). Records the fixes for the first code review of `bb6e532`. Every fix has
a regression test; no check was weakened. Encoding v2 is incompatible with v1; CPPminer was updated
(`feat/abacus-backend` `289cd11`).

## Consensus (`crates/abacus-chain`)

| Finding | Fix | Regression test |
|---|---|---|
| `append_checked` trusted the block's own `bits`: a peer chain could use any difficulty, and a miner could claim the achieved score lead as work. | `bits` must equal the ancestor-derived `next_bits()`; cumulative work sums `2^bits` of the required target (`u128`, `MAX_BITS = 120`). | `rejects_block_below_required_difficulty`, `rejects_claimed_bits_above_required`, `sync_rejects_peer_chain_with_lowered_difficulty` |
| `bits` was not committed: the same block id could carry different work. | **Preheader v2** commits `bits` between `timestamp` and `nonce`; the node uses `version = 2`. | `bits_are_committed_in_the_block_id` |
| A peer block with a wrong-length `C` panicked the verifier (remote crash); `C` entries `>= P` were accepted (a second encoding of the same product is an extra score attempt without a new product, and a debug-build panic). | Shape and canonical-residue checks in `verify`, `verify_fs` and `freivalds_verify` before any hashing; exact-length `decode_block`. | `rejects_wrong_length_or_noncanonical_c_without_panicking`, `sync_rejects_peer_block_with_wrong_c_length_without_panicking`, `rejects_bad_shapes_without_panicking` |
| Timestamps were not validated and the pool used the miner's timestamp as is, so a miner could steer the retarget. | Timestamp above the median of the last 11 blocks (consensus); a submission at most 120 s ahead of the node clock; the node and `JOB` use real time. | `rejects_timestamp_not_above_median_time_past`, `pool_rejects_nonce_outside_extranonce_and_future_timestamp` |
| The Fiat–Shamir root hashed only `C`, unlike spec/03 and ADR 0006. | Root = `SHA256("abacus/check" \|\| preheader \|\| C)`. | `challenges_are_deterministic_shaped_and_bound_to_preheader`, `test_challenges_bind_the_preheader` |

## Network and pool (`p2p.rs`, `abacus-node`)

- Lines are read with a hard cap derived from the profile; sockets have read timeouts; at most 64
  concurrent connections; at most `2^20` blocks per snapshot.
- The chain lock is not held while writing a snapshot or while the node mines (it mines on a template
  and appends under the lock); peer chains are fetched and validated without the lock; the A'
  dataset is shared (`Arc`), not copied per sync; `--resync SEC` re-pulls the peer.
- Per-miner accounting keyed by extranonce: accepted, **stale** (the tip moved since the connection's
  last `JOB`; answered `BAD stale`, not verified) and rejected. A `SUB` must use
  `nonce >> 32 == extranonce`. The earlier pool result "0 stale" counted only accepted submissions.

## Candidate A'

- **Data-dependent references.** `ref(u)` was `H(domain_ref || seed || u) mod u` — independent of the
  data, although spec/04 and the code said "data-dependent". It is now `LE64(blk[u-1][0..8]) mod u`.
  No time–memory trade-off analysis exists for either variant.
- **Storage is 8 bytes per block.** The prototype gather consumes the first 8 bytes of each 32-byte
  block, so a miner needs `8 N` bytes.
- **A linear segment fold collapses.** The v1 attempt bench folded a gathered segment by a `u64`
  sum; a per-epoch prefix table answers it with two reads. Measured on the CMP 50HX against a
  warp-cooperative honest miner, the attacker is 2.2–53x faster and becomes matmul-bound
  (`gpu-suite-v1.md`). Any A' fold must be nonlinear; `attempt-rate-v1` is withdrawn.

## Parity and candidate B

- `reference/chain.py` is the single Python reference for every consensus derivation;
  `tests/test_chain_parity.py` compares it with the Rust `abacus-vectors` binary (preheader, instance,
  product, score, block id, challenges, dataset, gather indices, gathered instance, sumcheck
  transcript). Previously only Freivalds and the challenges were compared, and the Python A' indices
  and `mine_sim.py` used domains different from Rust.
- **Sumcheck** accepted prover-chosen challenges, so any claimed sum passed with `r_j = 0`. Prove and
  verify now derive the challenges from a running transcript hash (domain `abacus/sumcheck`)
  (`rejects_zero_challenge_forgery`, `test_zero_challenge_forgery_is_rejected`).

## CPPminer

`JOB`/`SUB` formats are unchanged. CPPminer puts `bits` in the preheader, builds the A' dataset with
the data-dependent reference (host and device), and treats `--dataset` as a block count; the unused
`--seg` option was removed. Its A' gather reads one word per entry and has no fold to attack. Verified
end to end against the node (`cppminer-backend-v2.md`).
