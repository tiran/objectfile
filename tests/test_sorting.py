from __future__ import annotations

from objectfile import Export, Import


def test_imports_sort_by_name() -> None:
    imports = [
        Import(name="zlib_free", ordinal=None, library=None, is_weak=False),
        Import(name="abort", ordinal=None, library=None, is_weak=False),
        Import(name="malloc", ordinal=None, library=None, is_weak=False),
    ]
    assert [i.name for i in sorted(imports)] == ["abort", "malloc", "zlib_free"]


def test_exports_sort_by_name() -> None:
    exports = [
        Export(name="z", ordinal=None, address=3, is_weak=False),
        Export(name="a", ordinal=None, address=1, is_weak=False),
    ]
    assert [e.name for e in sorted(exports)] == ["a", "z"]


def test_ordinal_only_symbols_sort_without_error() -> None:
    # Name is None for ordinal imports (PE); sorting must not raise.
    imports = [
        Import(name=None, ordinal=7, library="k.dll", is_weak=False),
        Import(name=None, ordinal=2, library="k.dll", is_weak=False),
        Import(name="named", ordinal=None, library="k.dll", is_weak=False),
    ]
    ordered = sorted(imports)
    assert [i.ordinal for i in ordered[:2]] == [2, 7]
    assert ordered[-1].name == "named"
