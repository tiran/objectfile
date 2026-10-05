//! ELF dynamic-section and symbol-version extraction: the elfdeps-style view of
//! an object (soname, interpreter, rpath/runpath, symbol-hash style, link kind,
//! and the verdef/verneed version aggregates).
//!
//! The generic `Object` trait does not expose these, so this reads the ELF
//! dynamic section, program headers, and `.gnu.version_d` / `.gnu.version_r`
//! directly (the same ELF-specific approach as `collect_elf_dynamic_symbols`).
//!
//! `soname`, `interpreter`, and `rpaths` also have Mach-O analogues
//! (`LC_ID_DYLIB`, `LC_LOAD_DYLINKER`, `LC_RPATH`) and are read from there too.
//! Everything else is ELF-only: non-ELF files return empty / `Unknown`.
//!
//! These are best-effort reads: a malformed section or load command yields the
//! default (empty / `None` / `Unknown`) rather than an error, since the file
//! itself already parsed. Reads are section-/load-command-based, matching how
//! `object`'s own `libraries()` / `imports()` / `exports()` work.

use std::collections::BTreeMap;

use object::elf;
use object::read::elf::{Dyn, ElfFile, FileHeader, ProgramHeader};
use object::read::macho::{LoadCommandVariant, MachHeader, MachOFile};
use object::read::{Object, ReadRef};

use crate::enums::{LinkKind, SymbolHash};
use crate::file::ParsedFile;
use crate::model::decode_name;

/// Dispatch to the ELF32/ELF64 implementation, or return `$default` for non-ELF.
macro_rules! elf_dispatch {
    ($file:expr, $default:expr, $func:ident) => {
        match $file {
            object::File::Elf32(elf) => $func(elf),
            object::File::Elf64(elf) => $func(elf),
            _ => $default,
        }
    };
}

// --- public, format-dispatching entry points --------------------------------

/// The library's own name: ELF `DT_SONAME` or Mach-O `LC_ID_DYLIB` (install
/// name). `None` if unset.
pub(crate) fn soname(file: &ParsedFile) -> Option<String> {
    match file {
        object::File::Elf32(elf) => elf_soname(elf),
        object::File::Elf64(elf) => elf_soname(elf),
        object::File::MachO32(macho) => macho_string(macho, MachoString::IdDylib),
        object::File::MachO64(macho) => macho_string(macho, MachoString::IdDylib),
        _ => None,
    }
}

/// The program interpreter / dynamic loader: ELF `PT_INTERP` or Mach-O
/// `LC_LOAD_DYLINKER`. `None` if absent.
pub(crate) fn interpreter(file: &ParsedFile) -> Option<String> {
    match file {
        object::File::Elf32(elf) => elf_interpreter(elf),
        object::File::Elf64(elf) => elf_interpreter(elf),
        object::File::MachO32(macho) => macho_string(macho, MachoString::Dylinker),
        object::File::MachO64(macho) => macho_string(macho, MachoString::Dylinker),
        _ => None,
    }
}

/// Library search paths: ELF `DT_RPATH` or Mach-O `LC_RPATH` commands.
pub(crate) fn rpaths(file: &ParsedFile) -> Vec<String> {
    match file {
        object::File::Elf32(elf) => elf_rpaths(elf),
        object::File::Elf64(elf) => elf_rpaths(elf),
        object::File::MachO32(macho) => macho_rpaths(macho),
        object::File::MachO64(macho) => macho_rpaths(macho),
        _ => Vec::new(),
    }
}

/// `DT_RUNPATH`: the library search paths.
pub(crate) fn runpaths(file: &ParsedFile) -> Vec<String> {
    elf_dispatch!(file, Vec::new(), elf_runpaths)
}

/// Version names defined by this object (verdef), excluding the base entry.
pub(crate) fn provided_versions(file: &ParsedFile) -> Vec<String> {
    elf_dispatch!(file, Vec::new(), elf_provided_versions)
}

/// Required symbol versions per needed library (verneed): `{soname: [versions]}`.
pub(crate) fn required_versions(file: &ParsedFile) -> BTreeMap<String, Vec<String>> {
    elf_dispatch!(file, BTreeMap::new(), elf_required_versions)
}

/// Which symbol hash table(s) the object carries (`DT_HASH` / `DT_GNU_HASH`).
pub(crate) fn symbol_hash(file: &ParsedFile) -> SymbolHash {
    elf_dispatch!(file, SymbolHash::Unknown, elf_symbol_hash)
}

/// The link kind, distinguishing a PIE executable from a shared library (ELF).
pub(crate) fn link_kind(file: &ParsedFile) -> LinkKind {
    let base = match file.kind() {
        object::ObjectKind::Relocatable => LinkKind::Relocatable,
        object::ObjectKind::Executable => LinkKind::Executable,
        object::ObjectKind::Core => LinkKind::Core,
        object::ObjectKind::Dynamic => LinkKind::SharedLibrary,
        _ => LinkKind::Unknown,
    };
    // An ELF PIE executable is ET_DYN (so it reads as `Dynamic`/`SharedLibrary`)
    // but carries DT_DEBUG or DF_1_PIE; upgrade it.
    if matches!(base, LinkKind::SharedLibrary) && elf_dispatch!(file, false, elf_is_pie) {
        LinkKind::PieExecutable
    } else {
        base
    }
}

// --- ELF implementations ----------------------------------------------------

/// The `.dynamic` entries, if the file has a dynamic section.
fn dynamic_array<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> Option<&'data [Elf::Dyn]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let endian = elf.endian();
    elf.elf_section_table()
        .dynamic(endian, elf.data())
        .ok()?
        .map(|(entries, _)| entries)
}

/// The string value of the first dynamic entry with `tag` (e.g. `DT_SONAME`).
fn dyn_string_value<'data, Elf, R>(
    elf: &ElfFile<'data, Elf, R>,
    tag: elf::DynamicTag,
) -> Option<&'data [u8]>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let endian = elf.endian();
    let data = elf.data();
    let (dynamic, index) = elf.elf_section_table().dynamic(endian, data).ok()??;
    let strings = elf.elf_section_table().strings(endian, data, index).ok()?;
    let entry = dynamic.iter().find(|d| d.d_tag(endian) == tag)?;
    entry.string(endian, strings).ok()
}

/// Split a colon-separated path list (`DT_RPATH` / `DT_RUNPATH`), dropping empties.
fn split_paths(raw: Option<&[u8]>) -> Vec<String> {
    match raw.and_then(decode_name) {
        Some(value) => value
            .split(':')
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect(),
        None => Vec::new(),
    }
}

fn elf_soname<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> Option<String>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    dyn_string_value(elf, elf::DT_SONAME).and_then(decode_name)
}

fn elf_rpaths<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> Vec<String>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    split_paths(dyn_string_value(elf, elf::DT_RPATH))
}

fn elf_runpaths<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> Vec<String>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    split_paths(dyn_string_value(elf, elf::DT_RUNPATH))
}

fn elf_interpreter<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> Option<String>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let endian = elf.endian();
    let data = elf.data();
    for header in elf.elf_program_headers() {
        // `interpreter` returns the path with the trailing NUL already removed.
        if let Ok(Some(interp)) = header.interpreter(endian, data) {
            return decode_name(interp);
        }
    }
    None
}

fn elf_symbol_hash<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> SymbolHash
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let Some(dynamic) = dynamic_array(elf) else {
        return SymbolHash::Unknown;
    };
    let endian = elf.endian();
    let has_sysv = dynamic.iter().any(|d| d.d_tag(endian) == elf::DT_HASH);
    let has_gnu = dynamic.iter().any(|d| d.d_tag(endian) == elf::DT_GNU_HASH);
    match (has_sysv, has_gnu) {
        (false, false) => SymbolHash::Absent,
        (true, false) => SymbolHash::SysV,
        (false, true) => SymbolHash::GNU,
        (true, true) => SymbolHash::Both,
    }
}

fn elf_is_pie<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> bool
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let Some(dynamic) = dynamic_array(elf) else {
        return false;
    };
    let endian = elf.endian();
    let has_debug = dynamic.iter().any(|d| d.d_tag(endian) == elf::DT_DEBUG);
    let has_pie_flag = dynamic
        .iter()
        .any(|d| d.d_tag(endian) == elf::DT_FLAGS_1 && d.val(endian) & elf::DF_1_PIE.0 != 0);
    has_debug || has_pie_flag
}

fn elf_provided_versions<'data, Elf, R>(elf: &ElfFile<'data, Elf, R>) -> Vec<String>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let endian = elf.endian();
    let data = elf.data();
    let Ok(Some((verdefs, index))) = elf.elf_section_table().gnu_verdef(endian, data) else {
        return Vec::new();
    };
    let Ok(strings) = elf.elf_section_table().strings(endian, data, index) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in verdefs {
        let Ok((verdef, mut auxiliaries)) = entry else {
            break;
        };
        // The base entry names the object itself (its soname), not a version.
        if verdef.vd_flags.get(endian).0 & elf::VER_FLG_BASE.0 != 0 {
            continue;
        }
        // The version name is the first auxiliary record.
        if let Ok(Some(aux)) = auxiliaries.next() {
            if let Ok(name) = aux.name(endian, strings) {
                if let Some(name) = decode_name(name) {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn elf_required_versions<'data, Elf, R>(
    elf: &ElfFile<'data, Elf, R>,
) -> BTreeMap<String, Vec<String>>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    let mut map = BTreeMap::new();
    let endian = elf.endian();
    let data = elf.data();
    let Ok(Some((verneeds, index))) = elf.elf_section_table().gnu_verneed(endian, data) else {
        return map;
    };
    let Ok(strings) = elf.elf_section_table().strings(endian, data, index) else {
        return map;
    };

    for entry in verneeds {
        let Ok((verneed, auxiliaries)) = entry else {
            break;
        };
        let Some(library) = verneed.file(endian, strings).ok().and_then(decode_name) else {
            continue;
        };
        let versions: &mut Vec<String> = map.entry(library).or_default();
        for aux in auxiliaries {
            let Ok(aux) = aux else {
                break;
            };
            if let Ok(name) = aux.name(endian, strings) {
                if let Some(name) = decode_name(name) {
                    versions.push(name);
                }
            }
        }
    }
    for versions in map.values_mut() {
        versions.sort();
        versions.dedup();
    }
    map
}

// --- Mach-O implementations -------------------------------------------------

/// Which single-string load command to read.
#[derive(Clone, Copy)]
enum MachoString {
    /// `LC_ID_DYLIB`: the dylib's install name (soname analogue).
    IdDylib,
    /// `LC_LOAD_DYLINKER`: the dynamic linker path (interpreter analogue).
    Dylinker,
}

/// The string of the first matching load command, resolved against its data.
fn macho_string<'data, Mach, R>(
    macho: &MachOFile<'data, Mach, R>,
    which: MachoString,
) -> Option<String>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    let endian = macho.endian();
    let mut commands = macho.macho_load_commands().ok()?;
    while let Ok(Some(command)) = commands.next() {
        let name = match (which, command.variant()) {
            (MachoString::IdDylib, Ok(LoadCommandVariant::IdDylib(dylib))) => {
                command.string(endian, dylib.dylib.name)
            }
            (MachoString::Dylinker, Ok(LoadCommandVariant::LoadDylinker(dylinker))) => {
                command.string(endian, dylinker.name)
            }
            _ => continue,
        };
        if let Ok(name) = name {
            return decode_name(name);
        }
    }
    None
}

/// All `LC_RPATH` search paths (each command carries one path).
fn macho_rpaths<'data, Mach, R>(macho: &MachOFile<'data, Mach, R>) -> Vec<String>
where
    Mach: MachHeader,
    R: ReadRef<'data>,
{
    let endian = macho.endian();
    let mut out = Vec::new();
    let Ok(mut commands) = macho.macho_load_commands() else {
        return out;
    };
    while let Ok(Some(command)) = commands.next() {
        if let Ok(LoadCommandVariant::Rpath(rpath)) = command.variant() {
            if let Ok(path) = command.string(endian, rpath.path) {
                if let Some(path) = decode_name(path) {
                    out.push(path);
                }
            }
        }
    }
    out
}
