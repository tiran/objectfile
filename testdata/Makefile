NVCC = nvcc
NVCC_ARGS = -shared --compiler-options -fPIC -lcudart

# TORCH_CUDA_ARCH_LIST="7.5;8.0;9.0;9.0a;10.0;12.0+PTX;12.0a"
GENCODE_FLAGS = \
	-gencode arch=compute_75,code=sm_75 \
	-gencode arch=compute_80,code=sm_80 \
	-gencode arch=compute_90,code=sm_90 \
	-gencode arch=compute_90,code=sm_90a \
	-gencode arch=compute_100,code=sm_100 \
	-gencode arch=compute_120,code=sm_120 \
	-gencode arch=compute_120,code=sm_120a \
	-gencode arch=compute_120,code=compute_120

ROCM_LLVM = /opt/rocm/llvm/bin
HIPCC = hipcc
# MI250 (gfx90a) with feature qualifiers, MI300X (gfx942),
# MI350X (gfx950), Radeon RX 7900 XT (gfx1100)
HIPCC_OFFLOAD_ARCH = \
	--offload-arch=gfx90a:sramecc+:xnack- \
	--offload-arch=gfx90a:sramecc+:xnack+ \
	--offload-arch=gfx942 \
	--offload-arch=gfx950 \
	--offload-arch=gfx1100

CUDA_DIR = $(CURDIR)/cuda
CUDA_COMPRESSED_DIR = $(CURDIR)/cuda-compressed
ROCM_DIR = $(CURDIR)/rocm
ROCM_COMPRESSED_DIR = $(CURDIR)/rocm-compressed

.PHONY: help
help: ## Display this help
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  \033[36m%-26s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

.PHONY: cuda
cuda: $(CUDA_DIR)/hello.so $(CUDA_DIR)/hello.cuobjdump $(CUDA_DIR)/hello.readelf $(CUDA_DIR)/hello.sm_120.cubin $(CUDA_DIR)/hello.compute_120.ptx $(CUDA_COMPRESSED_DIR)/hello.so $(CUDA_COMPRESSED_DIR)/hello.cuobjdump $(CUDA_COMPRESSED_DIR)/hello.readelf $(CUDA_COMPRESSED_DIR)/hello.sm_120.cubin $(CUDA_COMPRESSED_DIR)/hello.compute_120.ptx  ## build, dump, and split CUDA files

.PHONY: rocm
rocm: $(ROCM_DIR)/hello.so $(ROCM_DIR)/hello.readobj $(ROCM_DIR)/hello.readelf $(ROCM_DIR)/hip_fatbin.bin $(ROCM_DIR)/gfx950.co $(ROCM_COMPRESSED_DIR)/hello.so $(ROCM_COMPRESSED_DIR)/hello.readobj $(ROCM_COMPRESSED_DIR)/hello.readelf $(ROCM_COMPRESSED_DIR)/hip_fatbin.bin $(ROCM_COMPRESSED_DIR)/gfx950.co  ## build, dump, and split ROCm/HIP files

$(CUDA_DIR) $(CUDA_COMPRESSED_DIR) $(ROCM_DIR) $(ROCM_COMPRESSED_DIR):
	mkdir -p $@

## CUDA (uncompressed fatbin)

$(CUDA_DIR)/hello.so: hello.cu | $(CUDA_DIR)
	$(NVCC) $(NVCC_ARGS) $(GENCODE_FLAGS) $< -o $@

$(CUDA_DIR)/hello.cuobjdump: $(CUDA_DIR)/hello.so
	cuobjdump $< > $@

$(CUDA_DIR)/hello.readelf: $(CUDA_DIR)/hello.so
	readelf -W -a $< > $@

$(CUDA_DIR)/hello.sm_120.cubin: $(CUDA_DIR)/hello.so
	cd $(CUDA_DIR) && cuobjdump -xelf sm_120 $<

$(CUDA_DIR)/hello.compute_120.ptx: $(CUDA_DIR)/hello.so
	cd $(CUDA_DIR) && cuobjdump -xptx compute_120 $<

## CUDA (compressed fatbin)

$(CUDA_COMPRESSED_DIR)/hello.so: hello.cu | $(CUDA_COMPRESSED_DIR)
	$(NVCC) $(NVCC_ARGS) -Xfatbin -compress-all $(GENCODE_FLAGS) $< -o $@

$(CUDA_COMPRESSED_DIR)/hello.cuobjdump: $(CUDA_COMPRESSED_DIR)/hello.so
	cuobjdump $< > $@

$(CUDA_COMPRESSED_DIR)/hello.readelf: $(CUDA_COMPRESSED_DIR)/hello.so
	readelf -W -a $< > $@

$(CUDA_COMPRESSED_DIR)/hello.sm_120.cubin: $(CUDA_COMPRESSED_DIR)/hello.so
	cd $(CUDA_COMPRESSED_DIR) && cuobjdump -xelf sm_120 $<

$(CUDA_COMPRESSED_DIR)/hello.compute_120.ptx: $(CUDA_COMPRESSED_DIR)/hello.so
	cd $(CUDA_COMPRESSED_DIR) && cuobjdump -xptx compute_120 $<

## ROCm/HIP

$(ROCM_DIR)/hello.so: hello.cu | $(ROCM_DIR)
	$(HIPCC) -shared -fPIC $(HIPCC_OFFLOAD_ARCH) $< -o $@

$(ROCM_DIR)/hello.readobj: $(ROCM_DIR)/hello.so
	$(ROCM_LLVM)/llvm-readobj --offloading $< > $@

$(ROCM_DIR)/hello.readelf: $(ROCM_DIR)/hello.so
	$(ROCM_LLVM)/llvm-readelf -W -a $< > $@

$(ROCM_DIR)/hip_fatbin.bin: $(ROCM_DIR)/hello.so
	$(ROCM_LLVM)/llvm-objcopy --dump-section=.hip_fatbin=$@ $< /dev/null

$(ROCM_DIR)/gfx950.co: $(ROCM_DIR)/hip_fatbin.bin
	$(ROCM_LLVM)/clang-offload-bundler --type=o --list --input=$< > $(ROCM_DIR)/bundles.txt
	$(ROCM_LLVM)/clang-offload-bundler --type=o --unbundle \
		--input=$< \
		--targets="$$(grep gfx950 $(ROCM_DIR)/bundles.txt)" \
		--output=$@

## ROCm/HIP (compressed offload bundle)

$(ROCM_COMPRESSED_DIR)/hello.so: hello.cu | $(ROCM_COMPRESSED_DIR)
	$(HIPCC) -shared -fPIC --offload-compress $(HIPCC_OFFLOAD_ARCH) $< -o $@

$(ROCM_COMPRESSED_DIR)/hello.readobj: $(ROCM_COMPRESSED_DIR)/hello.so
	$(ROCM_LLVM)/llvm-readobj --offloading $< > $@

$(ROCM_COMPRESSED_DIR)/hello.readelf: $(ROCM_COMPRESSED_DIR)/hello.so
	$(ROCM_LLVM)/llvm-readelf -W -a $< > $@

$(ROCM_COMPRESSED_DIR)/hip_fatbin.bin: $(ROCM_COMPRESSED_DIR)/hello.so
	$(ROCM_LLVM)/llvm-objcopy --dump-section=.hip_fatbin=$@ $< /dev/null

$(ROCM_COMPRESSED_DIR)/gfx950.co: $(ROCM_COMPRESSED_DIR)/hip_fatbin.bin
	$(ROCM_LLVM)/clang-offload-bundler --type=o --list --input=$< > $(ROCM_COMPRESSED_DIR)/bundles.txt
	$(ROCM_LLVM)/clang-offload-bundler --type=o --unbundle \
		--input=$< \
		--targets="$$(grep gfx950 $(ROCM_COMPRESSED_DIR)/bundles.txt)" \
		--output=$@

## cleanup

.PHONY: clean
clean:  ## remove all build artifacts
	rm -rf $(CUDA_DIR) $(CUDA_COMPRESSED_DIR) $(ROCM_DIR) $(ROCM_COMPRESSED_DIR)
