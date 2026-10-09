# TNet v1 test vectors

One JSON object per line, produced by `abacus-tnet vectors n b L w mult epoch hd nonce i...`
(`crates/abacus-chain/src/bin/abacus-tnet.rs`). Fields: parameters, `epoch` and `hd` (hex), `nonce`,
row `i`, `weights_sha256` (SHA-256 of each `W_l`, row-major int8), `x0_sha256` and `row_sha256`
(SHA-256 of row `i` of `X_0` and of `X_L`), `row` (hex, small instances only), and `tickets` (the
`n / w` ticket hashes of row `i`, `c = 0, 1, ...`).

| File | Parameters | Checked by |
|---|---|---|
| `tnet-v1-small.jsonl` | `n = 256, b = 64, L = 4, w = 64, M = 14170`, nonce 7, rows 0 and 63 | `tests/test_tnet_parity.py` (Python reference) |
| `tnet-v1-frozen.jsonl` | frozen TNet v1: `n = 8192, b = 65536, L = 8, w = 256, M = 2505`, nonce 0, rows 0, 1, 255, 65535 | Rust; row 255, ticket `c = 23` (15 leading zero bits) was found by the GPU miner `cuda/tnet_bench.cu` |

Both use `epoch = 00 01 .. 1f` and `hd = 20 21 .. 3f`. To re-check the frozen file (512 MiB of weights,
a few seconds):

```sh
cargo build --release -p abacus-chain
E=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
H=202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f
target/release/abacus-tnet vectors 8192 65536 8 256 2505 $E $H 0 0 1 255 65535 | diff - spec/vectors/tnet-v1-frozen.jsonl
```
