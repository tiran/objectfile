# AGENTS.md

Guidance for AI agents and new contributors working on `objectfile`.

## What this is

A typed Python wrapper (PyO3 + maturin) around the high-level **read** API of the Rust
[`object`](https://crates.io/crates/object) crate. Scope for now: format metadata,
imported/exported symbols, shared-library dependencies, and symbol tables.

## Layout (maturin mixed Rust/Python)

Almost everything lives in the Rust extension; the Python package is a thin re-export.

- `src/` - the Rust extension `objectfile._objectfile`, split by concern:
  - `lib.rs` - module wiring and `#[pymodule]` registration (`gil_used = false`).
  - `enums.rs` - native enums (`Format`, `Architecture`, `Endianness`, `ObjectKind`,
    `SymbolKind`, `SymbolScope`) with `from_object` conversions; catch-all arms map any
    future `#[non_exhaustive]` variant to `Unknown`.
  - `model.rs` - the value types `Import`, `Export`, `Symbol` (frozen, `eq`/`ord`/`hash`,
    constructible) and the builders that turn `object` entries into them.
  - `iter.rs` - the lazy iterators (`ImportIter`, ..., `SymbolIter`).
  - `file.rs` - `Backing` (mmap or owned bytes), the `self_cell` holding the parsed
    `object::File`, the `ObjectFile` type, and `parse` / `parse_file`.
  - `gpu.rs` - GPU fat-binary bridge (behind the default `gpu` feature): scans
    sections and exposes `GpuCodeObject` / `ObjectFile.gpu_targets()`.
- `crates/` - workspace members: pure-Rust, pyo3-free, unit-tested standalone:
  - `cuda-fatbin` - NVIDIA `.nv_fatbin` fat binary parser.
  - `offload-bundle` - AMD HIP / Clang Offload Bundle parser (`.hip_fatbin`,
    incl. CCOB).
  - `gpu-elf-flags` - decode a GPU code object's target (gfx / sm) from the ELF
    `e_machine` + `e_flags` fields; `amdgpu` + `nvptx` modules plus a common
    `Flags` / `GpuTarget` API. Constants pinned to an LLVM release (`LLVM_SOURCE`).
- `python/objectfile/` - `__init__.py` (import-only re-exports), `_cli.py` +
  `__main__.py` (the `objectfile` console script; output is explicitly **unstable**),
  `_objectfile.pyi` (stub for the compiled module), `py.typed`.
- `tests/` - pytest suite. Config in `Cargo.toml` / `pyproject.toml`; CI in `.github/`.

## Key design points

- `parse_file` **memory-maps** the file; `parse` accepts any buffer-protocol object and
  copies it. The parsed file borrows its bytes, bundled via `self_cell` - **no `unsafe`**
  except the standard `memmap2::Mmap::map`.
- Collections (`imports()` etc.) snapshot the relevant table into an owned `Vec`, then
  yield Python objects lazily. Iterators own their data, so they are `Send` and keep
  nothing borrowed - this is what makes free-threading safe.
- Enums are **pyo3-native** (not `enum.StrEnum`). A proper string representation for
  JSON/YAML serialization is a known follow-up.
- `imports`/`exports`/`libraries` are the dynamic-linking view; `symbols`/
  `dynamic_symbols` are the symbol-table view. They come from different `object`
  structures - do not derive one from the other.

## Conventions

- **uv** for everything; work in the project `.venv`.
- After editing Rust, rebuild with `uv sync --reinstall-package objectfile` (a plain
  `uv sync` may not detect Rust-only changes). Pure-Python edits are picked up live.
- All code and docs are **ASCII-only**: no em-dashes, arrows, or other non-ASCII
  characters.
- Imports at module top-level; typing-only imports under `TYPE_CHECKING`. Annotate by
  default (ships `py.typed`).
- The version lives in `Cargo.toml` `[package].version` (maturin reads it; `dynamic` in
  `pyproject.toml`). Tag releases to match.
- Branch for changes; **do not commit or push without the maintainer's OK.**

## Commands

```console
uv sync --reinstall-package objectfile         # build + install the extension
uv run pytest                                  # tests
uv run ruff check python tests                 # lint
uv run ruff format python tests                # format
uv run pyrefly check python/objectfile tests   # type-check (explicit paths, see below)
cargo fmt --all && cargo clippy --all-targets -- -D warnings
uv build && uvx twine check dist/*             # build + inspect sdist/wheel
tox                                            # full matrix
```

> **pyrefly note:** the editable install registers `python/` as a site-packages path,
> which pyrefly auto-excludes. Always pass explicit paths
> (`pyrefly check python/objectfile tests`), as the commands above and CI do.
