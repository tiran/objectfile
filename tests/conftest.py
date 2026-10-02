from __future__ import annotations

import sys
from pathlib import Path

import pytest


@pytest.fixture(scope="session")
def self_executable() -> Path:
    """The running interpreter: a real object file for the current platform."""
    return Path(sys.executable)


@pytest.fixture
def versioned_elf() -> Path:
    """A versioned ELF shared library, or skip if none is available."""
    candidates = [
        "/lib64/libstdc++.so.6",
        "/usr/lib/x86_64-linux-gnu/libstdc++.so.6",
        "/lib64/libc.so.6",
        "/usr/lib/x86_64-linux-gnu/libc.so.6",
    ]
    for candidate in candidates:
        path = Path(candidate)
        if path.exists():
            return path
    pytest.skip("no versioned ELF library available")
