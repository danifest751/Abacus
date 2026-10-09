#!/usr/bin/env python3
"""Abacus local check gate.

Runs the Python tests (including the Python/Rust differential and parity tests), rustfmt and
clippy (warnings are errors), the Rust workspace tests and the Rust self-test binary. No network, no third-party Python packages beyond pytest.

Usage: python scripts/check.py
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def run(name: str, cmd: list[str], cwd: str = ROOT) -> bool:
    print(f"== {name}: {' '.join(cmd)}")
    proc = subprocess.run(cmd, cwd=cwd)
    ok = proc.returncode == 0
    print(f"== {name}: {'OK' if ok else 'FAILED'}")
    return ok


def main() -> int:
    results = []

    # Python tests (include the Python/Rust differential corpus).
    results.append(run("pytest", [sys.executable, "-m", "pytest", "tests", "-q"]))

    # Rust tests and self-test, if a toolchain is present.
    cargo = shutil.which("cargo")
    if cargo:
        results.append(run("cargo fmt", [cargo, "fmt", "--all", "--check"]))
        results.append(
            run("cargo clippy", [cargo, "clippy", "--workspace", "--all-targets", "--quiet", "--", "-D", "warnings"])
        )
        results.append(run("cargo test", [cargo, "test", "--workspace", "--quiet"]))
        results.append(
            run("cargo run self-test", [cargo, "run", "--quiet", "--bin", "abacus-verifier"])
        )
    else:
        print("== cargo not found: Rust checks skipped")

    if all(results):
        print("\nALL CHECKS PASSED")
        return 0
    print("\nCHECKS FAILED")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
