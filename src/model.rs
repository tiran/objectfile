//! The per-entry value types: imported/exported symbols and symbol-table
//! entries, plus the helpers that build them from `object`'s types.
//!
//! `eq`, `ord` and `hash` make these comparable, sortable and hashable in
//! Python. Ordering follows field order (name first), so `sorted(...)` sorts by
//! name; a `None` name sorts before any real name.

use std::collections::HashSet;

use pyo3::prelude::*;

use crate::enums::{SymbolKind, SymbolScope};
use object::read::elf::{ElfFile, FileHeader};
use object::read::{Object, ObjectSymbol, ReadRef};

// --- small helpers for building Python-style reprs -------------------------

fn repr_opt_str(value: &Option<String>) -> String {
    match value {
        Some(text) => format!("{text:?}"),
        None => "None".to_string(),
    }
}

fn repr_opt<T: std::fmt::Display>(value: Option<T>) -> String {
    match value {
        Some(number) => number.to_string(),
        None => "None".to_string(),
    }
}

fn py_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

// --- value types -----------------------------------------------------------

/// An imported symbol.
#[pyclass(frozen, eq, ord, hash, module = "objectfile._objectfile")]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Import {
    /// Symbol name, or `None` when imported by ordinal (PE).
    #[pyo3(get)]
    name: Option<String>,
    /// Import ordinal, or `None` when imported by name.
    #[pyo3(get)]
    ordinal: Option<u16>,
    /// Library the symbol is imported from (PE DLL), or `None`.
    #[pyo3(get)]
    library: Option<String>,
    #[pyo3(get)]
    is_weak: bool,
}

#[pymethods]
impl Import {
    #[new]
    #[pyo3(signature = (name=None, ordinal=None, library=None, is_weak=false))]
    fn new(
        name: Option<String>,
        ordinal: Option<u16>,
        library: Option<String>,
        is_weak: bool,
    ) -> Self {
        Import {
            name,
            ordinal,
            library,
            is_weak,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Import(name={}, ordinal={}, library={}, is_weak={})",
            repr_opt_str(&self.name),
            repr_opt(self.ordinal),
            repr_opt_str(&self.library),
            py_bool(self.is_weak),
        )
    }
}

/// An exported symbol.
#[pyclass(frozen, eq, ord, hash, module = "objectfile._objectfile")]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Export {
    /// Symbol name, or `None` when exported by ordinal (PE).
    #[pyo3(get)]
    name: Option<String>,
    /// Export ordinal, or `None` when exported by name.
    #[pyo3(get)]
    ordinal: Option<u16>,
    /// Export address, or `None` for re-exports / forwarders / absolute values.
    #[pyo3(get)]
    address: Option<u64>,
    #[pyo3(get)]
    is_weak: bool,
    /// GNU symbol version (ELF), e.g. ``GLIBCXX_3.4``; ``None`` if unversioned.
    #[pyo3(get)]
    version: Option<String>,
    /// Whether the ELF ``VERSYM_HIDDEN`` bit is set (a non-default version).
    #[pyo3(get)]
    version_hidden: bool,
}

#[pymethods]
impl Export {
    #[new]
    #[pyo3(signature = (
        name=None,
        ordinal=None,
        address=None,
        is_weak=false,
        version=None,
        version_hidden=false,
    ))]
    fn new(
        name: Option<String>,
        ordinal: Option<u16>,
        address: Option<u64>,
        is_weak: bool,
        version: Option<String>,
        version_hidden: bool,
    ) -> Self {
        Export {
            name,
            ordinal,
            address,
            is_weak,
            version,
            version_hidden,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Export(name={}, ordinal={}, address={}, is_weak={}, version={}, version_hidden={})",
            repr_opt_str(&self.name),
            repr_opt(self.ordinal),
            repr_opt(self.address),
            py_bool(self.is_weak),
            repr_opt_str(&self.version),
            py_bool(self.version_hidden),
        )
    }
}

/// An entry from a symbol table.
#[pyclass(frozen, eq, ord, hash, module = "objectfile._objectfile")]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol {
    /// Symbol name, or `None` when unnamed.
    #[pyo3(get)]
    name: Option<String>,
    #[pyo3(get)]
    address: u64,
    #[pyo3(get)]
    size: u64,
    #[pyo3(get)]
    kind: SymbolKind,
    #[pyo3(get)]
    scope: SymbolScope,
    #[pyo3(get)]
    is_undefined: bool,
    #[pyo3(get)]
    is_global: bool,
    #[pyo3(get)]
    is_weak: bool,
    /// Index of the section defining the symbol, or `None`.
    #[pyo3(get)]
    section_index: Option<usize>,
    /// GNU symbol version (ELF dynamic symbols), or `None`. See `Export.version`.
    #[pyo3(get)]
    version: Option<String>,
    /// Whether the ELF `VERSYM_HIDDEN` bit is set (a non-default version).
    #[pyo3(get)]
    version_hidden: bool,
}

#[pymethods]
impl Symbol {
    #[new]
    #[pyo3(signature = (
        name=None,
        address=0,
        size=0,
        kind=SymbolKind::Unknown,
        scope=SymbolScope::Unknown,
        is_undefined=false,
        is_global=false,
        is_weak=false,
        section_index=None,
        version=None,
        version_hidden=false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: Option<String>,
        address: u64,
        size: u64,
        kind: SymbolKind,
        scope: SymbolScope,
        is_undefined: bool,
        is_global: bool,
        is_weak: bool,
        section_index: Option<usize>,
        version: Option<String>,
        version_hidden: bool,
    ) -> Self {
        Symbol {
            name,
            address,
            size,
            kind,
            scope,
            is_undefined,
            is_global,
            is_weak,
            section_index,
            version,
            version_hidden,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Symbol(name={}, address={:#x}, size={}, kind=SymbolKind.{:?}, \
             scope=SymbolScope.{:?}, is_undefined={}, is_global={}, is_weak={}, \
             section_index={}, version={}, version_hidden={})",
            repr_opt_str(&self.name),
            self.address,
            self.size,
            self.kind,
            self.scope,
            py_bool(self.is_undefined),
            py_bool(self.is_global),
            py_bool(self.is_weak),
            repr_opt(self.section_index),
            repr_opt_str(&self.version),
            py_bool(self.version_hidden),
        )
    }
}

// --- builders: `object` entry -> our value type ----------------------------

/// Decode a name that may not be valid UTF-8. An empty name becomes `None`.
pub(crate) fn decode_name(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

pub(crate) fn build_import(import: &object::read::Import<'_>) -> Import {
    let name = import.name();
    Import {
        name: name.name().and_then(decode_name),
        ordinal: name.ordinal(),
        library: decode_name(import.library()),
        is_weak: import.is_weak(),
    }
}

fn build_export(export: &object::read::Export<'_>) -> Export {
    let name = export.name();
    let address = match export.target() {
        object::read::ExportTarget::Address { address } => Some(address),
        _ => None,
    };
    // The GNU symbol version lives in the ELF-specific export flags.
    let (version, version_hidden) = match export.flags() {
        object::read::ExportFlags::Elf {
            version,
            version_hidden,
            ..
        } => (version.and_then(decode_name), version_hidden),
        _ => (None, false),
    };
    Export {
        name: name.name().and_then(decode_name),
        ordinal: name.ordinal(),
        address,
        is_weak: export.is_weak(),
        version,
        version_hidden,
    }
}

/// Build the export list, dropping ELF version-node marker symbols.
///
/// Versioned ELF libraries define an absolute symbol named after each version
/// node (e.g. `GLIBCXX_3.4`, `CXXABI_1.3`). Those are not real exports, so we
/// drop any absolute symbol whose name is itself one of the file's version
/// names. Real symbols keep their `version`, so `foo@V1` and `foo@V2` stay
/// distinct instead of collapsing to a duplicate `foo`.
pub(crate) fn collect_exports<'data>(
    exports: impl Iterator<Item = Result<object::read::Export<'data>, object::Error>>,
) -> Result<Vec<Export>, object::Error> {
    let mut built = Vec::new();
    let mut is_absolute = Vec::new();
    let mut version_names = HashSet::new();

    for export in exports {
        let export = export?;
        let entry = build_export(&export);
        if let Some(version) = &entry.version {
            version_names.insert(version.clone());
        }
        is_absolute.push(matches!(
            export.target(),
            object::read::ExportTarget::Absolute { .. }
        ));
        built.push(entry);
    }

    let kept = built
        .into_iter()
        .zip(is_absolute)
        .filter(|(entry, absolute)| {
            let is_version_marker = *absolute
                && entry
                    .name
                    .as_ref()
                    .is_some_and(|name| version_names.contains(name));
            !is_version_marker
        })
        .map(|(entry, _)| entry)
        .collect();
    Ok(kept)
}

pub(crate) fn build_symbol<'data>(symbol: &impl ObjectSymbol<'data>) -> Symbol {
    Symbol {
        name: symbol.name_bytes().ok().and_then(decode_name),
        address: symbol.address(),
        size: symbol.size(),
        kind: SymbolKind::from_object(symbol.kind()),
        scope: SymbolScope::from_object(symbol.scope()),
        is_undefined: symbol.is_undefined(),
        is_global: symbol.is_global(),
        is_weak: symbol.is_weak(),
        section_index: symbol.section_index().map(|index| index.0),
        // Versions come from a side table and are filled in per format; the
        // generic symbol has none (see `collect_elf_dynamic_symbols`).
        version: None,
        version_hidden: false,
    }
}

/// Build the dynamic symbol table for an ELF file, attaching GNU symbol versions.
///
/// The generic `Object` trait does not expose versions, so this reads the ELF
/// `.gnu.version` table directly. Only the dynamic symbol table is versioned;
/// the regular symbol table (`symbols()`) has no version information.
pub(crate) fn collect_elf_dynamic_symbols<'data, Elf, R>(
    elf: &ElfFile<'data, Elf, R>,
) -> Result<Vec<Symbol>, object::Error>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let endian = elf.endian();
    let versions = elf.elf_section_table().versions(endian, elf.data())?;

    let mut items = Vec::new();
    for symbol in elf.dynamic_symbols() {
        let mut entry = build_symbol(&symbol);
        if let Some(table) = versions.as_ref() {
            let versym = table.version_index(endian, symbol.index());
            if let Some(version) = table.version(versym.index())? {
                entry.version = decode_name(version.name());
                // Same VERSYM_HIDDEN bit that `Export.version_hidden` reports.
                entry.version_hidden = versym.is_hidden();
            }
        }
        items.push(entry);
    }
    Ok(items)
}
