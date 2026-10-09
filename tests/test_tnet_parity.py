"""Python/Rust differential test of candidate T (spec/07).

Runs ``abacus-tnet vectors`` and recomputes weights, input rows, output rows and every ticket hash
with ``reference.tnet``.
"""

import json
import os
import shutil
import subprocess

import pytest

from reference import tnet
from reference.chain import sha256

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = "abacus-tnet.exe" if os.name == "nt" else "abacus-tnet"

CASES = [
    # n, b, L, w, mult, epoch, header digest, nonce, rows
    (64, 16, 4, 16, tnet.default_mult(64), b"\x06" * 32, b"\x07" * 32, 0, [0, 15]),
    (128, 64, 3, 32, tnet.default_mult(128), bytes(range(32)), bytes(range(32, 64)), 2**64 - 1, [1, 63]),
    (64, 8, 2, 64, 1 << 20, b"\x00" * 32, b"\xff" * 32, 12345, [7]),  # coarse scale: saturates
]


def _adapter():
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("cargo not found")
    subprocess.run([cargo, "build", "--quiet", "--bin", "abacus-tnet"], cwd=ROOT, check=True)
    path = os.path.join(ROOT, "target", "debug", EXE)
    if not os.path.exists(path):
        pytest.skip("abacus-tnet not built")
    return path


@pytest.mark.parametrize("case", CASES)
def test_tnet_matches_rust(case):
    n, b, layers, w, mult, epoch, hd, nonce, rows = case
    args = [str(n), str(b), str(layers), str(w), str(mult), epoch.hex(), hd.hex(), str(nonce), *map(str, rows)]
    out = subprocess.run([_adapter(), "vectors", *args], capture_output=True, text=True, check=True).stdout
    got = [json.loads(line) for line in out.splitlines()]
    assert [g["i"] for g in got] == rows

    weights = [tnet.layer_weights(epoch, n, l) for l in range(layers)]
    seed = tnet.x0_seed(hd, nonce)
    for g in got:
        i = g["i"]
        assert g["weights_sha256"] == [sha256(tnet.encode(wl)).hex() for wl in weights]
        assert g["x0_sha256"] == sha256(tnet.encode(tnet.x0_row(seed, n, i))).hex()
        row = tnet.forward_row(weights, n, mult, seed, i)
        assert g["row"] == tnet.encode(row).hex()
        assert g["row_sha256"] == sha256(tnet.encode(row)).hex()
        want = [tnet.ticket_hash(row[c * w:(c + 1) * w], hd, nonce, i, c).hex() for c in range(n // w)]
        assert g["tickets"] == want


def test_requant_edges():
    m = 1 << 20  # scale 1/16
    assert [tnet.requant(y, m) for y in (0, 7, 8, -8, -9, 1 << 20, -(1 << 20))] == [0, 0, 1, 0, -1, 127, -128]
    assert tnet.default_mult(4096) == 3542 and tnet.default_mult(8192) == 2505


def test_committed_small_vectors():
    path = os.path.join(ROOT, "spec", "vectors", "tnet-v1-small.jsonl")
    with open(path, encoding="utf-8") as f:
        got = [json.loads(line) for line in f]
    for g in got:
        n, w, mult, layers = g["n"], g["w"], g["mult"], g["L"]
        epoch, hd = bytes.fromhex(g["epoch"]), bytes.fromhex(g["hd"])
        weights = [tnet.layer_weights(epoch, n, l) for l in range(layers)]
        row = tnet.forward_row(weights, n, mult, tnet.x0_seed(hd, g["nonce"]), g["i"])
        assert g["row"] == tnet.encode(row).hex()
        assert g["tickets"] == [tnet.ticket_hash(row[c * w:(c + 1) * w], hd, g["nonce"], g["i"], c).hex()
                                for c in range(n // w)]
