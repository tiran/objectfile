from __future__ import annotations

import collections.abc
import itertools
from typing import TYPE_CHECKING

import objectfile
from objectfile import Symbol, SymbolKind, SymbolScope

if TYPE_CHECKING:
    from pathlib import Path


def test_symbols_returns_fresh_iterator(self_executable: Path) -> None:
    obj = objectfile.parse_file(self_executable)
    first = obj.symbols()
    second = obj.symbols()
    assert isinstance(first, collections.abc.Iterator)
    assert first is not second


def test_symbols_yield_typed_entries(self_executable: Path) -> None:
    obj = objectfile.parse_file(self_executable)
    sym = next(iter(obj.symbols()), None)
    if sym is None:
        return  # a fully stripped binary - nothing to assert
    assert isinstance(sym, Symbol)
    assert isinstance(sym.kind, SymbolKind)
    assert isinstance(sym.scope, SymbolScope)
    assert isinstance(sym.address, int)
    assert isinstance(sym.is_undefined, bool)


def test_symbols_are_lazy(self_executable: Path) -> None:
    obj = objectfile.parse_file(self_executable)
    head = list(itertools.islice(obj.symbols(), 3))
    assert all(isinstance(s, Symbol) for s in head)


def test_dynamic_symbols_iterable(self_executable: Path) -> None:
    obj = objectfile.parse_file(self_executable)
    assert all(isinstance(s, Symbol) for s in obj.dynamic_symbols())


def test_symbols_sortable_by_name() -> None:
    syms = [
        Symbol(
            "z", 0x30, 0, SymbolKind.Text, SymbolScope.Dynamic, False, True, False, 1
        ),
        Symbol(
            "a", 0x10, 0, SymbolKind.Data, SymbolScope.Dynamic, False, True, False, 1
        ),
        Symbol(
            None,
            0x20,
            0,
            SymbolKind.Text,
            SymbolScope.Unknown,
            True,
            False,
            False,
            None,
        ),
    ]
    assert [s.name for s in sorted(syms)] == [None, "a", "z"]


def test_symbol_equality_and_hash() -> None:
    a = Symbol("x", 1, 2, SymbolKind.Text, SymbolScope.Dynamic, False, True, False, 0)
    b = Symbol("x", 1, 2, SymbolKind.Text, SymbolScope.Dynamic, False, True, False, 0)
    assert a == b
    assert hash(a) == hash(b)
    assert len({a, b}) == 1


def test_symbol_defaults() -> None:
    sym = Symbol("name")
    assert sym.address == 0
    assert sym.kind == SymbolKind.Unknown
    assert sym.section_index is None
