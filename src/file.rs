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
use std::path::PathBuf;

use crate::enums::{Architecture, Endianness, Format, ObjectKind};
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
type ParsedFile<'a> = object::File<'a, &'a [u8]>;

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
#[pyclass(frozen, module = "objectfile._objectfile")]
pub struct ObjectFile {
    cell: FileCell,
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

fn build(backing: Backing) -> Result<ObjectFile, object::Error> {
    let cell = FileCell::try_new(backing, |backing| object::File::parse(backing.as_ref()))?;
    Ok(ObjectFile { cell })
}

/// Parse an object file from any object supporting the buffer protocol
/// (`bytes`, `bytearray`, `memoryview`, `mmap`, ...). The bytes are copied into
/// an owned buffer; use `parse_file` to avoid copying large files.
#[pyfunction]
pub(crate) fn parse(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<ObjectFile> {
    let buffer = PyBuffer::<u8>::get(data)?;
    let bytes = buffer.to_vec(py)?;
    // Parsing touches no Python state, so release the GIL while we do it.
    let obj = py
        .detach(|| build(Backing::Bytes(bytes)))
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
        // An empty file cannot be memory-mapped; use an empty buffer so the
        // caller gets a clean parse error rather than an OSError.
        if file.metadata()?.len() == 0 {
            return Ok(build(Backing::Bytes(Vec::new()))?);
        }
        // Mapping is unsafe in general (another process could change the file
        // while it is mapped); this is the standard, accepted trade-off for mmap.
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(build(Backing::Mmap(mmap))?)
    })?;
    Ok(obj)
}
