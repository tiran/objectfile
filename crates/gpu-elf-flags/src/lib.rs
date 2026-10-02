//! Decode a GPU code object's target from the ELF header fields `e_machine` and
//! `e_flags`, for AMD (`EM_AMDGPU`) and NVIDIA (`EM_CUDA`) code objects.
//!
//! The crate has two layers:
//!
//! - **Per-vendor modules** ([`amdgpu`], [`nvptx`]) expose the raw decoded
//!   fields and the LLVM `ELF.h` constants, for callers that need the
//!   individual flags (mach, xnack/sramecc, sm number, accelerator bit, ...).
//! - **A common API** ([`Flags`], [`Vendor`], [`GpuTarget`]) dispatches on
//!   `e_machine` and renders a target string, for callers that just want the
//!   target id regardless of vendor.
//!
//! ```
//! use gpu_elf_flags::{Flags, GpuTarget, Vendor, EM_AMDGPU, EM_CUDA};
//!
//! let amd = Flags::decode(EM_AMDGPU, 0x3f).unwrap();
//! assert_eq!(amd.vendor(), Vendor::Amd);
//! assert_eq!(amd.target().as_deref(), Some("gfx90a"));
//!
//! let nv = Flags::decode(EM_CUDA, 0x5a).unwrap();
//! assert_eq!(nv.vendor(), Vendor::Nvidia);
//! assert_eq!(nv.target().as_deref(), Some("sm_90"));
//! ```
//!
//! Constants mirror LLVM `llvm/include/llvm/BinaryFormat/ELF.h`; see
//! [`LLVM_SOURCE`] for the exact tag and the date it was recorded.

#![forbid(unsafe_code)]

use core::fmt;

pub mod amdgpu;
pub mod nvptx;

/// The LLVM release the constant tables were taken from (update the tables and
/// this tag together when refreshing).
// llvmorg-23.1.2 was released 2026-09-22.
pub const LLVM_SOURCE: &str = "llvmorg-23.1.2";

/// `e_machine` for AMD GPU code objects (`EM_AMDGPU`).
pub const EM_AMDGPU: u16 = 224;
/// `e_machine` for NVIDIA CUDA code objects (`EM_CUDA`).
pub const EM_CUDA: u16 = 190;

/// The GPU vendor behind a code object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Vendor {
    /// AMD (AMDGPU / ROCm / HIP).
    Amd,
    /// NVIDIA (CUDA / NVPTX).
    Nvidia,
}

impl Vendor {
    /// A lower-case stable identifier, e.g. `"amd"` or `"nvidia"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Vendor::Amd => "amd",
            Vendor::Nvidia => "nvidia",
        }
    }
}

impl fmt::Display for Vendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The common interface over a decoded code object.
pub trait GpuTarget {
    /// The GPU vendor.
    fn vendor(&self) -> Vendor;

    /// The target id, e.g. `gfx90a:sramecc+:xnack-` or `sm_90a`. `None` when the
    /// processor cannot be identified from `e_flags`.
    fn target(&self) -> Option<String>;
}

/// A decoded GPU code object, dispatched on `e_machine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flags {
    /// AMD AMDGPU (`EM_AMDGPU`).
    Amd(amdgpu::Amd),
    /// NVIDIA CUDA (`EM_CUDA`).
    Nvidia(nvptx::Nvidia),
}

impl Flags {
    /// Decode `e_flags` for the given `e_machine`. `None` for a machine that is
    /// not a supported GPU.
    pub fn decode(e_machine: u16, e_flags: u32) -> Option<Flags> {
        match e_machine {
            EM_AMDGPU => Some(Flags::Amd(amdgpu::Amd::new(e_flags))),
            EM_CUDA => Some(Flags::Nvidia(nvptx::Nvidia::decode(e_flags))),
            _ => None,
        }
    }

    /// The raw `e_flags` value this was decoded from.
    pub fn e_flags(&self) -> u32 {
        match self {
            Flags::Amd(amd) => amd.e_flags,
            Flags::Nvidia(nv) => nv.e_flags,
        }
    }
}

impl GpuTarget for Flags {
    fn vendor(&self) -> Vendor {
        match self {
            Flags::Amd(_) => Vendor::Amd,
            Flags::Nvidia(_) => Vendor::Nvidia,
        }
    }

    fn target(&self) -> Option<String> {
        match self {
            Flags::Amd(amd) => amd.target(),
            Flags::Nvidia(nv) => nv.target(),
        }
    }
}

impl fmt::Display for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.target() {
            Some(target) => f.write_str(&target),
            None => write!(
                f,
                "{}:unknown(e_flags={:#x})",
                self.vendor(),
                self.e_flags()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatches_by_machine() {
        let amd = Flags::decode(EM_AMDGPU, 0x3f).unwrap();
        assert_eq!(amd.vendor(), Vendor::Amd);
        assert_eq!(amd.target().as_deref(), Some("gfx90a"));

        let nv = Flags::decode(EM_CUDA, 0x4b).unwrap();
        assert_eq!(nv.vendor(), Vendor::Nvidia);
        assert_eq!(nv.target().as_deref(), Some("sm_75"));

        // EM_X86_64 (62) is not a GPU.
        assert_eq!(Flags::decode(62, 0), None);
    }

    #[test]
    fn display_renders_target_or_fallback() {
        let amd = Flags::decode(EM_AMDGPU, 0x4c).unwrap();
        assert_eq!(amd.to_string(), "gfx942");

        // Unknown AMD mach falls back to a vendor-tagged form.
        let unknown = Flags::decode(EM_AMDGPU, 0xff).unwrap();
        assert_eq!(unknown.to_string(), "amd:unknown(e_flags=0xff)");
    }

    #[test]
    fn vendor_strings() {
        assert_eq!(Vendor::Amd.as_str(), "amd");
        assert_eq!(Vendor::Nvidia.to_string(), "nvidia");
    }
}
