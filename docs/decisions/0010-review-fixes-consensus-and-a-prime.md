# ADR 0010 — Review fixes: consensus validation, encoding v2, A' dataset and gather fold

Status: accepted. Records the fixes for the 2026-10-09 code review of `bb6e532`. Every fix has a
regression test; nothing was weakened. The encoding changes are **breaking** for CPPminer
(`feat/abacus-backend`), which must be updated before it can mine against this node again.

## Consensus (`crates/abacus-chain`)

| Finding | Fix | Test |
|---|---|---|
| `append_checked` trusted the block's own `bits`: a peer chain could use any difficulty, and a miner could claim the achieved lead as work. | `bits` must equal the ancestor-derived `next_bits()`; cumulative work counts `2^bits` of the **required** target (`u128`, `MAX_BITS = 120`). | `rejects_block_below_required_difficulty`, `rejects_claimed_bits_above_required`, `sync_rejects_peer_chain_with_lowered_difficulty` |
| `bits` was not committed: the same block id could carry different work. | **Preheader v2** commits `bits` (between `timestamp` and `nonce`); the node uses `version = 2`. | `bits_are_committed_in_the_block_id` |
| A peer block with a wrong-length `C` panicked the verifier (remote crash); `C` entries `>= P` were not rejected (a second encoding of the same product = an extra score attempt). | Length and canonical-residue checks in `verify`, `verify_fs` and `freivalds_verify`, before hashing; exact-length `decode_block`. | `rejects_wrong_length_or_noncanonical_c_without_panicking`, `sync_rejects_peer_block_with_wrong_c_length_without_panicking`, `rejects_bad_shapes_without_panicking` |
| Timestamps were unvalidated; the pool took the miner's timestamp as is, so a miner could drive difficulty down. | Timestamp `> median of the last 11 blocks` (consensus); a submission may be at most `MAX_FUTURE_DRIFT = 120 s` ahead of the node clock; the node and `JOB` use real time. | `rejects_timestamp_not_above_median_time_past`, `pool_rejects_nonce_outside_extranonce_and_future_timestamp` |
| Fiat–Shamir root hashed only `C`, contrary to spec/03 and ADR 0006. | Root = `SHA256("abacus/check" \|\| preheader \|\| C)`. | `challenges_are_deterministic_shaped_and_bound_to_preheader`, `test_challenges_bind_the_preheader` |

## Network and pool (`p2p.rs`, `abacus-node`)

- Lines are read with a hard cap derived from the profile (`max_line(n)`); sockets have a read
  timeout; at most 64 concurrent connections; at most `2^20` blocks per snapshot.
- The chain lock is no longer held while writing a snapshot, nor while the node mines (it mines on a
  template and appends under the lock); peer chains are fetched and validated without the lock.
- `Chain::dataset` is an `Arc`, so validating a peer chain no longer copies the A' dataset.
- `--resync SEC` re-pulls the peer periodically (previously only at start-up).
- Per-miner accounting (`PoolStats`, keyed by extranonce; the old counter was global). A `SUB` must
  use the miner's own range `nonce >> 32 == extranonce`. "Accepted" means full blocks: there is no
  share target below the block target.

## Candidate A' (spec/04)

- **Data-dependent references.** `ref(u)` was `H(domain_ref || seed || u) mod u`, independent of the
  data (an iMHF-style fixed graph), although spec/04 and the code comments said "data-dependent".
  It is now `LE64(blk[u-1][0..8]) mod u` (Argon2d-style). No pebbling / time-memory trade-off
  analysis exists for either variant; `recompute_vs_store` is the naive no-checkpoint cost.
- **Storage is 8 bytes per block, not 32.** The gather consumes only the first 8 bytes of each
  block, so a miner needs `8 * N` bytes. Recorded in spec/04 and `memhard_dataset.storage_bytes`;
  using the whole block is an open design choice.
- **Linear segment fold collapses.** `cuda/attempt_bench.cu` v1 folded a segment by a plain `u64`
  sum, which two prefix-sum reads answer (`seg/8` -> 2 reads; `test_linear_segment_fold_collapses_with_prefix_sums`).
  The v1 "memory-hardness is achieved" result (`docs/research/attempt-rate-v1.md`) is **withdrawn**.
  v2 of the bench folds with a sequential nonlinear mix; it has **not** been compiled or measured
  (no `nvcc` on the review machine). The v1 gather was also one uncoalesced thread per entry, so its
  11.6–85 GB/s describe that kernel, not the hardware (warm cooperative reads reach ~416 GB/s).

## Parity and candidate B

- `reference/chain.py` is the single Python reference for every consensus derivation;
  `tests/test_chain_parity.py` compares it with the Rust `abacus-vectors` binary (preheader,
  instance, product, score, block id, FS challenges, dataset, gather indices, gathered instance,
  sumcheck transcript). Previously only `freivalds_verify` and `fs_challenges` were compared, and the
  Python A' indices (`abacus/idx`) and `mine_sim.py` (`abacus/matmul`) used different domains from
  Rust.
- **Sumcheck** accepted prover-chosen challenges, so any claimed sum passed with `r_j = 0`. Prove and
  verify now derive challenges from a running transcript hash (domain `abacus/sumcheck`).
  `rejects_zero_challenge_forgery` / `test_zero_challenge_forgery_is_rejected`.

## CPPminer compatibility (action required, external repo)

`JOB`/`SUB` wire formats are unchanged, but CPPminer must (1) add `bits` (u32 LE) to the preheader
between `timestamp` and `nonce`, (2) use `nonce = (extranonce << 32) | counter` (already the case),
(3) build the A' dataset with the data-dependent `ref(u)`, and (4) re-check its own A' gather fold:
if it is a linear sum it has the same prefix-sum shortcut. The `B <hex>` block line now carries
`bits` before `nonce`.
