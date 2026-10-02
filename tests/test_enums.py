from __future__ import annotations

from objectfile import (
    Architecture,
    Endianness,
    Format,
    ObjectKind,
    SymbolKind,
    SymbolScope,
)


def test_members_are_distinct() -> None:
    assert Format.Elf != Format.Pe
    assert Format.Elf != Format.Unknown
    assert Architecture.Aarch64 != Architecture.X86_64
    assert Endianness.Little != Endianness.Big
    assert ObjectKind.Dynamic != ObjectKind.Executable
    assert SymbolKind.Text != SymbolKind.Data
    assert SymbolScope.Dynamic != SymbolScope.Linkage


def test_members_compare_equal_to_themselves() -> None:
    assert Format.Elf == Format.Elf
    assert SymbolKind.Text == SymbolKind.Text


def test_str_is_the_bare_name() -> None:
    assert str(Format.Elf) == "Elf"
    assert str(Architecture.X86_64) == "X86_64"
    assert str(SymbolKind.Text) == "Text"
    # repr keeps the type prefix.
    assert repr(Format.Elf) == "Format.Elf"


def test_enums_are_int_valued() -> None:
    # pyo3 native enums (eq_int) carry integer values; Unknown is first (0).
    assert int(Format.Unknown) == 0
    assert int(SymbolKind.Unknown) == 0
    assert int(SymbolScope.Unknown) == 0
