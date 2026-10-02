//! Parse the `.nv_fatbin` section of the real ELF fixtures copied from elfgpu.

use object::{Object, ObjectSection};
use std::path::PathBuf;

/// Extract the `.nv_fatbin` section bytes (matched by magic) from an ELF fixture.
fn nv_fatbin(rel: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let file = object::File::parse(&*data).expect("parse ELF");
    file.sections()
        .find_map(|section| {
            let bytes = section.data().ok()?;
            cuda_fatbin::has_magic(bytes).then(|| bytes.to_vec())
        })
        .expect("no CUDA fat binary section")
}

fn sorted_targets(data: &[u8]) -> Vec<String> {
    let mut targets: Vec<String> = cuda_fatbin::parse(data)
        .unwrap()
        .iter()
        .map(cuda_fatbin::Entry::target)
        .collect();
    targets.sort();
    targets.dedup();
    targets
}

const EXPECTED: [&str; 8] = [
    "compute_120",
    "sm_100",
    "sm_120",
    "sm_120a",
    "sm_75",
    "sm_80",
    "sm_90",
    "sm_90a",
];

#[test]
fn cuda_uncompressed() {
    assert_eq!(sorted_targets(&nv_fatbin("cuda/hello.so")), EXPECTED);
}

#[test]
fn cuda_compressed() {
    assert_eq!(
        sorted_targets(&nv_fatbin("cuda-compressed/hello.so")),
        EXPECTED
    );
}
