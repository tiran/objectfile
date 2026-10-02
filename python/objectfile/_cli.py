"""Command-line interface: ``objectfile <path>``.

Prints format metadata and shared-library dependencies for an object file, with
optional flags to list imported/exported symbols and the symbol table.

.. warning::
   The CLI and its output format are **unstable** and may change at any time.
   Do not parse this output in scripts; use the Python API instead.
"""

from __future__ import annotations

import argparse
import sys
from typing import TYPE_CHECKING

from ._objectfile import parse_file

if TYPE_CHECKING:
    from collections.abc import Callable, Sequence

    from ._objectfile import ObjectFile

    # Maps a (possibly mangled) symbol name to a display name, or None if it
    # cannot be demangled.
    Demangler = Callable[[str], str | None]


def _load_demangler() -> Demangler | None:
    """Return ``pycxxfilt.demangle`` if the optional dependency is installed."""
    try:
        import pycxxfilt
    except ImportError:
        return None
    return pycxxfilt.demangle


def _display(name: str | None, demangle: Demangler | None) -> str | None:
    if name is None or demangle is None:
        return name
    # Fall back to the raw name if it cannot be demangled: the demangler returns
    # None for unmangled names and raises ValueError on ones it cannot parse
    # (e.g. some libstdc++ transaction-clone symbols like _ZGTt...).
    try:
        return demangle(name) or name
    except ValueError:
        return name


def _print_summary(obj: ObjectFile) -> None:
    print(f"format:       {obj.format}")
    print(f"architecture: {obj.architecture}")
    print(f"bits:         {'64' if obj.is_64 else '32'}")
    print(f"endianness:   {obj.endianness}")
    print(f"kind:         {obj.kind}")

    libraries = list(obj.libraries())
    print(f"libraries ({len(libraries)}):")
    for library in libraries:
        print(f"  {library}")

    print(f"imports:      {sum(1 for _ in obj.imports())}")
    print(f"exports:      {sum(1 for _ in obj.exports())}")


def _print_imports(obj: ObjectFile, demangle: Demangler | None) -> None:
    print("imports:")
    for imp in sorted(obj.imports()):
        name = _display(imp.name, demangle) or f"#{imp.ordinal}"
        suffix = f"  [{imp.library}]" if imp.library else ""
        print(f"  {name}{suffix}")


def _print_exports(
    obj: ObjectFile, demangle: Demangler | None, *, show_version: bool
) -> None:
    print("exports:")
    for exp in sorted(obj.exports()):
        name = _display(exp.name, demangle) or f"#{exp.ordinal}"
        version = ""
        if show_version and exp.version:
            # "@@" marks the default version, "@" a non-default one (as in nm).
            separator = "@" if exp.version_hidden else "@@"
            version = f"{separator}{exp.version}"
        print(f"  {name}{version}")


def _print_symbols(obj: ObjectFile, demangle: Demangler | None) -> None:
    print("symbols:")
    for sym in sorted(obj.symbols()):
        name = _display(sym.name, demangle) or "<unnamed>"
        print(f"  {name}  ({sym.kind}, {sym.scope})")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="objectfile",
        description="Inspect an object file (ELF, PE, Mach-O, ...).",
    )
    parser.add_argument("path", help="path to the object file")
    parser.add_argument("--imports", action="store_true", help="list imported symbols")
    parser.add_argument("--exports", action="store_true", help="list exported symbols")
    parser.add_argument(
        "--symbols", action="store_true", help="list symbol-table entries"
    )
    # Both flags demangle names (requires the 'cli' extra: pycxxfilt); they
    # differ only in whether the ELF symbol version suffix is kept on exports.
    demangle_group = parser.add_mutually_exclusive_group()
    demangle_group.add_argument(
        "--demangle",
        action="store_true",
        help="demangle C++/Rust symbol names (drops the export version suffix)",
    )
    demangle_group.add_argument(
        "--demangle-with-version",
        action="store_true",
        help="demangle names and keep the ELF export version (name@@version)",
    )
    args = parser.parse_args(argv)

    demangle: Demangler | None = None
    if args.demangle or args.demangle_with_version:
        demangle = _load_demangler()
        if demangle is None:
            print(
                "objectfile: --demangle requires pycxxfilt; "
                "install it with 'pip install objectfile[cli]'",
                file=sys.stderr,
            )
            return 1
    # The version suffix is kept unless plain --demangle was requested.
    show_version = not args.demangle

    try:
        obj = parse_file(args.path)
    except (OSError, ValueError) as err:
        print(f"objectfile: {args.path}: {err}", file=sys.stderr)
        return 1

    _print_summary(obj)
    if args.imports:
        _print_imports(obj, demangle)
    if args.exports:
        _print_exports(obj, demangle, show_version=show_version)
    if args.symbols:
        _print_symbols(obj, demangle)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
