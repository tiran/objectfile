//! PyO3 bindings for the high-level read API of the Rust `object` crate.
//!
//! This is the private extension module `objectfile._objectfile`; the public
//! Python package just re-exports the names registered here. The code is split
//! into focused modules:
//!
//! * [`enums`]  - the native enums (format, architecture, symbol kind, ...).
//! * [`model`]  - the value types `Import`, `Export`, `Symbol`.
//! * [`iter`]   - the lazy iterators returned by `ObjectFile`.
//! * [`file`]   - the `ObjectFile` type and the `parse` / `parse_file` functions.

mod enums;
mod file;
#[cfg(feature = "gpu")]
mod gpu;
mod iter;
mod model;

use pyo3::prelude::*;

/// `gil_used = false` declares this module safe to run without the GIL, so a
/// free-threaded interpreter will not re-enable the GIL when it is imported.
/// Everything exposed here is immutable, read-only data, which is safe to share.
#[pymodule(gil_used = false)]
fn _objectfile(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<enums::Format>()?;
    m.add_class::<enums::Architecture>()?;
    m.add_class::<enums::Endianness>()?;
    m.add_class::<enums::ObjectKind>()?;
    m.add_class::<enums::SymbolKind>()?;
    m.add_class::<enums::SymbolScope>()?;

    m.add_class::<model::Import>()?;
    m.add_class::<model::Export>()?;
    m.add_class::<model::Symbol>()?;

    m.add_class::<iter::ImportIter>()?;
    m.add_class::<iter::ExportIter>()?;
    m.add_class::<iter::LibraryIter>()?;
    m.add_class::<iter::SymbolIter>()?;

    m.add_class::<file::ObjectFile>()?;

    #[cfg(feature = "gpu")]
    m.add_class::<gpu::GpuCodeObject>()?;

    m.add(
        "ObjectFileError",
        m.py().get_type::<file::ObjectFileError>(),
    )?;

    m.add_function(wrap_pyfunction!(file::parse, m)?)?;
    m.add_function(wrap_pyfunction!(file::parse_file, m)?)?;
    Ok(())
}
