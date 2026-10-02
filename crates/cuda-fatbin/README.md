# cuda-fatbin

Parser for the NVIDIA CUDA **fat binary** container found in the `.nv_fatbin`
ELF section of CUDA-accelerated shared libraries and executables.

A dependency-free library. It reads the container headers to list the embedded
code objects and their GPU architectures (`sm_90a`, `compute_120`, ...) without
decompressing any device code.

## Format

The `.nv_fatbin` section holds one or more **frames**, concatenated
back-to-back. Each frame is a 16-byte header followed by its entries:

```text
FatBinHeader: magic u32 = 0xBA55ED50 | version u16 = 1 | headerSize u16 | fatSize u64
```

Each **entry** describes one code object (PTX or a cubin ELF). The SM
architecture is carried in the entry header, so no decompression is needed to
enumerate targets:

- `kind` u16 - `0x1` PTX, `0x2` ELF (cubin SASS), `0x4` old cubin, `0x8` NVVM IR
- `headerSize` u32, `paddedPayloadSize` u32 (used to advance to the next entry)
- `payloadSize` u32 - payload length (compressed size when compressed)
- `smVersion` u32 at offset 28 - e.g. `90` -> `sm_90` / `compute_90`
- `flags` u64 at offset 40 - includes `archSpecific` (`0x100000`, e.g. `sm_90a`)
  and compression bits (`0x1000` NVIDIA, `0x2000` LZ4, `0x8000` Zstd)

The layout here follows publicly available documentation and the open-source
parsers listed below. The companion `.nvFatBinSegment` section is a runtime
registration stub and is not needed for static analysis.

## References

- [fatbinary.h (Gklee)](https://github.com/Geof23/Gklee/blob/master/Gklee/include/cuda/fatbinary.h) - fat binary header structures
- [cuda-fatbin-decompression](https://github.com/n-eiling/cuda-fatbin-decompression/blob/master/fatbin-decompress.h) - fatbin decompression research
- [cricket cpu-elf2.c](https://github.com/RWTH-ACS/cricket/blob/master/cpu/cpu-elf2.c) - CUDA ELF parsing
- [harmonv nvfatbin.py](https://github.com/pyxis-roc/gpu-api-interposer/blob/74379408f4b87fe5629d11ef92a766deaca2fe8c/harmonv/harmonv/nvfatbin.py) - Python fatbin parser
- [pytorch cubinsizes.py](https://github.com/pytorch/test-infra/blob/main/tools/analytics/cubinsizes.py) - PyTorch cubin size analysis
- [cudaparsers](https://github.com/VivekPanyam/cudaparsers) - Rust parsers for cubin and fatbin files
- [DeviceOffload.cpp (LLVM)](https://github.com/llvm/llvm-project/blob/llvmorg-20.1.8/clang/lib/Interpreter/DeviceOffload.cpp) - LLVM device offload handling
- [CUDA binary utilities](https://docs.nvidia.com/cuda/cuda-binary-utilities/index.html) - `cuobjdump`, `nvdisasm`, `nvprune`
- [nvFatbin](https://docs.nvidia.com/cuda/nvfatbin/index.html) - runtime fatbin creation library

## License

Dual-licensed under either of Apache-2.0 or MIT at your option.
