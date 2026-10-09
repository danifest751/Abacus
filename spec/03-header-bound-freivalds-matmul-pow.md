# ABACUS-LAB-v1 — header-bound Freivalds matmul PoW (candidate A)

Status: current candidate construction (revised 2026-10-09). Research design with a prototype
implementation (`crates/abacus-chain`, encoding v2); no parameters are adopted.

## 1. Idea

The work of an attempt is a dense matrix product `C = A * B` over Goldilocks. The instance `A, B` is
derived from the block preheader, so the miner cannot choose an easy instance, and the product is
verified with Fiat–Shamir-bound Freivalds challenges in `O(k n^2)`. Unlike Pearl (which accepts
external matrices and needs low-rank noise), nothing about the instance is miner-controlled except
through the preheader, which re-randomises the whole instance.

## 2. Construction

```
preheader = "abacus/ph" || chain_id(32) || version(u32) || prev(32) || height(u64)
            || timestamp(u64) || bits(u32) || nonce(u64)                    (little-endian)
seed      = SHA256("abacus/instance" || preheader)
A, B      = expand(seed, 2 n^2)       # SHA256("abacus/expand" || seed || LE32(i)), 4 x (LE64 mod P) per hash
C         = A * B                     # the work: n^omega field operations
score     = SHA256("abacus/score" || preheader || C)
root      = SHA256("abacus/check" || preheader || C)
r_i       = expand_root(root)[i*n .. (i+1)*n], i = 0..k-1     # SHA256(root || LE32(j)), 4 elements per hash
accept    = |C| = n^2, all C_ij < P
            and leading_zero_bits(score) >= bits
            and all_i A*(B*r_i) == C*r_i
block_id  = SHA256("abacus/block" || preheader || C)
```

`bits` is the difficulty descriptor. It is ancestor-derived and must equal the retarget value
(`next_bits`, §4); committing it in the preheader means it cannot be changed without changing the
instance, the score and the block id. The prototype does not implement `profile`, `tx_root` or
`reward_commitment` fields; a real header would commit them the same way.

## 3. Properties

- **No screening.** `A, B` are uniform and, with overwhelming probability, full rank; the only way to
  obtain a different instance is a different preheader, which costs a full product. A low-rank
  instance would be `n / 2r` cheaper, but the miner cannot request one (`scripts/instance_probe.py`).
- **No reuse across attempts.** Every attempt has fresh `A` and `B`; there is no shared operand to
  amortise. Within an attempt the score needs every entry of `C`.
- **Soundness.** A wrong `C` passes with probability at most `Q * 2^-63k` for a `Q`-query forger
  (spec/01 §2, ADR 0009). Suggested `k = 2..3`; the prototype uses `k = 8` (node) and `k = 4` (tests).
- **Work.** One attempt costs `c * n^omega` (best known algorithm, not `n^3`; `scripts/omega_probe.py`);
  expected work per block is `2^bits` attempts. A uniform algorithmic constant is absorbed by the
  retarget.
- **Verification.** `n^2 / 2` SHA-256 calls to expand the instance, two hashes of `8 n^2` bytes, and
  `2 k n^2` multiply-adds: ~19 ms at `n = 256`, `k = 2` on one CPU core, of which ~13 ms is the
  instance expansion (`docs/research/verifier-throughput-v2.md`).
- **Block size.** `C` is `8 n^2` bytes (512 KiB at `n = 256`, 8 MiB at `n = 1024`). This is the main
  cost of the construction for a network and is not reduced here.

## 4. Prototype chain rules (`crates/abacus-chain`)

- Timestamps must exceed the median of the previous 11 blocks (consensus); a node accepts neither a
  submitted nor a synced block more than 120 s ahead of its clock (local policy).
- Retarget every 16 blocks: the time from the first to the last block of the window (15 intervals)
  is compared with `15 x 10 s`; +1 bit below half, -1 bit above twice; bits in `[1, 120]`.
- Fork choice: greatest `sum 2^bits` over the required bits.
- Validation order: link and difficulty, timestamp, shape and canonicality of `C`, block id, score,
  Freivalds — cheap checks first, nothing hashed before the length check.

## 5. Open questions

1. Parameter profile `(n, bits, k)` under ADR 0009 and the block-size cost.
2. Hardware: dense matmul is ASIC-friendly; the GPU advantage is a constant factor that a dedicated
   64-bit modular MAC array would erase (ADR 0005, 0007).
3. Usefulness: none — the matrices are random.
4. Proof size: committing to `C` (e.g. a polynomial commitment with a sumcheck-based matmul proof)
   instead of shipping it would shrink blocks; not studied.

## Revision history

- 2026-10-09: encoding v2 (`bits` and `timestamp` committed, difficulty enforced, MTP timestamps);
  challenge root binds the preheader; canonical `C`; soundness per ADR 0009 (ADR 0010).
