"""GPU fat-binary detection (synthetic and real fixtures).

The negative test runs everywhere. The synthetic positive tests embed a fat
binary in an ELF section via ``rustc --emit obj`` (ELF ``link_section``), so they
run on Linux with a Rust host toolchain and skip otherwise - no CUDA/ROCm
toolchain or committed binaries needed.

The real-fixture tests use the CUDA and ROCm binaries under ``testdata/``, copied
from https://github.com/tiran/elfgpu (built from testdata/hello.cu; the extracted
cubins, code objects, and readelf dumps are not included).
"""

from __future__ import annotations

import shutil
import struct
import subprocess
import sys
from pathlib import Path

import pytest

import objectfile
from objectfile import _cli

_FATBIN_MAGIC = 0xBA55ED50
_FLAG_ARCH_SPECIFIC = 0x10_0000

TESTDATA = Path(__file__).parent.parent / "testdata"

# Sorted, unique targets, matching elfgpu's EXPECTED_CUDA_ARCHS.
EXPECTED_CUDA_TARGETS = [
    "compute_120",
    "sm_100",
    "sm_120",
    "sm_120a",
    "sm_75",
    "sm_80",
    "sm_90",
    "sm_90a",
]

# Sorted, unique targets (host excluded), matching elfgpu's EXPECTED_ROCM_ARCHS.
EXPECTED_ROCM_TARGETS = [
    "gfx1100",
    "gfx90a:sramecc+:xnack+",
    "gfx90a:sramecc+:xnack-",
    "gfx942",
    "gfx950",
]


def _offload_bundle(entries: list[tuple[str, int]]) -> bytes:
    """Build a plain ``__CLANG_OFFLOAD_BUNDLE__`` from ``(id, size)`` pairs."""
    out = b"__CLANG_OFFLOAD_BUNDLE__" + struct.pack("<Q", len(entries))
    offset = 0
    for bundle_id, size in entries:
        out += struct.pack("<QQQ", offset, size, len(bundle_id)) + bundle_id.encode()
        offset += size
    return out


def _cuda_fatbin(entries: list[tuple[int, int, int, bytes]]) -> bytes:
    """Build a fat binary frame from ``(kind, sm, flags, payload)`` entries."""
    body = b""
    for kind, sm, flags, payload in entries:
        header = bytearray(64)
        struct.pack_into("<H", header, 0, kind)
        struct.pack_into("<I", header, 4, 64)  # headerSize
        struct.pack_into("<I", header, 8, len(payload))  # paddedPayloadSize
        struct.pack_into("<I", header, 16, len(payload))  # payloadSize
        struct.pack_into("<H", header, 26, 1)  # code version major
        struct.pack_into("<I", header, 28, sm)  # smVersion
        struct.pack_into("<Q", header, 40, flags)
        body += bytes(header) + payload
    return struct.pack("<IHHQ", _FATBIN_MAGIC, 1, 16, len(body)) + body


def _elf_with_section(tmp_path: Path, section: str, data: bytes) -> Path:
    """Compile a tiny ELF object with `data` placed in a named section."""
    if sys.platform != "linux":
        pytest.skip("ELF link_section embedding is Linux-only")
    if shutil.which("rustc") is None:
        pytest.skip("rustc not available")

    array = ", ".join(str(b) for b in data)
    source = tmp_path / "fixture.rs"
    source.write_text(
        f'#[link_section = "{section}"]\n'
        "#[no_mangle]\n"
        "#[used]\n"
        f"pub static FIXTURE: [u8; {len(data)}] = [{array}];\n"
    )
    out = tmp_path / "fixture.o"
    result = subprocess.run(
        [
            "rustc",
            "--crate-type=lib",
            "--emit=obj",
            "--target=x86_64-unknown-linux-gnu",
            "-o",
            str(out),
            str(source),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        pytest.skip(f"rustc build failed: {result.stderr.strip()[:200]}")
    return out


# --- Synthetic detection (no toolchain / fixtures needed) --------------------


def test_no_gpu_code_objects(sample_object: Path) -> None:
    obj = objectfile.parse_file(sample_object)
    assert obj.gpu_targets() == []
    assert obj.gpu_code_objects() == []


def test_detects_hip_bundle(tmp_path: Path) -> None:
    bundle = _offload_bundle(
        [
            ("host-x86_64-unknown-linux-gnu", 0),
            ("hipv4-amdgcn-amd-amdhsa--gfx90a", 16),
            ("hipv4-amdgcn-amd-amdhsa--gfx942", 16),
        ]
    )
    obj = objectfile.parse_file(_elf_with_section(tmp_path, ".hip_fatbin", bundle))

    assert obj.gpu_targets() == ["gfx90a", "gfx942"]
    code = obj.gpu_code_objects()
    assert all(o.compute_platform == "hip" for o in code)
    assert any(o.kind == "host" and o.target is None for o in code)
    assert {o.target for o in code if o.target} == {"gfx90a", "gfx942"}


def test_detects_cuda_fatbin(tmp_path: Path) -> None:
    fatbin = _cuda_fatbin(
        [
            (0x2, 90, _FLAG_ARCH_SPECIFIC, b"CUBIN-A"),  # sm_90a
            (0x2, 80, 0, b"CUBIN-B"),  # sm_80
            (0x1, 120, 0, b"PTXTEXT"),  # compute_120
        ]
    )
    obj = objectfile.parse_file(_elf_with_section(tmp_path, ".nv_fatbin", fatbin))

    assert obj.gpu_targets() == ["compute_120", "sm_80", "sm_90a"]
    kinds = {o.target: o.kind for o in obj.gpu_code_objects()}
    assert kinds == {"sm_90a": "elf", "sm_80": "elf", "compute_120": "ptx"}


def test_cli_gpu(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    bundle = _offload_bundle(
        [
            ("host-x86_64-unknown-linux-gnu", 0),
            ("hipv4-amdgcn-amd-amdhsa--gfx942", 16),
        ]
    )
    path = _elf_with_section(tmp_path, ".hip_fatbin", bundle)

    rc = _cli.main([str(path), "--gpu"])
    out = capsys.readouterr().out
    assert rc == 0
    assert "gpu targets:  gfx942" in out  # shown in the summary
    assert "gpu code objects (2):" in out
    assert "hip   gfx942" in out


# --- Real fixtures (CUDA + ROCm) ---------------------------------------------
# Compressed and uncompressed variants must agree: architectures come from the
# container entry headers, so the payloads need no decompression.


@pytest.mark.parametrize(
    ("fixture", "expected"),
    [
        ("cuda/hello.so", EXPECTED_CUDA_TARGETS),
        ("cuda-compressed/hello.so", EXPECTED_CUDA_TARGETS),
        ("rocm/hello.so", EXPECTED_ROCM_TARGETS),
        ("rocm-compressed/hello.so", EXPECTED_ROCM_TARGETS),
    ],
)
def test_fixture_targets(fixture: str, expected: list[str]) -> None:
    assert objectfile.parse_file(TESTDATA / fixture).gpu_targets() == expected


def test_fixture_code_objects() -> None:
    cuda = objectfile.parse_file(TESTDATA / "cuda" / "hello.so").gpu_code_objects()
    assert cuda
    assert all(co.compute_platform == "cuda" for co in cuda)
    assert {co.kind for co in cuda} <= {"ptx", "elf"}
    assert any(co.kind == "ptx" for co in cuda)  # a compute_* PTX entry
    assert any(co.kind == "elf" for co in cuda)  # sm_* cubin entries

    rocm = objectfile.parse_file(TESTDATA / "rocm" / "hello.so").gpu_code_objects()
    assert len(rocm) == 6  # host + 5 device archs
    assert all(co.compute_platform == "hip" for co in rocm)
    assert any(co.kind == "host" and co.target is None for co in rocm)
    assert sum(1 for co in rocm if co.kind == "code-object") == 5


@pytest.mark.parametrize(
    ("fixture", "absent"),
    [("rocm/hello.so", "cuda"), ("cuda/hello.so", "hip")],
)
def test_platform_isolation(fixture: str, absent: str) -> None:
    code = objectfile.parse_file(TESTDATA / fixture).gpu_code_objects()
    assert [co for co in code if co.compute_platform == absent] == []


def test_cli_gpu_on_fixture(capsys: pytest.CaptureFixture[str]) -> None:
    rc = _cli.main([str(TESTDATA / "rocm" / "hello.so"), "--gpu"])
    out = capsys.readouterr().out
    assert rc == 0
    assert "gpu targets:  gfx1100, gfx90a:sramecc+:xnack+" in out
    assert "gpu code objects (6):" in out
    assert "hip   gfx950" in out
