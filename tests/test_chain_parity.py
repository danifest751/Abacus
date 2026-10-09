"""Python/Rust differential test of every consensus derivation (ADR 0010).

Runs the Rust ``abacus-vectors`` binary and compares preheader, instance, product, score, block id,
Fiat-Shamir challenges, the A' dataset, gather indices, gathered instance and the sumcheck
transcript against ``reference.chain`` / ``reference.sumcheck``.
"""

import os
import shutil
import subprocess

import pytest

from reference import chain, sumcheck
from reference.chain import sha256
from reference.freivalds import matmul

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = "abacus-vectors.exe" if os.name == "nt" else "abacus-vectors"

CASES = [
    # chain_id, version, height, prev, timestamp, bits, nonce, n, k, nblocks, epoch_seed
    (b"\xab" * 32, 2, 0, b"\x00" * 32, 10, 6, 0, 4, 3, 64, b"\x07" * 32),
    (bytes(range(32)), 1, 17, bytes(range(32, 64)), 1_760_000_000, 12, (5 << 32) | 99, 5, 2, 513, b"\x09" * 32),
    (b"\x01" * 32, 7, 2**40, b"\xff" * 32, 2**63, 120, 2**64 - 1, 3, 4, 1, b"\x00" * 32),
]


def _adapter():
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("cargo not found")
    subprocess.run([cargo, "build", "--quiet", "--bin", "abacus-vectors"], cwd=ROOT, check=True)
    path = os.path.join(ROOT, "target", "debug", EXE)
    if not os.path.exists(path):
        pytest.skip("vectors binary not built")
    return path


def _dec(s):
    return [int(x) for x in s.split(",")] if s else []


@pytest.mark.parametrize("case", CASES)
def test_chain_derivations_match_rust(case):
    path = _adapter()
    cid, ver, h, prev, ts, bits, nonce, n, k, nblocks, epoch = case
    args = [cid.hex(), str(ver), str(h), prev.hex(), str(ts), str(bits), str(nonce), str(n), str(k),
            str(nblocks), epoch.hex()]
    out = subprocess.run([path, *args], capture_output=True, text=True, check=True).stdout
    rust = dict(line.split("=", 1) for line in out.splitlines() if "=" in line)

    ph = chain.preheader(cid, ver, h, prev, ts, bits, nonce)
    assert rust["ph"] == ph.hex()
    a, b = chain.instance(ph, n)
    c = matmul(a, b, n)
    assert _dec(rust["a"]) == a and _dec(rust["b"]) == b and _dec(rust["c"]) == c
    assert rust["score"] == chain.score(ph, c).hex()
    assert int(rust["lead"]) == chain.score_lead(chain.score(ph, c))
    assert rust["id"] == chain.block_id(ph, c).hex()
    assert _dec(rust["fs"]) == [x for v in chain.fs_challenges(ph, c, n, k) for x in v]

    ds = chain.build_dataset(epoch, nblocks)
    assert rust["ds_first"] == ds[0].hex() and rust["ds_last"] == ds[-1].hex()
    assert rust["ds_sha"] == sha256(b"".join(ds)).hex()
    assert _dec(rust["idx"]) == chain.expand_indices(chain.instance_seed(ph), 2 * n * n, nblocks)
    ha, hb = chain.instance_hard(ph, n, ds)
    assert _dec(rust["hard_a"]) == ha and _dec(rust["hard_b"]) == hb
    assert rust["hard_score"] == chain.score(ph, matmul(ha, hb, n)).hex()

    table = chain.expand(ph, 16)
    claimed, rounds = sumcheck.prove(table, 4)
    assert _dec(rust["sc_table"]) == table
    assert int(rust["sc_claimed"]) == claimed
    assert _dec(rust["sc_rounds"]) == [x for r in rounds for x in r]
    assert _dec(rust["sc_challenges"]) == sumcheck.challenges(table, 4, claimed, rounds)
    assert sumcheck.verify(table, 4, claimed, rounds)
