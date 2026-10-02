//! Parse the `.hip_fatbin` section of the real ELF fixtures copied from elfgpu.

use object::{Object, ObjectSection};
use std::path::PathBuf;

/// Extract the offload-bundle section bytes (matched by magic) from an ELF fixture.
fn hip_fatbin(rel: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let file = object::File::parse(&*data).expect("parse ELF");
    file.sections()
        .find_map(|section| {
            let bytes = section.data().ok()?;
            offload_bundle::has_magic(bytes).then(|| bytes.to_vec())
        })
        .expect("no offload-bundle section")
}

fn sorted_targets(data: &[u8]) -> Vec<String> {
    let entries = offload_bundle::parse(data).unwrap();
    let mut targets: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.target().map(str::to_owned))
        .collect();
    targets.sort();
    targets.dedup();
    targets
}

const EXPECTED: [&str; 5] = [
    "gfx1100",
    "gfx90a:sramecc+:xnack+",
    "gfx90a:sramecc+:xnack-",
    "gfx942",
    "gfx950",
];

#[test]
fn rocm_uncompressed() {
    let data = hip_fatbin("rocm/hello.so");
    let entries = offload_bundle::parse(&data).unwrap();
    assert_eq!(entries.len(), 6); // host + 5 device
    assert!(entries.iter().any(offload_bundle::Entry::is_host));
    assert_eq!(sorted_targets(&data), EXPECTED);
}

#[test]
fn rocm_compressed_ccob() {
    // This fixture's .hip_fatbin uses Zstd-compressed CCOB blocks.
    assert_eq!(
        sorted_targets(&hip_fatbin("rocm-compressed/hello.so")),
        EXPECTED
    );
}
