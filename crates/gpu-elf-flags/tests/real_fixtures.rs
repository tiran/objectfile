//! Cross-check `gpu-elf-flags` against the real GPU fixtures copied from elfgpu.
//!
//! - **AMD**: the `.hip_fatbin` CCOB block is fully decompressed here, the device
//!   ELF code objects are read, and each one's decoded `e_flags` target is
//!   checked against the offload bundle's own target string.
//! - **NVIDIA**: the fixture's cubins are LZ4-compressed, so their raw `e_flags`
//!   are not readable. Instead the real SM set (and which entries are
//!   accelerator variants) is taken from the fat binary, re-encoded into
//!   `e_flags` per the LLVM layout, and the decoder is checked against it. This
//!   exercises the pre-/post-Blackwell boundary with the targets a real build
//!   ships (`sm_75` ... `sm_120a`).

use gpu_elf_flags::{nvptx, Flags, GpuTarget, Vendor, EM_AMDGPU, EM_CUDA};
use object::{Object, ObjectSection};
use std::io::Read;
use std::path::PathBuf;

fn fixture(rel: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The bytes of the first section whose data matches `magic`.
fn section_with_magic(elf: &[u8], magic: fn(&[u8]) -> bool) -> Vec<u8> {
    let file = object::File::parse(elf).expect("parse host ELF");
    file.sections()
        .find_map(|section| {
            let data = section.data().ok()?;
            magic(data).then(|| data.to_vec())
        })
        .expect("no matching section")
}

/// Read `(e_machine, e_flags)` straight from an ELF code-object header. GPU code
/// objects are little-endian; `e_flags` is at 0x30 (ELF64) / 0x24 (ELF32).
fn elf_header(payload: &[u8]) -> Option<(u16, u32)> {
    if payload.get(0..4)? != b"\x7fELF" || *payload.get(5)? != 1 {
        return None;
    }
    let is64 = *payload.get(4)? == 2;
    let e_machine = u16::from_le_bytes([*payload.get(18)?, *payload.get(19)?]);
    let off = if is64 { 0x30 } else { 0x24 };
    let bytes = payload.get(off..off + 4)?;
    Some((
        e_machine,
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
    ))
}

/// Decompress a single-block `.hip_fatbin` section into its plain
/// `__CLANG_OFFLOAD_BUNDLE__` bytes (the fixtures are one CCOB v3/zstd block).
fn plain_bundle(section: &[u8]) -> Vec<u8> {
    if section.starts_with(offload_bundle::BUNDLE_MAGIC) {
        return section.to_vec();
    }
    assert!(
        section.starts_with(offload_bundle::CCOB_MAGIC),
        "section is neither a plain bundle nor a CCOB block"
    );
    let version = u16::from_le_bytes([section[4], section[5]]);
    let method = u16::from_le_bytes([section[6], section[7]]);
    let header_len = match version {
        1 => 20,
        2 => 24,
        3 => 32,
        other => panic!("unsupported CCOB version {other}"),
    };
    let payload = &section[header_len..];
    let mut out = Vec::new();
    match method {
        0 => flate2::read::ZlibDecoder::new(payload)
            .read_to_end(&mut out)
            .map(|_| ())
            .expect("zlib"),
        1 => ruzstd::decoding::StreamingDecoder::new(payload)
            .expect("zstd init")
            .read_to_end(&mut out)
            .map(|_| ())
            .expect("zstd"),
        other => panic!("unsupported CCOB method {other}"),
    }
    out
}

/// Every device ELF in the real HIP bundle decodes to the gfx target the
/// bundle claims for it (including the sramecc/xnack qualifiers).
#[test]
fn amd_eflags_match_bundle_targets() {
    for rel in ["rocm/hello.so", "rocm-compressed/hello.so"] {
        let section = section_with_magic(&fixture(rel), offload_bundle::has_magic);
        let bundle = plain_bundle(&section);
        let entries = offload_bundle::parse(&bundle).unwrap();

        let mut checked = 0;
        for entry in &entries {
            let Some(target) = entry.target() else {
                continue; // host entry
            };
            let payload = bundle
                .get(entry.offset as usize..(entry.offset + entry.size) as usize)
                .unwrap_or_else(|| panic!("{rel}: payload out of range for {}", entry.id));
            let (machine, e_flags) =
                elf_header(payload).unwrap_or_else(|| panic!("{rel}: {} is not ELF", entry.id));

            assert_eq!(machine, EM_AMDGPU, "{rel}: {}", entry.id);
            let decoded = Flags::decode(machine, e_flags).unwrap();
            assert_eq!(decoded.vendor(), Vendor::Amd);
            assert_eq!(
                decoded.target().as_deref(),
                Some(target),
                "{rel}: e_flags {e_flags:#x} for {}",
                entry.id
            );
            checked += 1;
        }
        assert_eq!(checked, 5, "{rel}: expected 5 device code objects");
    }
}

/// Re-encode `e_flags` for an SM the way a toolchain does: pre-Blackwell keeps
/// the SM in the low byte, Blackwell+ (`sm_100`+) in bits 8-15.
fn encode_sm(sm: u32, accelerated: bool) -> u32 {
    if sm >= 100 {
        (sm << nvptx::EF_CUDA_SM_OFFSET)
            | if accelerated {
                nvptx::EF_CUDA_ACCELERATORS
            } else {
                0
            }
    } else {
        sm | if accelerated {
            nvptx::EF_CUDA_ACCELERATORS_V1
        } else {
            0
        }
    }
}

/// The decoder reproduces every SASS target the real CUDA fat binary ships,
/// across the pre-/post-Blackwell encoding boundary.
#[test]
fn nvidia_decoder_matches_fatbin_sm_set() {
    let section = section_with_magic(&fixture("cuda/hello.so"), cuda_fatbin::has_magic);
    let entries = cuda_fatbin::parse(&section).unwrap();

    let mut sass = 0;
    let mut saw_blackwell = false;
    let mut saw_accelerated = false;
    for entry in &entries {
        if entry.kind != cuda_fatbin::EntryKind::Elf {
            continue; // PTX -> compute_*, handled by cuda-fatbin
        }
        let e_flags = encode_sm(entry.sm_version, entry.arch_specific);
        let decoded = Flags::decode(EM_CUDA, e_flags).unwrap();
        assert_eq!(decoded.vendor(), Vendor::Nvidia);
        assert_eq!(
            decoded.target(),
            Some(entry.target()),
            "sm {} arch_specific={}",
            entry.sm_version,
            entry.arch_specific
        );

        let nv = nvptx::decode(e_flags);
        assert_eq!(nv.sm, entry.sm_version);
        assert_eq!(nv.accelerated, entry.arch_specific);
        saw_blackwell |= nv.new_abi;
        saw_accelerated |= nv.accelerated;
        sass += 1;
    }

    assert!(sass >= 5, "expected several cubin entries, got {sass}");
    assert!(
        saw_blackwell,
        "fixture should include a Blackwell (sm_100+) target"
    );
    assert!(
        saw_accelerated,
        "fixture should include an accelerator (sm_90a) target"
    );
}
