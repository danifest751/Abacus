"""Candidate T robustness probe (ADR 0016): does approximate computation ever yield a valid ticket?

1. Error propagation: change one activation by +-1 after layer 1 and count the entries of each later
   layer that differ from the exact computation.
2. Approximate last layer: drop the `k` smallest-magnitude product terms, or round the weights to
   even values (int7), and count exact entries and exact `w`-byte pieces of the output row.

Weights are uniform int8 drawn with numpy (statistically the same as the SHA-256 expansion of
spec/07; the consensus derivation is not needed for these statistics). Prints JSON lines.

    python scripts/tnet_robustness_probe.py [n] [layers] [rows] [seed]
"""

from __future__ import annotations

import json
import sys

import numpy as np

REQ_SHIFT = 24


def mult(n: int) -> int:
    return round((1 << REQ_SHIFT) / (74.0 * n ** 0.5))


def requant(y: np.ndarray, m: int) -> np.ndarray:
    return np.clip((y.astype(np.int64) * m + (1 << (REQ_SHIFT - 1))) >> REQ_SHIFT, -128, 127).astype(np.int8)


def main() -> None:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 8192
    layers = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    rows = int(sys.argv[3]) if len(sys.argv) > 3 else 16
    seed = int(sys.argv[4]) if len(sys.argv) > 4 else 1
    w_piece = 256
    m = mult(n)
    rng = np.random.default_rng(seed)
    ws = [rng.integers(-128, 128, size=(n, n), dtype=np.int8) for _ in range(layers)]
    w32 = [w.astype(np.int32) for w in ws]

    def layer(x: np.ndarray, l: int) -> np.ndarray:
        return requant(x.astype(np.int32) @ w32[l], m)

    prop = [[] for _ in range(layers)]
    approx = {}
    for r in range(rows):
        x0 = rng.integers(-128, 128, size=n, dtype=np.int8)
        exact = [x0]
        for l in range(layers):
            exact.append(layer(exact[-1], l))
        # 1. one +-1 change after layer 1
        x = exact[1].copy()
        k = int(rng.integers(n))
        x[k] = x[k] + 1 if x[k] < 127 else x[k] - 1
        for l in range(1, layers):
            x = layer(x, l)
            prop[l].append(int(np.count_nonzero(x != exact[l + 1])))
        # 2. approximate last layer
        xin = exact[layers - 1].astype(np.int32)
        y_exact = xin @ w32[layers - 1]
        order = np.argsort(np.abs(xin))
        variants = {f"drop_{d}_smallest_terms": d for d in (1, 8, 64, 512)}
        for name, d in variants.items():
            keep = xin.copy()
            keep[order[:d]] = 0
            out = requant(keep @ w32[layers - 1], m)
            approx.setdefault(name, []).append(out == exact[layers])
        w_even = (w32[layers - 1] >> 1) << 1
        approx.setdefault("int7_weights", []).append(requant(xin @ w_even, m) == exact[layers])
        if r == 0:
            spread = float(np.std(y_exact))
    for l in range(1, layers):
        v = np.array(prop[l])
        print(json.dumps({"probe": "propagation", "n": n, "after_layer": l + 1, "rows": rows,
                          "differing_median": float(np.median(v)), "differing_frac": float(np.median(v)) / n}))
    for name, oks in approx.items():
        ok = np.array(oks)
        pieces = ok.reshape(rows, n // w_piece, w_piece).all(axis=2)
        print(json.dumps({"probe": "approximation", "n": n, "variant": name, "rows": rows,
                          "entry_exact_frac": float(ok.mean()), "piece_exact_frac": float(pieces.mean()),
                          "pieces": int(pieces.size)}))
    print(json.dumps({"probe": "scale", "n": n, "mult": m, "acc_std_last_layer": spread,
                      "lsb_in_acc_units": (1 << REQ_SHIFT) / m}))


if __name__ == "__main__":
    main()
