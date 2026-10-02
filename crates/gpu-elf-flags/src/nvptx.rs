//! Decode the NVIDIA SM target (`sm_90`, `sm_100a`, ...) from the `e_flags`
//! field of a CUDA ELF code object / cubin (`e_machine == EM_CUDA`).
//!
//! Constants mirror the CUDA `e_flags` enum in LLVM
//! `llvm/include/llvm/BinaryFormat/ELF.h` (see [`crate::LLVM_SOURCE`]). The real
//! (SASS) SM number is stored as its decimal value, e.g. `0x5a == 90` for
//! `sm_90`. Up to and including `sm_90` the number lives in the low byte
//! (`EF_CUDA_SM`); from Blackwell (`sm_100`+) it moved to bits 8-15
//! (`EF_CUDA_SM_MASK`), where the older layout kept the texture/addressing
//! flags, so the two encodings are mutually exclusive.

use crate::{GpuTarget, Vendor};

/// Processor (real SM) mask for the pre-Blackwell layout (bits 0-7).
pub const EF_CUDA_SM: u32 = 0x0000_00ff;
/// Processor (real SM) mask for the Blackwell+ layout (bits 8-15).
pub const EF_CUDA_SM_MASK: u32 = 0x0000_ff00;
/// Right-shift for [`EF_CUDA_SM_MASK`].
pub const EF_CUDA_SM_OFFSET: u32 = 8;
/// Virtual (PTX) SM mask (bits 16-23).
pub const EF_CUDA_VIRTUAL_SM: u32 = 0x00ff_0000;

/// Unified texture binding is enabled (pre-Blackwell layout).
pub const EF_CUDA_TEXMODE_UNIFIED: u32 = 0x0000_0100;
/// Independent texture binding is enabled (pre-Blackwell layout).
pub const EF_CUDA_TEXMODE_INDEPENDANT: u32 = 0x0000_0200;
/// The target uses 64-bit addressing (pre-Blackwell layout).
pub const EF_CUDA_64BIT_ADDRESS: u32 = 0x0000_0400;
/// Accelerator variant in the pre-Blackwell layout (`sm_90a`).
pub const EF_CUDA_ACCELERATORS_V1: u32 = 0x0000_0800;
/// Accelerator variant in the Blackwell+ layout (`sm_100a`, ...).
pub const EF_CUDA_ACCELERATORS: u32 = 0x0000_0008;

/// Known real-SM values (the `EF_CUDA_SM*` constants), used to tell the
/// pre-Blackwell encoding (SM in the low byte) from the Blackwell+ encoding
/// (SM in bits 8-15).
pub const SM_VALUES: &[u32] = &[
    20, 21, 30, 32, 35, 37, 50, 52, 53, 60, 61, 62, 70, 72, 75, 80, 86, 87, 88, 89, 90, 100, 101,
    103, 110, 120, 121,
];

fn is_known_sm(value: u32) -> bool {
    SM_VALUES.contains(&value)
}

/// Decoded NVIDIA CUDA `e_flags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nvidia {
    /// The raw ELF `e_flags`.
    pub e_flags: u32,
    /// The real (SASS) SM number, e.g. `90` for `sm_90`. `0` if not identifiable.
    pub sm: u32,
    /// The virtual (PTX) SM number (`EF_CUDA_VIRTUAL_SM`), e.g. `90`. `0` if
    /// absent.
    pub virtual_sm: u32,
    /// An accelerator variant (`sm_90a`, `sm_100a`, ...).
    pub accelerated: bool,
    /// 64-bit addressing (`EF_CUDA_64BIT_ADDRESS`, pre-Blackwell layout only).
    pub address_64bit: bool,
    /// True when the Blackwell+ layout (real SM in bits 8-15) was detected.
    pub new_abi: bool,
}

/// Decode a raw CUDA `e_flags` value.
pub fn decode(e_flags: u32) -> Nvidia {
    Nvidia::decode(e_flags)
}

impl Nvidia {
    /// Decode a raw CUDA `e_flags` value.
    pub fn decode(e_flags: u32) -> Nvidia {
        let low = e_flags & EF_CUDA_SM;
        let high = (e_flags & EF_CUDA_SM_MASK) >> EF_CUDA_SM_OFFSET;
        // Prefer a known SM in the low byte (pre-Blackwell); otherwise take the
        // high byte (Blackwell+). The low byte is checked first so that older
        // cubins, whose high bits carry texture/addressing flags, decode right.
        let (sm, new_abi) = if is_known_sm(low) {
            (low, false)
        } else if is_known_sm(high) {
            (high, true)
        } else {
            (low, false)
        };
        let accelerated = if new_abi {
            e_flags & EF_CUDA_ACCELERATORS != 0
        } else {
            e_flags & EF_CUDA_ACCELERATORS_V1 != 0
        };
        // In the Blackwell+ layout bit 10 is part of the SM number, not the
        // 64-bit flag, so only honor it for the pre-Blackwell layout.
        let address_64bit = !new_abi && (e_flags & EF_CUDA_64BIT_ADDRESS != 0);
        Nvidia {
            e_flags,
            sm,
            virtual_sm: (e_flags & EF_CUDA_VIRTUAL_SM) >> 16,
            accelerated,
            address_64bit,
            new_abi,
        }
    }

    /// The real (SASS) target id, e.g. `sm_90` or `sm_90a`. `None` if the SM is
    /// not identifiable.
    pub fn target(&self) -> Option<String> {
        if self.sm == 0 {
            return None;
        }
        let suffix = if self.accelerated { "a" } else { "" };
        Some(format!("sm_{}{}", self.sm, suffix))
    }

    /// The virtual (PTX) target id, e.g. `compute_90`, from
    /// `EF_CUDA_VIRTUAL_SM`. `None` if no virtual arch is encoded.
    pub fn virtual_target(&self) -> Option<String> {
        if self.virtual_sm == 0 {
            return None;
        }
        Some(format!("compute_{}", self.virtual_sm))
    }
}

impl GpuTarget for Nvidia {
    fn vendor(&self) -> Vendor {
        Vendor::Nvidia
    }

    fn target(&self) -> Option<String> {
        self.target()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_blackwell_sm() {
        assert_eq!(decode(0x5a).target().as_deref(), Some("sm_90"));
        assert_eq!(decode(0x4b).target().as_deref(), Some("sm_75"));
        assert_eq!(decode(0x50).target().as_deref(), Some("sm_80"));
        // SM lives in the low byte; unrelated high bits do not shift it.
        let nv = decode(0x5a);
        assert_eq!(nv.sm, 90);
        assert!(!nv.new_abi);
    }

    #[test]
    fn accelerated_pre_blackwell() {
        let nv = decode(0x5a | EF_CUDA_ACCELERATORS_V1);
        assert!(nv.accelerated);
        assert!(!nv.new_abi);
        assert_eq!(nv.target().as_deref(), Some("sm_90a"));
    }

    #[test]
    fn blackwell_sm_in_high_byte() {
        // sm_100: 0x64 == 100 in bits 8-15.
        let nv = decode(100 << EF_CUDA_SM_OFFSET);
        assert_eq!(nv.sm, 100);
        assert!(nv.new_abi);
        assert_eq!(nv.target().as_deref(), Some("sm_100"));

        // sm_120a: SM in the high byte + the new accelerator bit.
        let nv = decode((120 << EF_CUDA_SM_OFFSET) | EF_CUDA_ACCELERATORS);
        assert_eq!(nv.sm, 120);
        assert!(nv.accelerated);
        assert_eq!(nv.target().as_deref(), Some("sm_120a"));
    }

    #[test]
    fn flags_and_virtual_arch() {
        let nv = decode(0x50 | EF_CUDA_64BIT_ADDRESS | (90 << 16));
        assert_eq!(nv.sm, 80);
        assert!(nv.address_64bit);
        assert_eq!(nv.virtual_sm, 90);
        assert_eq!(nv.virtual_target().as_deref(), Some("compute_90"));
    }

    #[test]
    fn unknown_is_none() {
        let nv = decode(0);
        assert_eq!(nv.sm, 0);
        assert_eq!(nv.target(), None);
        assert_eq!(nv.vendor(), Vendor::Nvidia);
    }
}
