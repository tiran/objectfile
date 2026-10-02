#ifdef __HIPCC__
#include <hip/hip_runtime.h>
#endif
#include <stdio.h>

__global__ void cuda_hello(){
	    printf("Hello World from GPU!\n");
}

extern "C" void hello() {
	cuda_hello<<<1,1>>>();
}
