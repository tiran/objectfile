from __future__ import annotations

from pathlib import Path

import pytest


@pytest.fixture(scope="session")
def sample_object() -> Path:
    """A real, single-architecture object file for the current platform.

    Uses the compiled extension module (ELF ``.so`` / Mach-O ``.so`` / PE
    ``.pyd``), which is always present. Unlike ``sys.executable``, it is never a
    universal (fat) Mach-O - which ``object`` cannot parse - so this is stable
    across macOS as well as Linux and Windows.
    """
    from objectfile import _objectfile

    assert _objectfile.__file__ is not None
    return Path(_objectfile.__file__)


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
