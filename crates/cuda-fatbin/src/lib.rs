//! Parser for the NVIDIA CUDA fat binary container found in the `.nv_fatbin`
//! ELF section.
//!
//! The layout here follows publicly available documentation and open-source
//! parsers (see the crate README for references). A section holds one or more
//! **frames**, each a
//! 16-byte [`FatBinHeader`](https://en.wikipedia.org/wiki/Fat_binary) followed
//! by back-to-back **entries**. Every entry describes one embedded code object
//! (PTX or a cubin ELF) and - crucially - carries its SM architecture in the
//! header, so listing architectures needs no decompression.
//!
//! ```text
//! frame: [ magic u32 | version u16 | headerSize u16 | fatSize u64 ] entries...
//! entry: [ kind u16 | ... | smVersion u32 @28 | ... | flags u64 @40 | ... ]
//! ```

#![forbid(unsafe_code)]

use std::fmt;

/// Magic at the start of every fat binary frame (`0xBA55ED50`).
pub const FATBIN_MAGIC: u32 = 0xBA55_ED50;

// Entry flag bits (subset we interpret).
const FLAG_COMPRESSED: u64 = 0x1000; // NVIDIA's own compression (not LZ4/Zstd)
const FLAG_COMPRESSED_LZ4: u64 = 0x2000;
const FLAG_COMPRESSED_ZSTD: u64 = 0x8000;
const FLAG_ARCH_SPECIFIC: u64 = 0x10_0000;

/// What an entry's payload contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// PTX assembly (virtual ISA).
    Ptx,
    /// Compiled SASS in a cubin ELF.
    Elf,
    /// Old cubin format.
    OldCubin,
    /// NVVM IR.
    Ir,
    /// Any other/unknown kind value.
    Other(u16),
}

impl EntryKind {
    fn from_u16(value: u16) -> Self {
        match value {
            0x1 => EntryKind::Ptx,
            0x2 => EntryKind::Elf,
            0x4 => EntryKind::OldCubin,
            0x8 => EntryKind::Ir,
            other => EntryKind::Other(other),
        }
    }
}

/// How an entry's payload is compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    /// NVIDIA's own compression (flag `0x1000`), distinct from LZ4/Zstd.
    Nvidia,
    Lz4,
    Zstd,
}

/// One code object within a fat binary.
#[derive(Debug, Clone)]
pub struct Entry {
    pub kind: EntryKind,
    /// SM architecture number, e.g. `90` for `sm_90` / `compute_90`.
    pub sm_version: u32,
    /// True for architecture-specific variants such as `sm_90a`.
    pub arch_specific: bool,
    /// Raw entry flags.
    pub flags: u64,
    /// Code format version `(major, minor)`.
    pub code_version: (u16, u16),
    /// Offset of the payload within the parsed section bytes. Taken from the
    /// header and not validated against the input length - bounds-check before
    /// slicing a (possibly malformed) input.
    pub payload_offset: usize,
    /// Payload length in bytes (compressed size when compressed). As declared in
    /// the header; see [`Entry::payload_offset`].
    pub payload_size: usize,
}

impl Entry {
    /// Architecture name: `sm_<N>[a]` for cubins, `compute_<N>` for PTX.
    pub fn target(&self) -> String {
        match self.kind {
            EntryKind::Ptx => format!("compute_{}", self.sm_version),
            _ => {
                let suffix = if self.arch_specific { "a" } else { "" };
                format!("sm_{}{}", self.sm_version, suffix)
            }
        }
    }

    /// How the payload is compressed.
    pub fn compression(&self) -> Compression {
        if self.flags & FLAG_COMPRESSED_ZSTD != 0 {
            Compression::Zstd
        } else if self.flags & FLAG_COMPRESSED_LZ4 != 0 {
            Compression::Lz4
        } else if self.flags & FLAG_COMPRESSED != 0 {
            Compression::Nvidia
        } else {
            Compression::None
        }
    }
}

/// An error parsing a fat binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Input ended in the middle of a header or entry.
    Truncated,
    /// A malformed entry that would not advance the cursor.
    ZeroLengthEntry,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => f.write_str("truncated CUDA fat binary"),
            Error::ZeroLengthEntry => f.write_str("zero-length fat binary entry"),
        }
    }
}

impl std::error::Error for Error {}

/// True if `data` starts with the fat binary magic.
pub fn has_magic(data: &[u8]) -> bool {
    read_u32(data, 0).is_some_and(|magic| magic == FATBIN_MAGIC)
}

/// Parse all code-object entries from a `.nv_fatbin` section.
///
/// Stops cleanly at the first frame that does not start with the magic (the
/// section may contain trailing padding). Returns [`Error::Truncated`] if a
/// frame or entry header runs past the end of the data.
pub fn parse(data: &[u8]) -> Result<Vec<Entry>, Error> {
    let mut entries = Vec::new();
    let mut offset = 0usize;

    // `checked_sub` both bounds `offset` to the data and avoids overflow in the
    // "16 bytes remaining" check when a prior frame advanced `offset` far.
    while data
        .len()
        .checked_sub(offset)
        .is_some_and(|remaining| remaining >= 16)
    {
        let magic = read_u32(data, offset).ok_or(Error::Truncated)?;
        if magic != FATBIN_MAGIC {
            break; // end of frames / padding
        }
        let header_size = read_u16(data, offset + 6).ok_or(Error::Truncated)? as usize;
        let fat_size = read_u64(data, offset + 8).ok_or(Error::Truncated)? as usize;

        let mut entry_offset = offset.checked_add(header_size).ok_or(Error::Truncated)?;
        let frame_end = entry_offset
            .checked_add(fat_size)
            .ok_or(Error::Truncated)?
            .min(data.len());

        while entry_offset + 48 <= frame_end {
            let entry = parse_entry(data, entry_offset)?;
            let advance = entry
                .header_len
                .checked_add(entry.padded_payload_size)
                .ok_or(Error::Truncated)?;
            if advance == 0 {
                return Err(Error::ZeroLengthEntry);
            }
            entries.push(entry.entry);
            entry_offset = entry_offset.checked_add(advance).ok_or(Error::Truncated)?;
        }

        // Advance to the next frame.
        let next_offset = offset
            .checked_add(header_size)
            .and_then(|value| value.checked_add(fat_size))
            .ok_or(Error::Truncated)?;
        if next_offset <= offset {
            break; // a zero-size frame would otherwise loop forever
        }
        offset = next_offset;
    }

    Ok(entries)
}

/// A parsed entry plus the bookkeeping needed to advance the cursor.
struct ParsedEntry {
    entry: Entry,
    header_len: usize,
    padded_payload_size: usize,
}

fn parse_entry(data: &[u8], at: usize) -> Result<ParsedEntry, Error> {
    let kind = read_u16(data, at).ok_or(Error::Truncated)?;
    let header_len = read_u32(data, at + 4).ok_or(Error::Truncated)? as usize;
    let padded_payload_size = read_u32(data, at + 8).ok_or(Error::Truncated)? as usize;
    let payload_size = read_u32(data, at + 16).ok_or(Error::Truncated)? as usize;
    let code_minor = read_u16(data, at + 24).ok_or(Error::Truncated)?;
    let code_major = read_u16(data, at + 26).ok_or(Error::Truncated)?;
    let sm_version = read_u32(data, at + 28).ok_or(Error::Truncated)?;
    let flags = read_u64(data, at + 40).ok_or(Error::Truncated)?;

    let payload_offset = at.checked_add(header_len).ok_or(Error::Truncated)?;

    Ok(ParsedEntry {
        entry: Entry {
            kind: EntryKind::from_u16(kind),
            sm_version,
            arch_specific: flags & FLAG_ARCH_SPECIFIC != 0,
            flags,
            code_version: (code_major, code_minor),
            payload_offset,
            payload_size,
        },
        header_len,
        padded_payload_size,
    })
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

    /// Build one entry header (64 bytes) + a payload of `payload` bytes.
    fn entry(kind: u16, sm: u32, flags: u64, payload: &[u8]) -> Vec<u8> {
        let mut e = vec![0u8; 64];
        e[0..2].copy_from_slice(&kind.to_le_bytes());
        e[4..8].copy_from_slice(&64u32.to_le_bytes()); // headerSize
        e[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes()); // paddedPayloadSize
        e[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes()); // payloadSize
        e[24..26].copy_from_slice(&0u16.to_le_bytes()); // code minor
        e[26..28].copy_from_slice(&1u16.to_le_bytes()); // code major
        e[28..32].copy_from_slice(&sm.to_le_bytes());
        e[40..48].copy_from_slice(&flags.to_le_bytes());
        e.extend_from_slice(payload);
        e
    }

    fn frame(entries: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = entries.concat();
        let mut f = Vec::new();
        f.extend_from_slice(&FATBIN_MAGIC.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes()); // version
        f.extend_from_slice(&16u16.to_le_bytes()); // headerSize
        f.extend_from_slice(&(body.len() as u64).to_le_bytes()); // fatSize
        f.extend_from_slice(&body);
        f
    }

    #[test]
    fn parses_elf_and_ptx_targets() {
        let data = frame(&[
            entry(0x2, 90, FLAG_ARCH_SPECIFIC, b"CUBIN"), // sm_90a
            entry(0x2, 80, 0, b"cubin2"),                 // sm_80
            entry(0x1, 120, 0, b"PTXTEXT"),               // compute_120
        ]);
        let entries = parse(&data).unwrap();
        let targets: Vec<String> = entries.iter().map(Entry::target).collect();
        assert_eq!(targets, ["sm_90a", "sm_80", "compute_120"]);
    }

    #[test]
    fn exposes_payload_slice_and_compression() {
        let data = frame(&[entry(0x2, 75, FLAG_COMPRESSED_ZSTD, b"PAYLOAD")]);
        let entries = parse(&data).unwrap();
        let e = &entries[0];
        assert_eq!(e.compression(), Compression::Zstd);
        assert_eq!(
            &data[e.payload_offset..e.payload_offset + e.payload_size],
            b"PAYLOAD"
        );
    }

    #[test]
    fn handles_multiple_frames() {
        let mut data = frame(&[entry(0x2, 70, 0, b"a")]);
        data.extend_from_slice(&frame(&[entry(0x2, 86, 0, b"bb")]));
        let entries = parse(&data).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].target(), "sm_86");
    }

    #[test]
    fn stops_at_non_magic_padding() {
        let mut data = frame(&[entry(0x2, 70, 0, b"a")]);
        data.extend_from_slice(&[0u8; 32]); // trailing zero padding
        assert_eq!(parse(&data).unwrap().len(), 1);
    }

    #[test]
    fn empty_input_is_no_entries() {
        assert!(parse(&[]).unwrap().is_empty());
        assert!(!has_magic(&[]));
    }

    #[test]
    fn zero_size_frame_terminates() {
        // headerSize == 0 and fatSize == 0 would loop forever without the
        // progress guard (the magic still matches at the same offset).
        let mut data = Vec::new();
        data.extend_from_slice(&FATBIN_MAGIC.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // version
        data.extend_from_slice(&0u16.to_le_bytes()); // headerSize = 0
        data.extend_from_slice(&0u64.to_le_bytes()); // fatSize = 0
        assert!(parse(&data).unwrap().is_empty());
    }
}
