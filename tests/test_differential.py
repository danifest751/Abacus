import os
import random
import shutil
import subprocess
import sys

import pytest

from reference.freivalds import P, freivalds_verify, matmul, random_matrix, random_vector

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = "abacus-adapter.exe" if os.name == "nt" else "abacus-adapter"


def _build_adapter():
    cargo = shutil.which("cargo")
    if cargo is None:
        pytest.skip("cargo not found")
    subprocess.run(
        [cargo, "build", "--quiet", "--bin", "abacus-adapter"],
        cwd=ROOT,
        check=True,
    )
    path = os.path.join(ROOT, "target", "debug", EXE)
    if not os.path.exists(path):
        pytest.skip("adapter binary not built")
    return path


def test_python_rust_differential():
    adapter = _build_adapter()

    rng = random.Random(20261009)
    n = 16
    cases = []
    for i in range(64):
        a = random_matrix(rng, n)
        b = random_matrix(rng, n)
        c = matmul(a, b, n)
        if i % 3 == 0:  # tamper some cases
            c[i % (n * n)] = (c[i % (n * n)] + 1) % P
        r = random_vector(rng, n)
        cases.append((a, b, c, r))

    lines = [f"{n} {len(cases)}"]
    for a, b, c, r in cases:
        lines.append(" ".join(str(x) for x in (a + b + c + r)))
    stdin = "\n".join(lines) + "\n"

    out = subprocess.run(
        [adapter], input=stdin, capture_output=True, text=True, check=True
    ).stdout.split()
    assert len(out) == len(cases)

    for (a, b, c, r), verdict in zip(cases, out):
        py = freivalds_verify(a, b, c, n, r)
        py_s = "1" if py else "0"
        assert py_s == verdict, "Python/Rust disagreement"
