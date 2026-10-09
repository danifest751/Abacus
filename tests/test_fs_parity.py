import os
import random
import shutil
import subprocess

import pytest

from reference.chain import fs_challenges
from reference.freivalds import random_matrix

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = "abacus-fs-adapter.exe" if os.name == "nt" else "abacus-fs-adapter"


def test_fs_challenge_parity():
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("cargo not found")
    subprocess.run(
        [cargo, "build", "--quiet", "--bin", "abacus-fs-adapter"], cwd=ROOT, check=True
    )
    path = os.path.join(ROOT, "target", "debug", EXE)
    if not os.path.exists(path):
        pytest.skip("adapter binary not built")

    rng = random.Random(5)
    for n, k, ph in ((4, 3, b"abacus/ph-test"), (5, 2, b""), (3, 7, bytes(range(77)))):
        C = random_matrix(rng, n)
        stdin = f"{n} {k}\n" + (ph.hex() or "-") + "\n" + " ".join(str(x) for x in C) + "\n"
        out = subprocess.run([path], input=stdin, capture_output=True, text=True, check=True).stdout
        rust = [int(x) for x in out.split()]
        py = [x for vec in fs_challenges(ph, C, n, k) for x in vec]
        assert rust == py


def test_challenges_bind_the_preheader():
    C = list(range(16))
    assert fs_challenges(b"a", C, 4, 2) != fs_challenges(b"b", C, 4, 2)
