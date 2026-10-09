# ABACUS-LAB-v1 — interactive proof of tensor throughput (attestation)

Status: implemented prototype (2026-10-09, ADR 0014). Rust verifier and reference prover in
`crates/abacus-attest`; GPU prover `cuda/attest_prover.cu`. Not a consensus mechanism: an interactive
challenge–response between one verifier and one prover.

## 1. Purpose

Let a verifier (e.g. a compute marketplace or a customer) check, cheaply and without trusting the
prover's hardware reports, that a prover performed a stated amount of exact int8 matrix-multiply work
within a measured time — a cryptographic lower bound on tensor throughput.

## 2. Protocol

```
precompute (verifier, before the challenge, secret):
    seed, secret  <- fresh randomness
    r             = first n field elements of the stream expand(secret)            (secret)
    v_j           = B_j r  (mod P)  for j = 0..m-1                                  (O(m n^2))

1. V -> P   CHAL seed n m                                        (clock starts)
2. P        for j < m:  s_j = SHA256("abacus/attest-seed" || seed || LE32(j))
                        A_j || B_j = first 2 n^2 bytes of SHA256("abacus/expand" || s_j || LE32(c)), c = 0..
                                     read as int8, row-major
                        C_j = A_j * B_j exactly (int32)
            leaf(j, i) = SHA256(C_j[i, :] as LE int32 || 0x00 || LE32(j) || LE32(i))
            root       = Merkle(leaves in order j n + i, padded with SHA256("abacus/attest-empty"),
                                node = SHA256(0x01 || left || right))
   P -> V   ROOT root                                            (clock stops: prove time T)
3. V        S = k distinct (j, i), derived from (secret, root)
   V -> P   OPEN S
4. P -> V   for each (j, i) in S: the row C_j[i, :] and its Merkle path; END
5. V        accept iff every path verifies against root and, for every opened row,
            C_j[i, :] . r == A_j[i, :] . v_j   (mod P, Goldilocks)
```

`A_j[i, :]` costs the verifier `n` bytes of expansion; the online check is `O(k n)` plus `k` Merkle
paths.

## 3. Soundness

- **Opened rows.** For a wrong row, `e = row - C_j[i, :]` is a nonzero integer vector with entries
  `< 2^32 < P` in absolute value (`n <= 2^16`), so `e . r` vanishes for the secret `r` with probability
  at most `2^-63` (spec/01, spec/05). The prover never sees `r`.
- **Unopened rows.** The root is fixed before `S` is chosen, so there is no grinding. A prover whose
  committed rows are wrong in a fraction `f` passes with probability
  `C((1-f) N, k) / C(N, k) <= (1-f)^k`, `N = m n`. Repeated challenges multiply this.
- **Throughput.** An accepted response certifies, except with the probability above, that at least
  `(1 - f) m n^3` exact int8 multiply-adds were performed between CHAL and ROOT: `T` gives the lower
  bound `(1 - f) m n^3 / T`. Expansion, hashing and network time are included in `T`, so the bound is
  conservative.
- **Freshness.** `seed` is new per challenge; no product can be precomputed.

## 4. What it does not show

- **Location or ownership.** The prover can forward the challenge to other hardware; the protocol
  proves access to compute within the deadline, not which machine computed.
- **Floating point.** Only exact integer GEMM is covered; fp16/bf16 workloads are not.
- **Peak vs sustained.** A challenge measures one burst; sustained throughput needs repeated challenges.
- **Clock state.** An idle GPU is under-clocked; a warm-up challenge is needed before measuring (the
  first challenge after idle measured 5.4 instead of ~41 TMAC/s).

## 5. Parameters and costs (CMP 50HX; `docs/research/attest-v1.md`)

| n | m | proven TMAC/s | prove ms | open ms (LAN) | verify ms | precompute s | detects f = 5% / 10% (k = 32) |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 8192 | 4 | ~41 | ~53 | ~19 | ~12 | ~3.8 | 81% / 97% |
| 16384 | 2 | ~49 | ~180 | ~37 | ~24 | ~7.7 | 81% / 97% |

`k` trades bandwidth (`4 n` bytes per opened row) for detection; `k = 128` detects `f = 5%` with
99.9%.

## 6. Wire format

Text lines over TCP: `CHAL <hex> <n> <m>`, `ROOT <hex>`, `OPEN j:i ...`,
`ROW <j> <i> <row hex> <path hex>`, `END`. `n` a multiple of 16 (the GPU prover requires 32).
