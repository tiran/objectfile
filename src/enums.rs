//! Enums exposed to Python as native (pyo3) enums.
//!
//! Each enum mirrors the matching `object` enum. The `object` enums are
//! `#[non_exhaustive]`, so every conversion has a catch-all arm that maps any
//! future variant to `Unknown`.

use pyo3::prelude::*;

/// Binary file format (mirrors `object::BinaryFormat`).
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    Unknown,
    Coff,
    Elf,
    MachO,
    Pe,
    Wasm,
    Xcoff,
}

impl Format {
    pub(crate) fn from_object(format: object::BinaryFormat) -> Self {
        match format {
            object::BinaryFormat::Coff => Format::Coff,
            object::BinaryFormat::Elf => Format::Elf,
            object::BinaryFormat::MachO => Format::MachO,
            object::BinaryFormat::Pe => Format::Pe,
            object::BinaryFormat::Wasm => Format::Wasm,
            object::BinaryFormat::Xcoff => Format::Xcoff,
            _ => Format::Unknown,
        }
    }
}

/// CPU architecture (mirrors `object::Architecture`).
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types)] // keep the familiar names, e.g. Architecture.X86_64
pub enum Architecture {
    Unknown,
    Aarch64,
    Aarch64_Ilp32,
    Alpha,
    Arm,
    Avr,
    Bpf,
    Csky,
    E2K32,
    E2K64,
    I386,
    X86_64,
    X86_64_X32,
    Hexagon,
    Hppa,
    Ia64,
    LoongArch32,
    LoongArch64,
    M68k,
    Mips,
    Mips64,
    Mips64_N32,
    Msp430,
    PowerPc,
    PowerPc64,
    Riscv32,
    Riscv64,
    S390x,
    Sbf,
    Sharc,
    Sparc,
    Sparc32Plus,
    Sparc64,
    SuperH,
    Wasm32,
    Wasm64,
    Xtensa,
}

impl Architecture {
    pub(crate) fn from_object(arch: object::Architecture) -> Self {
        match arch {
            object::Architecture::Aarch64 => Architecture::Aarch64,
            object::Architecture::Aarch64_Ilp32 => Architecture::Aarch64_Ilp32,
            object::Architecture::Alpha => Architecture::Alpha,
            object::Architecture::Arm => Architecture::Arm,
            object::Architecture::Avr => Architecture::Avr,
            object::Architecture::Bpf => Architecture::Bpf,
            object::Architecture::Csky => Architecture::Csky,
            object::Architecture::E2K32 => Architecture::E2K32,
            object::Architecture::E2K64 => Architecture::E2K64,
            object::Architecture::I386 => Architecture::I386,
            object::Architecture::X86_64 => Architecture::X86_64,
            object::Architecture::X86_64_X32 => Architecture::X86_64_X32,
            object::Architecture::Hexagon => Architecture::Hexagon,
            object::Architecture::Hppa => Architecture::Hppa,
            object::Architecture::Ia64 => Architecture::Ia64,
            object::Architecture::LoongArch32 => Architecture::LoongArch32,
            object::Architecture::LoongArch64 => Architecture::LoongArch64,
            object::Architecture::M68k => Architecture::M68k,
            object::Architecture::Mips => Architecture::Mips,
            object::Architecture::Mips64 => Architecture::Mips64,
            object::Architecture::Mips64_N32 => Architecture::Mips64_N32,
            object::Architecture::Msp430 => Architecture::Msp430,
            object::Architecture::PowerPc => Architecture::PowerPc,
            object::Architecture::PowerPc64 => Architecture::PowerPc64,
            object::Architecture::Riscv32 => Architecture::Riscv32,
            object::Architecture::Riscv64 => Architecture::Riscv64,
            object::Architecture::S390x => Architecture::S390x,
            object::Architecture::Sbf => Architecture::Sbf,
            object::Architecture::Sharc => Architecture::Sharc,
            object::Architecture::Sparc => Architecture::Sparc,
            object::Architecture::Sparc32Plus => Architecture::Sparc32Plus,
            object::Architecture::Sparc64 => Architecture::Sparc64,
            object::Architecture::SuperH => Architecture::SuperH,
            object::Architecture::Wasm32 => Architecture::Wasm32,
            object::Architecture::Wasm64 => Architecture::Wasm64,
            object::Architecture::Xtensa => Architecture::Xtensa,
            _ => Architecture::Unknown,
        }
    }
}

/// Byte order (mirrors `object::Endianness`).
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Endianness {
    Little,
    Big,
}

impl Endianness {
    pub(crate) fn from_object(endianness: object::Endianness) -> Self {
        match endianness {
            object::Endianness::Little => Endianness::Little,
            object::Endianness::Big => Endianness::Big,
        }
    }
}

/// High-level object file kind (mirrors `object::ObjectKind`).
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKind {
    Unknown,
    Relocatable,
    Executable,
    Dynamic,
    Core,
}

impl ObjectKind {
    pub(crate) fn from_object(kind: object::ObjectKind) -> Self {
        match kind {
            object::ObjectKind::Relocatable => ObjectKind::Relocatable,
            object::ObjectKind::Executable => ObjectKind::Executable,
            object::ObjectKind::Dynamic => ObjectKind::Dynamic,
            object::ObjectKind::Core => ObjectKind::Core,
            _ => ObjectKind::Unknown,
        }
    }
}

/// What a symbol refers to (mirrors `object::SymbolKind`).
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolKind {
    Unknown,
    Text,
    Data,
    Section,
    File,
    Label,
    Tls,
}

impl SymbolKind {
    pub(crate) fn from_object(kind: object::SymbolKind) -> Self {
        match kind {
            object::SymbolKind::Text => SymbolKind::Text,
            object::SymbolKind::Data => SymbolKind::Data,
            object::SymbolKind::Section => SymbolKind::Section,
            object::SymbolKind::File => SymbolKind::File,
            object::SymbolKind::Label => SymbolKind::Label,
            object::SymbolKind::Tls => SymbolKind::Tls,
            _ => SymbolKind::Unknown,
        }
    }
}

/// Visibility scope of a symbol (mirrors `object::SymbolScope`).
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolScope {
    Unknown,
    Compilation,
    Linkage,
    Dynamic,
}

impl SymbolScope {
    pub(crate) fn from_object(scope: object::SymbolScope) -> Self {
        match scope {
            object::SymbolScope::Compilation => SymbolScope::Compilation,
            object::SymbolScope::Linkage => SymbolScope::Linkage,
            object::SymbolScope::Dynamic => SymbolScope::Dynamic,
            _ => SymbolScope::Unknown,
        }
    }
}

/// How an object participates in linking.
///
/// ELF distinguishes a position-independent executable (`ET_DYN` carrying
/// `DT_DEBUG` / `DF_1_PIE`) from a shared library; other formats map from the
/// generic object kind, so `PieExecutable` only occurs for ELF.
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkKind {
    Unknown,
    Relocatable,
    Executable,
    PieExecutable,
    SharedLibrary,
    Core,
}

/// Which symbol hash table(s) an ELF object carries (`DT_HASH` / `DT_GNU_HASH`).
///
/// `Unknown` for non-ELF files or when there is no dynamic section; `Absent` for
/// an ELF with a dynamic section but neither hash.
#[pyclass(eq, eq_int, frozen, from_py_object, module = "objectfile._objectfile")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::upper_case_acronyms)] // SysV / GNU are the correct spellings
pub enum SymbolHash {
    Unknown,
    Absent,
    SysV,
    GNU,
    Both,
}

// Each enum gets a `__str__` that is just the variant name (e.g. `Elf`), so
// Python code and serialization get a clean string without the type prefix.
// `{self:?}` is the derived `Debug`, which prints exactly the variant name.
// (`__repr__` keeps the pyo3 default, e.g. `Format.Elf`.)

#[pymethods]
impl Format {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl Architecture {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl Endianness {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl ObjectKind {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl SymbolKind {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl SymbolScope {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl LinkKind {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}

#[pymethods]
impl SymbolHash {
    fn __str__(&self) -> String {
        format!("{self:?}")
    }
}
