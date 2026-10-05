# objectfile

Parse ELF, Mach-O, PE, and WebAssembly binaries (plus COFF and XCOFF) from Python:
read their format metadata, imported and exported symbols, shared-library dependencies,
and symbol tables. `objectfile` is a fast, typed extension built with
[PyO3](https://pyo3.rs) on top of the read API of the Rust
[`object`](https://crates.io/crates/object) crate.

Files are **memory-mapped**, not read into memory, so even very large binaries only
page in what you actually touch.

## Install

```console
$ pip install objectfile
```

Wheels are built with the stable ABI (`abi3`), so a single wheel per platform supports
CPython 3.12 and newer.

## Quickstart

```pycon
>>> import objectfile
>>> obj = objectfile.parse_file("/bin/ls")

>>> obj.format
Format.Elf
>>> obj.architecture
Architecture.X86_64
>>> obj.is_64
True
>>> obj.endianness
Endianness.Little
>>> obj.kind
ObjectKind.Dynamic

>>> # Shared-library dependencies (DT_NEEDED / dylibs / imported DLLs)
>>> list(obj.libraries())
['libc.so.6']

>>> # Imported and exported symbols (dynamic-linking view)
>>> for imp in obj.imports():
...     print(imp.name, imp.library, imp.is_weak)
>>> for exp in obj.exports():
...     print(exp.name, hex(exp.address) if exp.address is not None else None)

>>> # Symbol tables (symbol-table view)
>>> undefined = [s for s in obj.symbols() if s.is_undefined]
>>> for sym in obj.dynamic_symbols():
...     print(sym.name, sym.kind, sym.scope)
```

All collections (`imports()`, `exports()`, `libraries()`, `symbols()`,
`dynamic_symbols()`) are methods returning lazy iterators - pass them to `list()`,
`sorted()`, or a comprehension.

You can also parse an in-memory buffer:

```pycon
>>> obj = objectfile.parse(open("/bin/ls", "rb").read())
```

## Demangling symbols

Symbol names are returned raw (mangled), e.g. `_ZN3std2io5Write9write_fmt`. To turn
them into readable signatures, pair `objectfile` with
[pycxxfilt](https://pypi.org/project/pycxxfilt/), which demangles C++ and Rust symbols
(including the IA-64/Itanium and MSVC schemes):

```pycon
>>> import objectfile
>>> import pycxxfilt
>>> obj = objectfile.parse_file("/bin/ls")
>>> for sym in obj.dynamic_symbols():
...     if sym.name:
...         print(pycxxfilt.demangle(sym.name))
```

## GPU fat binaries

Detect embedded NVIDIA CUDA and AMD HIP/ROCm device code, and the GPU
architectures a binary ships for:

```pycon
>>> obj = objectfile.parse_file("libtorch_hip.so")
>>> obj.gpu_targets()
['gfx90a', 'gfx942']
>>> for co in obj.gpu_code_objects():
...     print(co.compute_platform, co.target, co.kind)
hip None host
hip gfx90a code-object
hip gfx942 code-object
```

- `gpu_targets() -> list[str]` - sorted, unique target IDs (`sm_90a`,
  `compute_120`, `gfx942`, ...).
- `gpu_code_objects() -> list[GpuCodeObject]` - one per embedded code object,
  with `compute_platform` (`"cuda"` / `"hip"`), `target` (`None` for a host entry), and
  `kind`.

Sections are matched by magic (CUDA `.nv_fatbin` and the Clang Offload Bundle in
`.hip_fatbin`), and only the container headers are read, so no device code is
decompressed. The container parsers are the standalone crates
[`cuda-fatbin`](crates/cuda-fatbin) and
[`offload-bundle`](crates/offload-bundle). GPU support is on by default and can
be dropped with the crate's `--no-default-features`.

## Shared-library metadata (ELF)

Beyond the dependency list, `objectfile` exposes the ELF dynamic-linking metadata
needed to generate RPM-style `Provides` / `Requires`, as
[elfdeps](https://github.com/python-wheel-build/elfdeps) does:

```pycon
>>> obj = objectfile.parse_file("libexample.so.1")
>>> obj.link_kind
LinkKind.SharedLibrary
>>> obj.soname
'libexample.so.1'
>>> obj.symbol_hash
SymbolHash.GNU
>>> obj.runpaths()
['$ORIGIN/../lib']
>>> obj.provided_versions()
['LIBEXAMPLE_1.0', 'LIBEXAMPLE_1.1']
>>> obj.required_versions()
{'libc.so.6': ['GLIBC_2.2.5', 'GLIBC_2.3.4'], 'libstdc++.so.6': ['GLIBCXX_3.4']}
```

- `link_kind` tells a shared library from a (PIE) executable - a PIE is `ET_DYN`
  like a `.so`, so `kind` alone cannot distinguish them.
- `soname`, `interpreter`, and `rpaths()` come from ELF `DT_SONAME` / `PT_INTERP`
  / `DT_RPATH` or their Mach-O equivalents (`LC_ID_DYLIB` install name,
  `LC_LOAD_DYLINKER`, `LC_RPATH`); `runpaths()` is ELF `DT_RUNPATH`.
- `provided_versions()` and `required_versions()` are the verdef / verneed version
  aggregates; `required_versions()` maps each needed library to the symbol versions
  it requires.

`soname` / `interpreter` / `rpaths()` work on ELF and Mach-O; `link_kind` works on
every format (from the generic object kind, with the PIE distinction added for
ELF). The symbol-version aggregates, `symbol_hash`, and `runpaths()` are ELF-only,
returning `[]` / `{}` / `SymbolHash.Unknown` elsewhere.

## Command line (unstable)

A small `objectfile` command prints a summary of a file:

```console
$ objectfile /bin/ls
format:       Elf
architecture: X86_64
bits:         64
endianness:   Little
link kind:    PieExecutable
interpreter:  /lib64/ld-linux-x86-64.so.2
symbol hash:  GNU
libraries (1):
  libc.so.6
imports:      128
exports:      0
required versions (1):
  libc.so.6: GLIBC_2.2.5, GLIBC_2.34
```

The summary includes the dependency metadata (soname, interpreter, rpath/runpath,
symbol hash, and the provided/required symbol versions) for ELF and Mach-O; the
version and path sections are shown only when present.

Add `--imports`, `--exports`, `--symbols`, or `--gpu` to list those entries, or
run it as `python -m objectfile <path>`. When a binary ships GPU code, the summary
shows a `gpu targets:` line, and `--gpu` lists each embedded code object:

```console
$ objectfile libtorch_hip.so --gpu
...
gpu targets:  gfx90a, gfx942
gpu code objects (3):
  hip   -                  host
  hip   gfx90a             code-object
  hip   gfx942             code-object
```

With the `cli` extra installed (`pip install objectfile[cli]`, which pulls in
[pycxxfilt](https://pypi.org/project/pycxxfilt/)), `--demangle` renders C++/Rust symbol
names in a readable form.

> **The CLI and its output format are unstable** and may change at any time. Do not
> parse this output in scripts - use the Python API instead.

## API

- `parse(data: bytes, *, path=None) -> ObjectFile` - parse a buffer; `path`
  optionally records a logical source path (for `$ORIGIN`), stored verbatim.
- `parse_file(path) -> ObjectFile` - memory-map a file (`str` or `os.PathLike`) and
  parse it.
- `parse` accepts any buffer-protocol object (`bytes`, `bytearray`, `memoryview`,
  `mmap`, ...).
- `ObjectFile` - properties `path` (absolute source `pathlib.Path`, `None` for a
  buffer without one),
  `format`, `architecture`, `is_64`, `endianness`, `kind`,
  `link_kind`, `soname`, `interpreter`, `symbol_hash`; methods `imports()`,
  `exports()`, `libraries()`, `symbols()`, `dynamic_symbols()` (lazy iterators), and
  `rpaths()`, `runpaths()`, `provided_versions()`, `required_versions()` (ELF
  dependency metadata; see above).
- `Import` - `name`, `ordinal`, `library`, `is_weak`.
- `Export` - `name`, `ordinal`, `address`, `is_weak`, `version`, `version_hidden`.
- `Symbol` - `name`, `address`, `size`, `kind`, `scope`, `is_undefined`, `is_global`,
  `is_weak`, `section_index`, `version`, `version_hidden`.
- `Import`, `Export`, and `Symbol` are comparable, hashable, and ordered by name, so
  `sorted(...)` works; they are also directly constructible.
- Enums: `Format`, `Architecture`, `Endianness`, `ObjectKind`, `SymbolKind`,
  `SymbolScope`, `LinkKind`, `SymbolHash`.

`imports()`/`exports()` are the dynamic-linking tables (what the loader resolves);
`symbols()`/`dynamic_symbols()` are the symbol tables (every symbol) - complementary
views. Invalid input raises `ObjectFileError` (a subclass of `ValueError`); a missing
file raises the usual `OSError` (e.g. `FileNotFoundError`).

### Symbol versions (ELF)

Exports and dynamic symbols carry the GNU symbol version: `.version` (e.g.
`GLIBCXX_3.4.22`) and `.version_hidden` (`False` for a default version - `nm`'s `@@` -
and `True` for a non-default one - `@`). It is set on `exports()` and
`dynamic_symbols()`; the full `symbols()` table (`.symtab`) is unversioned. This is
ELF-specific; Mach-O, PE, and wasm have no equivalent per-symbol versioning, so
`version` is `None` there. ELF version-node marker symbols (the pseudo-symbols named
after a version node) are filtered out of `exports()`, but remain in
`dynamic_symbols()`, which is the raw symbol-table view.

Enum members stringify to their bare name for display/serialization: `str(obj.format)`
is `"Elf"` (while `repr` keeps `Format.Elf`).

## License

Dual-licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option - the same terms as the upstream `object` crate.
