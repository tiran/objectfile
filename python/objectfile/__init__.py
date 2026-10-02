"""objectfile - parse ELF, Mach-O, PE, and WebAssembly object files from Python.

Parse a buffer or a (memory-mapped) file and inspect its format metadata, imported
and exported symbols, shared-library dependencies, and symbol tables. Under the
hood this is a thin, typed wrapper around the read API of the Rust ``object`` crate.

    >>> import objectfile
    >>> obj = objectfile.parse_file("/bin/ls")
    >>> obj.format, obj.architecture, obj.is_64
    (Format.Elf, Architecture.X86_64, True)
    >>> list(obj.libraries())
    ['libc.so.6']
    >>> undefined = [s for s in obj.symbols() if s.is_undefined]

Everything is implemented in the compiled ``_objectfile`` module and re-exported
here; this file is import-only.
"""

from __future__ import annotations

from ._objectfile import (
    Architecture,
    Endianness,
    Export,
    Format,
    GpuCodeObject,
    Import,
    ObjectFile,
    ObjectFileError,
    ObjectKind,
    Symbol,
    SymbolKind,
    SymbolScope,
    parse,
    parse_file,
)

__all__ = [
    "Architecture",
    "Endianness",
    "Export",
    "Format",
    "GpuCodeObject",
    "Import",
    "ObjectFile",
    "ObjectFileError",
    "ObjectKind",
    "Symbol",
    "SymbolKind",
    "SymbolScope",
    "parse",
    "parse_file",
]
