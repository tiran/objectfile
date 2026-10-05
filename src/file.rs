//! The parsed object file and the `parse` / `parse_file` entry points.
//!
//! `parse_file` memory-maps the file (via `memmap2`) instead of reading it into
//! memory, so large binaries only page in what is actually touched. The parsed
//! `object::File` borrows the mapped bytes; `self_cell` bundles the two together
//! safely, with no `unsafe` on our side.

use object::read::Object;
use pyo3::buffer::PyBuffer;
use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use self_cell::self_cell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::enums::{Architecture, Endianness, Format, LinkKind, ObjectKind, SymbolHash};
use crate::iter::{ExportIter, ImportIter, LibraryIter, SymbolIter};
use crate::model::{
    build_import, build_symbol, collect_elf_dynamic_symbols, collect_exports, decode_name,
};

// Raised when parsing fails. Subclasses ValueError so `except ValueError` keeps
// working, while callers can also catch this type specifically.
create_exception!(
    objectfile,
    ObjectFileError,
    PyValueError,
    "Raised when an object file cannot be parsed."
);

/// The bytes a parsed object file reads from: either a memory map or an owned
/// buffer (used for `parse(bytes)` and for empty files, which cannot be mapped).
enum Backing {
    Mmap(memmap2::Mmap),
    Bytes(Vec<u8>),
}

impl AsRef<[u8]> for Backing {
    fn as_ref(&self) -> &[u8] {
        match self {
            Backing::Mmap(mmap) => mmap,
            Backing::Bytes(bytes) => bytes,
        }
    }
}

/// The parsed file. Its lifetime `'a` ties it to the `Backing` it reads from.
pub(crate) type ParsedFile<'a> = object::File<'a, &'a [u8]>;

// `FileCell` owns the `Backing` and the `ParsedFile` that borrows it, keeping
// the two together for as long as the Python object lives.
self_cell!(
    struct FileCell {
        owner: Backing,
        #[covariant]
        dependent: ParsedFile,
    }
);

/// A parsed object file: owns its (memory-mapped) bytes and the parsed file.
///
/// The format metadata and symbol/dependency tables work across all formats.
/// The dynamic-linking accessors are format-specific:
///
/// | accessor                                     | ELF | Mach-O | PE / COFF / wasm |
/// | -------------------------------------------- | --- | ------ | ---------------- |
/// | `soname`, `interpreter`, `rpaths()`          | yes | yes    | no               |
/// | `link_kind`                                  | yes | yes    | yes (no PIE)     |
/// | `runpaths()`, `symbol_hash`,                 | yes | no     | no               |
/// | `provided_versions()`, `required_versions()` | yes | no     | no               |
///
/// Where a field does not apply, a scalar returns `None`, a collection returns an
/// empty list/dict, and `symbol_hash`/`link_kind` return their `Unknown` member.
/// An absent value and an unsupported format are reported the same way (as with
/// `Symbol.version`); check `format` if you need to tell them apart. These reads
/// use the ELF section headers / Mach-O load commands, so a binary stripped of its
/// section-header table (uncommon; plain `strip` keeps them) reports empties.
#[pyclass(frozen, module = "objectfile._objectfile")]
pub struct ObjectFile {
    cell: FileCell,
    // The absolute, symlink-resolved path for `parse_file`; `None` for `parse`.
    path: Option<PathBuf>,
}

impl ObjectFile {
    /// Borrow the parsed file. Safe: `self_cell` guarantees the bytes it
    /// borrows from live exactly as long as `self`.
    fn file(&self) -> &ParsedFile<'_> {
        self.cell.borrow_dependent()
    }
}

#[pymethods]
impl ObjectFile {
    /// The absolute, symlink-resolved path this file was parsed from, or `None`
    /// for `parse(bytes)`. Handy for resolving ELF `$ORIGIN` in rpath/runpath.
    #[getter]
    fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }

    #[getter]
    fn format(&self) -> Format {
        Format::from_object(self.file().format())
    }

    #[getter]
    fn architecture(&self) -> Architecture {
        Architecture::from_object(self.file().architecture())
    }

    #[getter]
    fn is_64(&self) -> bool {
        self.file().is_64()
    }

    #[getter]
    fn endianness(&self) -> Endianness {
        Endianness::from_object(self.file().endianness())
    }

    #[getter]
    fn kind(&self) -> ObjectKind {
        ObjectKind::from_object(self.file().kind())
    }

    /// Link kind, distinguishing a PIE executable from a shared library (ELF).
    #[getter]
    fn link_kind(&self) -> LinkKind {
        crate::dynamic::link_kind(self.file())
    }

    /// The object's own library name: ELF `DT_SONAME` or Mach-O install name
    /// (`LC_ID_DYLIB`). `None` if unset.
    #[getter]
    fn soname(&self) -> Option<String> {
        crate::dynamic::soname(self.file())
    }

    /// The program interpreter / dynamic loader: ELF `PT_INTERP` or Mach-O
    /// `LC_LOAD_DYLINKER`. `None` if absent.
    #[getter]
    fn interpreter(&self) -> Option<String> {
        crate::dynamic::interpreter(self.file())
    }

    /// Which symbol hash table(s) the object carries (ELF `DT_HASH`/`DT_GNU_HASH`).
    #[getter]
    fn symbol_hash(&self) -> SymbolHash {
        crate::dynamic::symbol_hash(self.file())
    }

    /// Library search paths: ELF `DT_RPATH` or Mach-O `LC_RPATH`.
    fn rpaths(&self) -> Vec<String> {
        crate::dynamic::rpaths(self.file())
    }

    /// `DT_RUNPATH`: the library search paths (ELF only).
    fn runpaths(&self) -> Vec<String> {
        crate::dynamic::runpaths(self.file())
    }

    /// Symbol version names this object defines (ELF verdef), base excluded.
    fn provided_versions(&self) -> Vec<String> {
        crate::dynamic::provided_versions(self.file())
    }

    /// Required symbol versions per needed library (ELF verneed):
    /// `{soname: [version, ...]}`.
    fn required_versions(&self) -> BTreeMap<String, Vec<String>> {
        crate::dynamic::required_versions(self.file())
    }

    /// Iterate imported symbols (the dynamic-linking import table).
    fn imports(&self) -> PyResult<ImportIter> {
        let mut items = Vec::new();
        for import in self.file().imports().map_err(parse_error)? {
            let import = import.map_err(parse_error)?;
            items.push(build_import(&import));
        }
        Ok(ImportIter::new(items))
    }

    /// Iterate exported symbols (the dynamic-linking export table).
    ///
    /// ELF version-node marker symbols are dropped; see `collect_exports`.
    fn exports(&self) -> PyResult<ExportIter> {
        let iter = self.file().exports().map_err(parse_error)?;
        let items = collect_exports(iter).map_err(parse_error)?;
        Ok(ExportIter::new(items))
    }

    /// Iterate shared-library dependency names (DT_NEEDED / dylibs / DLLs).
    fn libraries(&self) -> PyResult<LibraryIter> {
        let mut items = Vec::new();
        for library in self.file().import_libraries().map_err(parse_error)? {
            let library = library.map_err(parse_error)?;
            if let Some(name) = decode_name(library.name()) {
                items.push(name);
            }
        }
        Ok(LibraryIter::new(items))
    }

    /// Iterate the full symbol table.
    fn symbols(&self) -> SymbolIter {
        let mut items = Vec::new();
        for symbol in self.file().symbols() {
            items.push(build_symbol(&symbol));
        }
        SymbolIter::new(items)
    }

    /// Iterate the dynamic symbol table.
    ///
    /// For ELF files this attaches the GNU symbol version to each entry; other
    /// formats have no per-symbol versioning. Unlike `exports()`, this is the raw
    /// table view: ELF version-node marker symbols (e.g. `GLIBCXX_3.4`) are *not*
    /// filtered out here.
    fn dynamic_symbols(&self) -> PyResult<SymbolIter> {
        let file = self.file();
        let items = match file {
            object::File::Elf32(elf) => collect_elf_dynamic_symbols(elf).map_err(parse_error)?,
            object::File::Elf64(elf) => collect_elf_dynamic_symbols(elf).map_err(parse_error)?,
            _ => {
                let mut items = Vec::new();
                for symbol in file.dynamic_symbols() {
                    items.push(build_symbol(&symbol));
                }
                items
            }
        };
        Ok(SymbolIter::new(items))
    }

    /// Embedded GPU code objects (CUDA cubins/PTX and AMD HIP code objects).
    #[cfg(feature = "gpu")]
    fn gpu_code_objects(&self) -> Vec<crate::gpu::GpuCodeObject> {
        crate::gpu::code_objects(self.file())
    }

    /// Sorted, unique GPU target names (e.g. `sm_90a`, `gfx942`).
    #[cfg(feature = "gpu")]
    fn gpu_targets(&self) -> Vec<String> {
        crate::gpu::targets(self.file())
    }
}

/// Turn an `object` parse error into our Python exception. Used by the
/// `ObjectFile` methods, whose errors are always parse failures.
fn parse_error(err: object::Error) -> PyErr {
    ObjectFileError::new_err(format!("failed to parse object: {err}"))
}

/// Error from the parse entry points: a filesystem error (mapped to the matching
/// `OSError`, e.g. `FileNotFoundError`) or a parse failure (`ObjectFileError`).
enum ParseError {
    Io(std::io::Error),
    Object(object::Error),
}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        ParseError::Io(err)
    }
}

impl From<object::Error> for ParseError {
    fn from(err: object::Error) -> Self {
        ParseError::Object(err)
    }
}

impl From<ParseError> for PyErr {
    fn from(err: ParseError) -> Self {
        match err {
            ParseError::Io(err) => err.into(),
            ParseError::Object(err) => parse_error(err),
        }
    }
}

fn build(backing: Backing, path: Option<PathBuf>) -> Result<ObjectFile, object::Error> {
    let cell = FileCell::try_new(backing, |backing| object::File::parse(backing.as_ref()))?;
    Ok(ObjectFile { cell, path })
}

/// Parse an object file from any object supporting the buffer protocol
/// (`bytes`, `bytearray`, `memoryview`, `mmap`, ...). The bytes are copied into
/// an owned buffer; use `parse_file` to avoid copying large files.
///
/// The keyword-only `path` optionally records a logical source path (exposed as
/// `ObjectFile.path`, e.g. for resolving ELF `$ORIGIN`). Unlike `parse_file` it
/// is stored verbatim and need not exist on disk.
#[pyfunction]
#[pyo3(signature = (data, *, path=None))]
pub(crate) fn parse(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    path: Option<PathBuf>,
) -> PyResult<ObjectFile> {
    let buffer = PyBuffer::<u8>::get(data)?;
    let bytes = buffer.to_vec(py)?;
    // Parsing touches no Python state, so release the GIL while we do it.
    let obj = py
        .detach(|| build(Backing::Bytes(bytes), path))
        .map_err(parse_error)?;
    Ok(obj)
}

/// Memory-map `path` and parse it as an object file.
#[pyfunction]
pub(crate) fn parse_file(py: Python<'_>, path: PathBuf) -> PyResult<ObjectFile> {
    // Opening, mapping and parsing touch no Python state; release the GIL.
    let obj = py.detach(|| -> Result<ObjectFile, ParseError> {
        // Opening maps OS errors to Python (e.g. FileNotFoundError).
        let file = std::fs::File::open(&path)?;
        // Store the absolute, symlink-resolved path (what ELF `$ORIGIN` uses);
        // fall back to the path as given if it cannot be canonicalized.
        let resolved = std::fs::canonicalize(&path).unwrap_or(path);
        // An empty file cannot be memory-mapped; use an empty buffer so the
        // caller gets a clean parse error rather than an OSError.
        if file.metadata()?.len() == 0 {
            return Ok(build(Backing::Bytes(Vec::new()), Some(resolved))?);
        }
        // Mapping is unsafe in general (another process could change the file
        // while it is mapped); this is the standard, accepted trade-off for mmap.
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(build(Backing::Mmap(mmap), Some(resolved))?)
    })?;
    Ok(obj)
}
