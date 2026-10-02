"""Cross-format / cross-architecture coverage.

These build a tiny object file for several Rust targets with ``rustc --emit obj``
(no linker or cross-SDK needed) and check the detected format/architecture plus
a defined export and an undefined (imported) symbol.

They run by default. A case skips when ``rustc`` or its target std is not
installed (install targets with ``rustup target add ...`` or Fedora's
``rust-std-static-<target>`` packages). Set ``OBJECTFILE_REQUIRE_CROSS=1`` to
turn those skips into failures - the CI "cross-fixtures" job does this after
installing every target, so it is a real regression guard.
"""

from __future__ import annotations

import os
import shutil
import subprocess
from typing import TYPE_CHECKING

import pytest

import objectfile

if TYPE_CHECKING:
    from pathlib import Path

# (rustc target, expected format name, expected architecture name)
CASES = [
    ("x86_64-unknown-linux-gnu", "Elf", "X86_64"),
    ("aarch64-unknown-none-softfloat", "Elf", "Aarch64"),
    ("powerpc64le-unknown-linux-gnu", "Elf", "PowerPc64"),
    ("s390x-unknown-linux-gnu", "Elf", "S390x"),
    ("x86_64-pc-windows-gnu", "Coff", "X86_64"),
    ("i686-pc-windows-gnu", "Coff", "I386"),
    ("aarch64-apple-darwin", "MachO", "Aarch64"),
    ("wasm32-unknown-unknown", "Wasm", "Wasm32"),
]

# `#![no_std]` so bare-metal targets (e.g. aarch64-unknown-none-softfloat) build
# too. Exports `objectfile_fixture` and references `abort`, giving the object a
# defined symbol and an undefined (imported) one.
_SOURCE = """
#![no_std]

extern "C" {
    fn abort();
}

#[no_mangle]
pub extern "C" fn objectfile_fixture(x: u32) -> u32 {
    if x == 0 {
        unsafe { abort() };
    }
    x.wrapping_add(1)
}
"""

_REQUIRE = os.environ.get("OBJECTFILE_REQUIRE_CROSS") == "1"


def _unavailable(reason: str) -> None:
    """Fail when cross builds are required, otherwise skip."""
    if _REQUIRE:
        pytest.fail(reason)
    pytest.skip(reason)


@pytest.mark.parametrize(("target", "fmt", "arch"), CASES)
def test_cross_format(tmp_path: Path, target: str, fmt: str, arch: str) -> None:
    if shutil.which("rustc") is None:
        _unavailable("rustc not available")

    source = tmp_path / "fixture.rs"
    source.write_text(_SOURCE)
    out = tmp_path / "fixture.o"

    result = subprocess.run(
        [
            "rustc",
            "--crate-name=fixture",
            "--crate-type=lib",
            "--emit=obj",
            f"--target={target}",
            "-o",
            str(out),
            str(source),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        _unavailable(f"target {target} unavailable: {result.stderr.strip()[:200]}")

    obj = objectfile.parse_file(out)
    assert str(obj.format) == fmt
    assert str(obj.architecture) == arch

    # Relocatable objects have empty dynamic-linking tables (imports/exports);
    # the real symbols live in the symbol table.
    symbols = list(obj.symbols())

    def has(name: str, *, undefined: bool) -> bool:
        # Mach-O prefixes C symbol names with an underscore.
        return any(
            sym.name in (name, f"_{name}") and sym.is_undefined == undefined
            for sym in symbols
        )

    assert has("objectfile_fixture", undefined=False)  # defined export
    assert has("abort", undefined=True)  # undefined import
