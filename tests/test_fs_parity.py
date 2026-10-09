import os
import random
import shutil
import subprocess
import sys

import pytest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

from freivalds_forgery_probe import fs_challenges  # noqa: E402
from reference.freivalds import random_matrix  # noqa: E402

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
    n, k = 4, 3
    C = random_matrix(rng, n)
    stdin = f"{n} {k}\n" + " ".join(str(x) for x in C) + "\n"
    out = subprocess.run([path], input=stdin, capture_output=True, text=True, check=True).stdout
    rust = [int(x) for x in out.split()]
    py = [x for vec in fs_challenges(C, n, k) for x in vec]
    assert rust == py
