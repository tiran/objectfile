# gpu-elf-flags

Decode a GPU code object's target from the ELF header fields `e_machine` and
`e_flags`, for AMD (`EM_AMDGPU`) and NVIDIA (`EM_CUDA`) code objects.

No dependencies.

```rust
use gpu_elf_flags::{Flags, GpuTarget, Vendor, EM_AMDGPU, EM_CUDA};

// Common API: dispatch on e_machine, render the target string.
Flags::decode(EM_AMDGPU, 0x2f | 0xc00 | 0x200).unwrap().target();
// -> Some("gfx906:sramecc+:xnack-")
Flags::decode(EM_CUDA, 0x5a).unwrap().target();        // -> Some("sm_90")

// Individual flags, per vendor.
gpu_elf_flags::amdgpu::sramecc(0xc00);                 // Feature::On
gpu_elf_flags::nvptx::decode(0x5a).accelerated;        // false
```

## Two layers

- **Per-vendor modules** (`amdgpu`, `nvptx`) expose the raw decoded fields and
  the LLVM `ELF.h` constants, for callers that need the individual flags.
- **A common API** (`Flags`, `Vendor`, `GpuTarget`) dispatches on `e_machine`
  and renders a target string, for callers that just want the target id.

## What `e_flags` encodes

### AMD (`amdgpu`)

- **Processor** (`EF_AMDGPU_MACH`, bits 0-7): the gfx model. The value -> name
  mapping is an enumerated table (no formula), reproduced here from LLVM.
- **XNACK** / **SRAMECC** (bits 8-9 / 10-11): 4-state (unsupported / any / off /
  on). `target_id` renders explicit on/off as `+` / `-`, in LLVM's order
  (`sramecc` before `xnack`).

The feature encoding is the code-object **ABI v4+** layout. Older ABI versions
(v2/v3) differ, so check the ELF header's ABI version (`e_ident[EI_ABIVERSION]`,
`ELFABIVERSION_AMDGPU_HSA_V4 == 2`) first.

### NVIDIA (`nvptx`)

- **Real (SASS) SM** number, stored as its decimal value (`0x5a == 90` for
  `sm_90`). Up to `sm_90` it lives in the low byte (`EF_CUDA_SM`); from Blackwell
  (`sm_100`+) it moved to bits 8-15 (`EF_CUDA_SM_MASK`), where the older layout
  kept the texture/addressing flags. The decoder picks the layout by looking for
  a known SM value, preferring the low byte.
- **Accelerator** variant (`sm_90a` / `sm_100a`): `EF_CUDA_ACCELERATORS_V1`
  (bit 11) in the old layout, `EF_CUDA_ACCELERATORS` (bit 3) in the new one.
- **Virtual (PTX) SM** (`EF_CUDA_VIRTUAL_SM`, bits 16-23): `virtual_target`
  renders it as `compute_<N>`.

## Not here: board VRAM

`e_flags` encodes the **ISA target**, not the physical board. VRAM and similar
per-board properties map from the PCI device id, not the gfx/sm target (one
target covers several SKUs), so they are out of scope for this crate.

## Source of the constants

The tables mirror LLVM `llvm/include/llvm/BinaryFormat/ELF.h`. The exact release
is in the crate's `LLVM_SOURCE` constant (currently **llvmorg-23.1.2**, released
2026-09-22); update the tables and `LLVM_SOURCE` together when refreshing.

- [ELF.h (LLVM llvmorg-23.1.2)](https://github.com/llvm/llvm-project/blob/llvmorg-23.1.2/llvm/include/llvm/BinaryFormat/ELF.h)
- [AMDGPU target ID](https://llvm.org/docs/AMDGPUUsage.html#target-id)
- [AMDGPU processors table](https://llvm.org/docs/AMDGPUUsage.html#processors)

## License

Dual-licensed under either of Apache-2.0 or MIT at your option.
