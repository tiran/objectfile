# offload-bundle

Parser for the **Clang Offload Bundle** format used by AMD HIP/ROCm in the
`.hip_fatbin` ELF section (and by OpenMP offload). Lists the embedded code
objects and their GPU target IDs (`gfx90a`, `gfx942:sramecc+`, ...).

Decompression uses `flate2` (zlib, miniz_oxide backend) and `ruzstd` (Zstd) -
both pure Rust, so the crate links no C library.

## Format

A `.hip_fatbin` section holds one or more blocks. A block is either a plain
bundle or a compressed (CCOB) block whose payload decompresses to a plain
bundle. For large libraries, many independently-compressed CCOB blocks are
concatenated with zero padding (e.g. `libtorch_hip.so` has hundreds).

Plain bundle (`clang/lib/Driver/OffloadBundler.cpp`):

```text
magic "__CLANG_OFFLOAD_BUNDLE__" (24) | numBundles u64
per entry: offset u64 | size u64 | idLength u64 | id[idLength]
```

The target ID is the part of `id` after the first `--`, e.g.
`hipv4-amdgcn-amd-amdhsa--gfx90a`. Host entries (`host-...`) have no target.

Compressed bundle (CCOB, `llvm/lib/Object/OffloadBundle.cpp`):

```text
magic "CCOB" (4) | version u16 | method u16   (method: 0 = Zlib, 1 = Zstd)
  V1: uncompressedSize u32 | hash u64
  V2: fileSize u32 | uncompressedSize u32 | hash u64
  V3: fileSize u64 | uncompressedSize u64 | hash u64
then a Zlib/Zstd payload that decompresses to a plain bundle
```

`fileSize` (V2/V3) covers the whole block (header + payload). Because only the
bundle headers are needed to list targets, this crate decompresses each block
only up to a small cap, never materialising the code objects.

The device entry payloads are AMDGPU ELF code objects (`e_machine = EM_AMDGPU`
= 224, `EI_OSABI = ELFOSABI_AMDGPU_HSA` = 64); the processor/features are also
encoded in their `e_flags` (`EF_AMDGPU_MACH`, `EF_AMDGPU_FEATURE_*`). The
companion `.hipFatBinSegment` section is a runtime registration stub and is not
needed for static analysis.

## References

- [ClangOffloadBundler format](https://clang.llvm.org/docs/ClangOffloadBundler.html) - official specification
- [OffloadBundler.cpp (LLVM)](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/clang/lib/Driver/OffloadBundler.cpp) - bundler implementation
- [OffloadBundle.cpp (LLVM)](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/lib/Object/OffloadBundle.cpp) - compressed bundle (CCOB) handling
- [AMDGPU ELF code object](https://llvm.org/docs/AMDGPUUsage.html#elf-code-object) - AMDGPU ELF spec
- [AMDGPU target ID](https://llvm.org/docs/AMDGPUUsage.html#target-id) - target ID format with feature qualifiers
- [ELF.h AMDGPU constants (LLVM)](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/BinaryFormat/ELF.h) - `EM_AMDGPU`, `EF_AMDGPU_MACH`, `EF_AMDGPU_FEATURE_*`
- [llvm compression format](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/llvm/include/llvm/Support/Compression.h) - CCOB `method`: Zlib = 0, Zstd = 1
- Tools: `clang-offload-bundler --type=o --list`, `llvm-objdump --offloading`, `llvm-readobj --offloading`

## License

Dual-licensed under either of Apache-2.0 or MIT at your option.
