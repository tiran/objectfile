//! Decode the AMDGPU processor (`gfx...`) and target features from the `e_flags`
//! field of an AMDGPU ELF code object (`e_machine == EM_AMDGPU`, OS/ABI
//! `ELFOSABI_AMDGPU_HSA`).
//!
//! Constants mirror LLVM `llvm/include/llvm/BinaryFormat/ELF.h` (see
//! [`crate::LLVM_SOURCE`]). The feature encoding here is the code-object
//! **ABI v4+** layout; older ABI versions (v2/v3) encoded features differently,
//! so check the ELF header's ABI version (`e_ident[EI_ABIVERSION]`,
//! `ELFABIVERSION_AMDGPU_HSA_V4 == 2`) before trusting [`Amd::xnack`] /
//! [`Amd::sramecc`].

use crate::{GpuTarget, Vendor};

/// Mask selecting the processor (`EF_AMDGPU_MACH`) in `e_flags`.
pub const EF_AMDGPU_MACH: u32 = 0x0ff;

/// Mask selecting the XNACK feature (ABI v4, `EF_AMDGPU_FEATURE_XNACK_V4`).
pub const EF_AMDGPU_FEATURE_XNACK_V4: u32 = 0x300;
/// Mask selecting the SRAMECC feature (ABI v4, `EF_AMDGPU_FEATURE_SRAMECC_V4`).
pub const EF_AMDGPU_FEATURE_SRAMECC_V4: u32 = 0xc00;

/// `EF_AMDGPU_MACH_AMDGCN_GFX*` value -> processor name, from LLVM `ELF.h`
/// (see [`crate::LLVM_SOURCE`]).
pub const MACH_TABLE: &[(u32, &str)] = &[
    (0x20, "gfx600"),
    (0x21, "gfx601"),
    (0x22, "gfx700"),
    (0x23, "gfx701"),
    (0x24, "gfx702"),
    (0x25, "gfx703"),
    (0x26, "gfx704"),
    (0x28, "gfx801"),
    (0x29, "gfx802"),
    (0x2a, "gfx803"),
    (0x2b, "gfx810"),
    (0x2c, "gfx900"),
    (0x2d, "gfx902"),
    (0x2e, "gfx904"),
    (0x2f, "gfx906"),
    (0x30, "gfx908"),
    (0x31, "gfx909"),
    (0x32, "gfx90c"),
    (0x33, "gfx1010"),
    (0x34, "gfx1011"),
    (0x35, "gfx1012"),
    (0x36, "gfx1030"),
    (0x37, "gfx1031"),
    (0x38, "gfx1032"),
    (0x39, "gfx1033"),
    (0x3a, "gfx602"),
    (0x3b, "gfx705"),
    (0x3c, "gfx805"),
    (0x3d, "gfx1035"),
    (0x3e, "gfx1034"),
    (0x3f, "gfx90a"),
    (0x41, "gfx1100"),
    (0x42, "gfx1013"),
    (0x43, "gfx1150"),
    (0x44, "gfx1103"),
    (0x45, "gfx1036"),
    (0x46, "gfx1101"),
    (0x47, "gfx1102"),
    (0x48, "gfx1200"),
    (0x49, "gfx1250"),
    (0x4a, "gfx1151"),
    (0x4c, "gfx942"),
    (0x4e, "gfx1201"),
    (0x4f, "gfx950"),
    (0x50, "gfx1310"),
    (0x51, "gfx9-generic"),
    (0x52, "gfx10-1-generic"),
    (0x53, "gfx10-3-generic"),
    (0x54, "gfx11-generic"),
    (0x55, "gfx1152"),
    (0x57, "gfx1154"),
    (0x58, "gfx1153"),
    (0x59, "gfx12-generic"),
    (0x5a, "gfx1251"),
    (0x5b, "gfx12-5-generic"),
    (0x5c, "gfx1172"),
    (0x5d, "gfx1170"),
    (0x5e, "gfx1171"),
    (0x5f, "gfx9-4-generic"),
    (0x62, "gfx11-7-generic"),
    (0x63, "gfx13-generic"),
];

/// The processor name for `e_flags`, e.g. `gfx90a`. `None` for an unknown mach.
pub fn processor(e_flags: u32) -> Option<&'static str> {
    let mach = e_flags & EF_AMDGPU_MACH;
    MACH_TABLE
        .iter()
        .find(|(value, _)| *value == mach)
        .map(|(_, name)| *name)
}

/// A target feature's state (ABI v4 encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// The target does not support the feature.
    Unsupported,
    /// Either setting is allowed.
    Any,
    /// Explicitly off (`-` in a target ID).
    Off,
    /// Explicitly on (`+` in a target ID).
    On,
}

fn decode_feature(e_flags: u32, mask: u32) -> Feature {
    // The two state bits occupy the low end of the mask; shift them down.
    let shift = mask.trailing_zeros();
    match (e_flags & mask) >> shift {
        0 => Feature::Unsupported,
        1 => Feature::Any,
        2 => Feature::Off,
        _ => Feature::On,
    }
}

/// XNACK feature state (ABI v4).
pub fn xnack(e_flags: u32) -> Feature {
    decode_feature(e_flags, EF_AMDGPU_FEATURE_XNACK_V4)
}

/// SRAMECC feature state (ABI v4).
pub fn sramecc(e_flags: u32) -> Feature {
    decode_feature(e_flags, EF_AMDGPU_FEATURE_SRAMECC_V4)
}

fn feature_suffix(name: &str, feature: Feature) -> Option<String> {
    // Only explicit on/off appear in a target ID; "any" is the omitted default.
    match feature {
        Feature::On => Some(format!("{name}+")),
        Feature::Off => Some(format!("{name}-")),
        Feature::Unsupported | Feature::Any => None,
    }
}

/// Reconstruct the AMDGPU target ID from `e_flags` (ABI v4), e.g.
/// `gfx90a:sramecc+:xnack-`. `None` if the processor mach is unknown.
///
/// Feature qualifiers follow LLVM's order (`sramecc` before `xnack`).
pub fn target_id(e_flags: u32) -> Option<String> {
    let mut id = processor(e_flags)?.to_string();
    if let Some(suffix) = feature_suffix("sramecc", sramecc(e_flags)) {
        id.push(':');
        id.push_str(&suffix);
    }
    if let Some(suffix) = feature_suffix("xnack", xnack(e_flags)) {
        id.push(':');
        id.push_str(&suffix);
    }
    Some(id)
}

/// Decoded AMDGPU `e_flags`. Holds the raw value; accessors decode on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amd {
    /// The raw ELF `e_flags`.
    pub e_flags: u32,
}

impl Amd {
    /// Wrap a raw `e_flags` value.
    pub fn new(e_flags: u32) -> Self {
        Amd { e_flags }
    }

    /// The raw processor mach value (`e_flags & EF_AMDGPU_MACH`).
    pub fn mach(&self) -> u32 {
        self.e_flags & EF_AMDGPU_MACH
    }

    /// The processor name, e.g. `gfx90a`. `None` for an unknown mach.
    pub fn processor(&self) -> Option<&'static str> {
        processor(self.e_flags)
    }

    /// The XNACK feature state (ABI v4).
    pub fn xnack(&self) -> Feature {
        xnack(self.e_flags)
    }

    /// The SRAMECC feature state (ABI v4).
    pub fn sramecc(&self) -> Feature {
        sramecc(self.e_flags)
    }

    /// The full target id, e.g. `gfx90a:sramecc+:xnack-`.
    pub fn target_id(&self) -> Option<String> {
        target_id(self.e_flags)
    }
}

impl GpuTarget for Amd {
    fn vendor(&self) -> Vendor {
        Vendor::Amd
    }

    fn target(&self) -> Option<String> {
        self.target_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_processors() {
        assert_eq!(processor(0x2f), Some("gfx906"));
        assert_eq!(processor(0x3f), Some("gfx90a"));
        assert_eq!(processor(0x4c), Some("gfx942"));
        assert_eq!(processor(0x4f), Some("gfx950"));
        assert_eq!(processor(0x51), Some("gfx9-generic"));
        // mach is only the low byte; upper feature bits are ignored.
        assert_eq!(processor(0xe2f), Some("gfx906"));
    }

    #[test]
    fn unknown_mach_is_none() {
        assert_eq!(processor(0x0ff), None);
        assert_eq!(processor(0x40), None); // gfx940 was removed upstream
        assert_eq!(target_id(0x0ff), None);
    }

    #[test]
    fn decodes_features() {
        // gfx906 + SRAMECC on (0xc00) + XNACK off (0x200)
        let e_flags = 0x2f | 0xc00 | 0x200;
        assert_eq!(sramecc(e_flags), Feature::On);
        assert_eq!(xnack(e_flags), Feature::Off);
        assert_eq!(
            target_id(e_flags).as_deref(),
            Some("gfx906:sramecc+:xnack-")
        );
    }

    #[test]
    fn omits_any_features() {
        // gfx90a with both features "any" (0x100 xnack-any, 0x400 sramecc-any).
        let e_flags = 0x3f | 0x100 | 0x400;
        assert_eq!(xnack(e_flags), Feature::Any);
        assert_eq!(sramecc(e_flags), Feature::Any);
        assert_eq!(target_id(e_flags).as_deref(), Some("gfx90a"));
    }

    #[test]
    fn amd_struct_matches_free_functions() {
        let e_flags = 0x2f | 0xc00 | 0x200;
        let amd = Amd::new(e_flags);
        assert_eq!(amd.mach(), 0x2f);
        assert_eq!(amd.processor(), Some("gfx906"));
        assert_eq!(amd.sramecc(), Feature::On);
        assert_eq!(amd.xnack(), Feature::Off);
        assert_eq!(amd.target().as_deref(), Some("gfx906:sramecc+:xnack-"));
        assert_eq!(amd.vendor(), Vendor::Amd);
    }

    #[test]
    fn mach_table_values_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (value, _) in MACH_TABLE {
            assert!(seen.insert(*value), "duplicate mach value {value:#x}");
        }
    }
}
