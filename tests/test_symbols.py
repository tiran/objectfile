from __future__ import annotations

import collections.abc
import itertools
from typing import TYPE_CHECKING

import objectfile
from objectfile import Symbol, SymbolKind, SymbolScope

if TYPE_CHECKING:
    from pathlib import Path


def test_symbol_iteration(sample_object: Path) -> None:
    obj = objectfile.parse_file(sample_object)

    # Each call returns a fresh, independent iterator.
    first, second = obj.symbols(), obj.symbols()
    assert isinstance(first, collections.abc.Iterator)
    assert first is not second

    # Lazy: a slice yields typed entries, as does dynamic_symbols().
    head = list(itertools.islice(obj.symbols(), 3))
    assert all(isinstance(s, Symbol) for s in head)
    assert all(isinstance(s, Symbol) for s in obj.dynamic_symbols())

    sym = next(iter(obj.symbols()), None)
    if sym is not None:  # a fully stripped binary has nothing to check
        assert isinstance(sym.kind, SymbolKind)
        assert isinstance(sym.scope, SymbolScope)
        assert isinstance(sym.address, int)
        assert isinstance(sym.is_undefined, bool)


def test_symbol_value_semantics() -> None:
    # Constructor defaults.
    default = Symbol("name")
    assert default.address == 0
    assert default.kind == SymbolKind.Unknown
    assert default.section_index is None

    # Equality and hashing.
    a = Symbol("x", 1, 2, SymbolKind.Text, SymbolScope.Dynamic, False, True, False, 0)
    b = Symbol("x", 1, 2, SymbolKind.Text, SymbolScope.Dynamic, False, True, False, 0)
    assert a == b
    assert hash(a) == hash(b)
    assert len({a, b}) == 1

    # Sorting by name, with a None name first.
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
