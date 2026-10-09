# ABACUS-LAB-v1 — header-bound Freivalds matmul PoW (candidate A)

Status: candidate construction (research). No consensus is adopted; this records the design and its
open questions so the falsifiers in `docs/CRITICAL-PATH.md` can be tested against it.

## 1. Idea

Compute `C = A * B` and prove the result with **Freivalds' algorithm** (`O(n^2)` verification). Unlike
Pearl, **no noising or ZK is needed**: the instance `A, B` is derived from the block header, so a miner
cannot choose an easy instance. Header binding replaces Pearl's low-rank-noise permissionlessness.

## 2. Construction

```
preheader = encode(chain_id, version, profile, prev_block, height, tx_root,
                   timestamp, difficulty_descriptor, reward_commitment, nonce)
seed      = SHAKE256(domain_instance || preheader)
A, B      = expand(seed, profile)          # two dense n x n matrices over the field
C         = A * B                          # the work: n^omega field operations
root      = HASH(domain_check || preheader || encode(C))                  # commit (ADR 0006)
r_i       = expand(root)[i*n .. (i+1)*n], i = 0..k-1                       # k = 2..3 (ADR 0009)
accept    = ( all_i Freivalds_verify(A, B, C, r_i) )
            and ( HASH(domain_score || preheader || encode(C)) <= target )
```

The challenge must be **Fiat–Shamir bound to the committed `C`** (see ADR 0004); a single,
header-derived `r` is forgeable. Each challenge errs with probability `<= 2^-63` (ADR 0009).

**Prototype encoding (v2, `crates/abacus-chain`, `reference/chain.py`).** The implemented preheader is
`"abacus/ph" || chain_id(32) || version(u32) || prev(32) || height(u64) || timestamp(u64) ||
bits(u32) || nonce(u64)` (little-endian). `profile`, `tx_root` and `reward_commitment` are not
implemented. `bits` is the difficulty descriptor: it is ancestor-derived, must equal the retarget
value, and is committed so it cannot be altered without changing the instance and the block id
(ADR 0010). Timestamps must exceed the median of the previous 11 blocks. `C` must be exactly
`n^2` canonical residues.

- The miner submits `C` (n^2 field elements). The block also binds the preheader.
- Verification is `O(k n^2)`: for each of `k` challenges check `A*(B*r_i) == C*r_i`, then the score
  hash against `target`. Shape and canonicality are checked before any hashing.
- Difficulty: adjust `n` (profile) and/or `target`. Changing `A,B` requires a new header (new nonce).

## 3. Why header binding removes the noising

Pearl must accept *arbitrary* `A,B` (permissionless), so a miner could pick rank-deficient or sparse
matrices; noising breaks that linear structure. Here `A,B` come only from the header: a miner who
wants different matrices must change the header, which recomputes everything. Low-rank/sparse
screening is therefore excluded **by construction** (see `scripts/instance_probe.py`: random dense
matrices are full rank; a low-rank instance would be `n/(2r)` cheaper, but the miner cannot request
one). The verification challenge `r` is likewise header-derived and not miner-chosen.

## 4. Open questions (falsifiers to test)

1. **Algorithmic accounting.** The work is `n^omega`, not `n^3`. The work model must use the best
   known multiplication cost (`omega ~ 2.37` theory, `~2.81` Strassen practical), and difficulty must
   be monotone in it. See `scripts/omega_probe.py`.
2. **Decomposition / precompute.** For a single header there is one product; partial-product reuse
   across attempts is impossible because `A,B` change with the header. Confirm no cheaper route
   (e.g. batched products) exists within a header.
3. **Lottery structure.** `C` does not depend on a nonce, so each attempt is one full matmul and the
   only search is over headers. Confirm this yields a usable hashrate/difficulty scale and that the
   score hash cannot be ground independently of the matmul.
4. **Verification throughput.** Every submitted share is verified at `O(n^2)` per node; measure the
   network cost for the chosen `n`.
5. **Hardware asymmetry.** Dense matmul is **ASIC/FPGA-friendly** (tensor cores, systolic arrays);
   GPU advantage exists, but ASIC resistance does not. This is a known downside to record, not hide.
6. **Usefulness.** A random matmul is not a consumer computation; do **not** claim usefulness.

## 5. Scope

This is a research candidate, not a protocol. It specifies no signatures, reward schedule, mempool or
P2P. `C` is `n^2` field elements on the wire; canonical encoding and size bounds are required before
any networking (see `docs/THREAT-MODEL.md` sections 6 and 3).
