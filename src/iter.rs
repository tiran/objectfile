//! Lazy iterators over an object file's imports, exports, libraries and symbols.
//!
//! Each iterator holds an owned `Vec` of already-extracted entries and yields
//! one Python object per `__next__`. Building the Vec walks the relevant table
//! once (when you call `imports()` / `symbols()` / ...), but the Python objects
//! are created lazily as you iterate.

use pyo3::prelude::*;

use crate::model::{Export, Import, Symbol};

/// Lazy iterator over imported symbols.
#[pyclass(module = "objectfile._objectfile")]
pub struct ImportIter {
    items: std::vec::IntoIter<Import>,
}

impl ImportIter {
    pub(crate) fn new(items: Vec<Import>) -> Self {
        ImportIter {
            items: items.into_iter(),
        }
    }
}

#[pymethods]
impl ImportIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<Import> {
        self.items.next()
    }
}

/// Lazy iterator over exported symbols.
#[pyclass(module = "objectfile._objectfile")]
pub struct ExportIter {
    items: std::vec::IntoIter<Export>,
}

impl ExportIter {
    pub(crate) fn new(items: Vec<Export>) -> Self {
        ExportIter {
            items: items.into_iter(),
        }
    }
}

#[pymethods]
impl ExportIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<Export> {
        self.items.next()
    }
}

/// Lazy iterator over shared-library dependency names.
#[pyclass(module = "objectfile._objectfile")]
pub struct LibraryIter {
    items: std::vec::IntoIter<String>,
}

impl LibraryIter {
    pub(crate) fn new(items: Vec<String>) -> Self {
        LibraryIter {
            items: items.into_iter(),
        }
    }
}

#[pymethods]
impl LibraryIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<String> {
        self.items.next()
    }
}

/// Lazy iterator over symbol-table entries.
#[pyclass(module = "objectfile._objectfile")]
pub struct SymbolIter {
    items: std::vec::IntoIter<Symbol>,
}

impl SymbolIter {
    pub(crate) fn new(items: Vec<Symbol>) -> Self {
        SymbolIter {
            items: items.into_iter(),
        }
    }
}

#[pymethods]
impl SymbolIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<Symbol> {
        self.items.next()
    }
}
