//! Parser for the Clang Offload Bundle format used by AMD HIP/ROCm in the
//! `.hip_fatbin` ELF section (also used by OpenMP offload).
//!
//! A section holds one or more blocks. A block is either a plain bundle
//! (magic `__CLANG_OFFLOAD_BUNDLE__`) or a **compressed** block (magic `CCOB`)
//! whose Zstd/Zlib payload decompresses to a plain bundle. A plain bundle is a
//! count followed by entries, each `(offset, size, id)` where `id` is a target
//! string like `hipv4-amdgcn-amd-amdhsa--gfx90a`.
//!
//! Only the bundle *headers* are needed to list architectures, so compressed
//! blocks are decompressed just far enough to read them (see [`HEADER_CAP`]),
//! never materialising the (potentially GiB) code objects.

#![forbid(unsafe_code)]

use std::fmt;
use std::io::Read;

/// Magic at the start of an uncompressed bundle.
pub const BUNDLE_MAGIC: &[u8] = b"__CLANG_OFFLOAD_BUNDLE__";
/// Magic at the start of a compressed (CCOB) block.
pub const CCOB_MAGIC: &[u8] = b"CCOB";

/// How far to decompress each CCOB block: enough for the bundle header and
/// entry records (hundreds of entries), far short of the code objects.
pub const HEADER_CAP: usize = 64 * 1024;

/// One bundle entry (a host or device code object).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Bundle ID / target string, e.g. `hipv4-amdgcn-amd-amdhsa--gfx90a`.
    pub id: String,
    /// Offset of the code object within its (decompressed) bundle.
    pub offset: u64,
    /// Size of the code object in bytes (0 for empty host entries).
    pub size: u64,
}

impl Entry {
    /// True for the host bundle (`host-...`), which has no GPU target.
    pub fn is_host(&self) -> bool {
        self.id.starts_with("host-") || self.id.starts_with("host_")
    }

    /// GPU target ID (the part after the first `--`), e.g. `gfx90a` or
    /// `gfx90a:sramecc+:xnack-`. `None` for host entries.
    pub fn target(&self) -> Option<&str> {
        if self.is_host() {
            return None;
        }
        self.id.split_once("--").map(|(_, target)| target)
    }
}

/// An error parsing an offload bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A CCOB header ran past the end of the data.
    Truncated,
    /// Unknown CCOB header version.
    UnsupportedVersion(u16),
    /// Unknown CCOB compression method (not Zlib=0 / Zstd=1).
    UnsupportedCompression(u16),
    /// Decompression failed.
    Decompress(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => f.write_str("truncated offload bundle"),
            Error::UnsupportedVersion(v) => write!(f, "unsupported CCOB version {v}"),
            Error::UnsupportedCompression(m) => {
                write!(f, "unsupported CCOB compression method {m}")
            }
            Error::Decompress(msg) => write!(f, "CCOB decompression failed: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// True if `data` starts with either bundle magic.
pub fn has_magic(data: &[u8]) -> bool {
    data.starts_with(BUNDLE_MAGIC) || data.starts_with(CCOB_MAGIC)
}

/// Parse all bundle entries from a `.hip_fatbin` section.
///
/// Handles a section containing a single plain bundle, or one or more CCOB
/// blocks concatenated with zero padding (as produced for large libraries).
pub fn parse(section: &[u8]) -> Result<Vec<Entry>, Error> {
    let mut entries = Vec::new();
    let mut offset = 0usize;

    while offset < section.len() {
        let rest = &section[offset..];
        if rest.starts_with(CCOB_MAGIC) {
            let (bundle, consumed) = decompress_ccob(section, offset)?;
            parse_plain_bundle(&bundle, &mut entries);
            if consumed == 0 {
                break; // guard against a bogus header
            }
            // `consumed` comes from an untrusted fileSize; saturate so a bogus
            // value simply ends the scan instead of overflowing.
            offset = offset.saturating_add(consumed);
            // Blocks are separated by zero padding.
            while offset < section.len() && section[offset] == 0 {
                offset += 1;
            }
        } else if rest.starts_with(BUNDLE_MAGIC) {
            parse_plain_bundle(rest, &mut entries);
            break;
        } else {
            break;
        }
    }

    Ok(entries)
}

/// Parse a plain `__CLANG_OFFLOAD_BUNDLE__` and append its entries.
///
/// Tolerant of truncation: when the data was only partially decompressed (the
/// common case), it stops at the first entry it cannot read in full.
fn parse_plain_bundle(data: &[u8], out: &mut Vec<Entry>) {
    if !data.starts_with(BUNDLE_MAGIC) {
        return;
    }
    let count = match read_u64(data, BUNDLE_MAGIC.len()) {
        Some(n) => n,
        None => return,
    };
    let mut pos = BUNDLE_MAGIC.len() + 8;
    for _ in 0..count {
        let (Some(offset), Some(size), Some(id_len)) = (
            read_u64(data, pos),
            read_u64(data, pos + 8),
            read_u64(data, pos + 16),
        ) else {
            break;
        };
        let id_start = pos + 24;
        let Some(id_bytes) = id_len
            .try_into()
            .ok()
            .and_then(|len: usize| id_start.checked_add(len)) // untrusted idLength
            .and_then(|end| data.get(id_start..end))
        else {
            break;
        };
        out.push(Entry {
            id: String::from_utf8_lossy(id_bytes).into_owned(),
            offset,
            size,
        });
        pos = id_start + id_bytes.len();
    }
}

/// Decompress one CCOB block at `offset`, returning `(bundle, consumed)` where
/// `consumed` is the block size so the caller can advance to the next block.
fn decompress_ccob(section: &[u8], offset: usize) -> Result<(Vec<u8>, usize), Error> {
    let data = &section[offset..];
    // magic(4) + version(u16) + method(u16)
    let version = read_u16(data, 4).ok_or(Error::Truncated)?;
    let method = read_u16(data, 6).ok_or(Error::Truncated)?;

    // Version-specific fields; `file_size` (when present) covers the whole block.
    let (header_len, block_size) = match version {
        1 => (20, None), // u32 uncompressedSize, u64 hash
        2 => (
            24,
            Some(read_u32(data, 8).ok_or(Error::Truncated)? as usize),
        ),
        3 => (
            32,
            Some(read_u64(data, 8).ok_or(Error::Truncated)? as usize),
        ),
        other => return Err(Error::UnsupportedVersion(other)),
    };

    let (payload, consumed) = match block_size {
        Some(size) => {
            let end = size.min(data.len());
            (data.get(header_len..end).ok_or(Error::Truncated)?, size)
        }
        None => (data.get(header_len..).ok_or(Error::Truncated)?, data.len()),
    };

    Ok((decompress(method, payload, HEADER_CAP)?, consumed))
}

/// Decompress up to `cap` bytes of `payload` with the given CCOB method.
fn decompress(method: u16, payload: &[u8], cap: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    let cap = cap as u64;
    match method {
        0 => {
            // Zlib
            let reader = flate2::read::ZlibDecoder::new(payload);
            reader
                .take(cap)
                .read_to_end(&mut out)
                .map_err(|e| Error::Decompress(e.to_string()))?;
        }
        1 => {
            // Zstd
            let reader = ruzstd::decoding::StreamingDecoder::new(payload)
                .map_err(|e| Error::Decompress(format!("{e:?}")))?;
            reader
                .take(cap)
                .read_to_end(&mut out)
                .map_err(|e| Error::Decompress(e.to_string()))?;
        }
        other => return Err(Error::UnsupportedCompression(other)),
    }
    Ok(out)
}

// --- little-endian readers (return None when out of bounds) -----------------

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u64(data: &[u8], at: usize) -> Option<u64> {
    let bytes = data.get(at..at + 8)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a plain `__CLANG_OFFLOAD_BUNDLE__` from `(id, size)` pairs.
    fn plain_bundle(ids: &[(&str, u64)]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(BUNDLE_MAGIC);
        b.extend_from_slice(&(ids.len() as u64).to_le_bytes());
        let mut off = 0u64;
        for (id, size) in ids {
            b.extend_from_slice(&off.to_le_bytes());
            b.extend_from_slice(&size.to_le_bytes());
            b.extend_from_slice(&(id.len() as u64).to_le_bytes());
            b.extend_from_slice(id.as_bytes());
            off += size;
        }
        b
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(data).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn parses_plain_bundle_targets() {
        let data = plain_bundle(&[
            ("host-x86_64-unknown-linux-gnu", 0),
            ("hipv4-amdgcn-amd-amdhsa--gfx90a", 4096),
            ("hipv4-amdgcn-amd-amdhsa--gfx942:sramecc+", 8192),
        ]);
        let entries = parse(&data).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_host());
        assert_eq!(entries[0].target(), None);
        assert_eq!(entries[1].target(), Some("gfx90a"));
        assert_eq!(entries[2].target(), Some("gfx942:sramecc+"));
    }

    #[test]
    fn parses_compressed_ccob_v2_zlib() {
        let bundle = plain_bundle(&[("hipv4-amdgcn-amd-amdhsa--gfx1100", 16)]);
        let payload = zlib(&bundle);

        // CCOB V2 header: magic, version=2, method=0 (zlib), fileSize u32,
        // uncompressedSize u32, hash u64.
        let header_len = 24;
        let file_size = header_len + payload.len();
        let mut block = Vec::new();
        block.extend_from_slice(CCOB_MAGIC);
        block.extend_from_slice(&2u16.to_le_bytes());
        block.extend_from_slice(&0u16.to_le_bytes());
        block.extend_from_slice(&(file_size as u32).to_le_bytes());
        block.extend_from_slice(&(bundle.len() as u32).to_le_bytes());
        block.extend_from_slice(&0u64.to_le_bytes()); // hash
        block.extend_from_slice(&payload);

        let entries = parse(&block).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].target(), Some("gfx1100"));
    }

    #[test]
    fn parses_multiple_ccob_blocks_with_padding() {
        let make_block = |target: &str| {
            let bundle = plain_bundle(&[(target, 16)]);
            let payload = zlib(&bundle);
            let file_size = 24 + payload.len();
            let mut block = Vec::new();
            block.extend_from_slice(CCOB_MAGIC);
            block.extend_from_slice(&2u16.to_le_bytes());
            block.extend_from_slice(&0u16.to_le_bytes());
            block.extend_from_slice(&(file_size as u32).to_le_bytes());
            block.extend_from_slice(&(bundle.len() as u32).to_le_bytes());
            block.extend_from_slice(&0u64.to_le_bytes());
            block.extend_from_slice(&payload);
            block
        };
        let mut section = make_block("hipv4-amdgcn-amd-amdhsa--gfx906");
        section.extend_from_slice(&[0u8; 5]); // padding between blocks
        section.extend_from_slice(&make_block("hipv4-amdgcn-amd-amdhsa--gfx908"));

        let entries = parse(&section).unwrap();
        let targets: Vec<Option<&str>> = entries.iter().map(|e| e.target()).collect();
        assert_eq!(targets, [Some("gfx906"), Some("gfx908")]);
    }

    #[test]
    fn empty_and_unknown_inputs() {
        assert!(parse(&[]).unwrap().is_empty());
        assert!(parse(b"not a bundle").unwrap().is_empty());
        assert!(!has_magic(b"xx"));
        assert!(has_magic(BUNDLE_MAGIC));
        assert!(has_magic(CCOB_MAGIC));
    }

    #[test]
    fn huge_id_length_does_not_overflow() {
        // idLength = u64::MAX must break cleanly, not overflow `id_start + len`.
        let mut b = Vec::new();
        b.extend_from_slice(BUNDLE_MAGIC);
        b.extend_from_slice(&1u64.to_le_bytes()); // numBundles
        b.extend_from_slice(&0u64.to_le_bytes()); // offset
        b.extend_from_slice(&0u64.to_le_bytes()); // size
        b.extend_from_slice(&u64::MAX.to_le_bytes()); // idLength
        assert!(parse(&b).unwrap().is_empty());
    }

    #[test]
    fn huge_ccob_file_size_does_not_overflow_or_hang() {
        // CCOB V3 with fileSize = u64::MAX must terminate without overflowing
        // `offset += consumed`.
        let mut block = Vec::new();
        block.extend_from_slice(CCOB_MAGIC);
        block.extend_from_slice(&3u16.to_le_bytes()); // version 3
        block.extend_from_slice(&0u16.to_le_bytes()); // method: zlib
        block.extend_from_slice(&u64::MAX.to_le_bytes()); // fileSize
        block.extend_from_slice(&0u64.to_le_bytes()); // uncompressedSize
        block.extend_from_slice(&0u64.to_le_bytes()); // hash
        let _ = parse(&block); // must not panic or hang
    }
}
