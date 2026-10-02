//! GPU fat-binary support: find embedded NVIDIA CUDA and AMD HIP code objects.
//!
//! This bridges the pure-Rust `cuda-fatbin` and `offload-bundle` crates to
//! Python. It scans the object's sections for the fat-binary magic (rather than
//! trusting section names) and reports one [`GpuCodeObject`] per embedded code
//! object. Only container headers are read, so no GPU code is decompressed.

use object::read::{Object, ObjectSection};
use pyo3::prelude::*;

use crate::file::ParsedFile;

/// One embedded GPU code object (a CUDA cubin/PTX or an AMD code object).
#[pyclass(frozen, eq, skip_from_py_object, module = "objectfile._objectfile")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuCodeObject {
    /// The compute platform: `"cuda"` or `"hip"`.
    #[pyo3(get)]
    compute_platform: String,
    /// Target architecture: `sm_90a` / `compute_120` (CUDA) or `gfx942` (HIP).
    /// `None` for an AMD host entry.
    #[pyo3(get)]
    target: Option<String>,
    /// Payload kind: `"ptx"`, `"elf"`, `"old-cubin"`, `"ir"` (CUDA) or
    /// `"code-object"` / `"host"` (HIP).
    #[pyo3(get)]
    kind: String,
}

#[pymethods]
impl GpuCodeObject {
    fn __repr__(&self) -> String {
        format!(
            "GpuCodeObject(compute_platform={:?}, target={:?}, kind={:?})",
            self.compute_platform, self.target, self.kind
        )
    }
}

fn cuda_kind(kind: cuda_fatbin::EntryKind) -> String {
    match kind {
        cuda_fatbin::EntryKind::Ptx => "ptx",
        cuda_fatbin::EntryKind::Elf => "elf",
        cuda_fatbin::EntryKind::OldCubin => "old-cubin",
        cuda_fatbin::EntryKind::Ir => "ir",
        cuda_fatbin::EntryKind::Other(_) => "other",
    }
    .to_owned()
}

/// Append the code objects found in one section's bytes (if it is a fat binary).
fn append_from_section(data: &[u8], out: &mut Vec<GpuCodeObject>) {
    if cuda_fatbin::has_magic(data) {
        if let Ok(entries) = cuda_fatbin::parse(data) {
            out.extend(entries.into_iter().map(|entry| GpuCodeObject {
                compute_platform: "cuda".to_owned(),
                target: Some(entry.target()),
                kind: cuda_kind(entry.kind),
            }));
        }
    } else if offload_bundle::has_magic(data) {
        if let Ok(entries) = offload_bundle::parse(data) {
            out.extend(entries.into_iter().map(|entry| {
                let kind = if entry.is_host() {
                    "host"
                } else {
                    "code-object"
                };
                GpuCodeObject {
                    compute_platform: "hip".to_owned(),
                    target: entry.target().map(str::to_owned),
                    kind: kind.to_owned(),
                }
            }));
        }
    }
}

/// All embedded GPU code objects across the file's sections.
pub(crate) fn code_objects(file: &ParsedFile) -> Vec<GpuCodeObject> {
    let mut out = Vec::new();
    for section in file.sections() {
        if let Ok(data) = section.data() {
            append_from_section(data, &mut out);
        }
    }
    out
}

/// Sorted, de-duplicated GPU target names (host entries excluded).
pub(crate) fn targets(file: &ParsedFile) -> Vec<String> {
    let mut targets: Vec<String> = code_objects(file)
        .into_iter()
        .filter_map(|object| object.target)
        .collect();
    targets.sort();
    targets.dedup();
    targets
}
