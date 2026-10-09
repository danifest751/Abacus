// Goldilocks matmul baseline: CPU vs GPU throughput for candidate A.
//
// Field: P = 2**64 - 2**32 + 1. Device multiply uses the standard reduce128 (plonky3-style).
// This is a bounded throughput baseline that substantiates the "GPU-optimal" premise; it is not a
// miner or a work model.
//
// Build:  nvcc -O3 -arch=sm_75 goldilocks_matmul_bench.cu -o gl_mm_bench
// Run:    ./gl_mm_bench [n]

#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <vector>
#include <chrono>

#define P 0xFFFFFFFF00000001ULL
#define EPS 0xFFFFFFFFULL

__host__ __device__ __forceinline__ unsigned long long gl_add(unsigned long long a, unsigned long long b) {
    unsigned long long s = a + b;
    if (s < a) s += EPS;      // 2**64 == EPS (mod P)
    if (s >= P) s -= P;
    return s;
}

__device__ __forceinline__ unsigned long long gl_mul(unsigned long long a, unsigned long long b) {
    unsigned long long lo = a * b;
    unsigned long long hi = __umul64hi(a, b);
    unsigned long long hi_hi = hi >> 32;
    unsigned long long hi_lo = hi & 0xFFFFFFFFULL;
    unsigned long long t0 = lo - hi_hi;
    if (lo < hi_hi) t0 -= EPS;
    unsigned long long t1 = hi_lo * EPS;
    unsigned long long res = t0 + t1;
    if (res < t0) res += EPS;
    return res;
}

__host__ unsigned long long gl_mul_host(unsigned long long a, unsigned long long b) {
    unsigned __int128 prod = (unsigned __int128)a * (unsigned __int128)b;
    unsigned long long lo = (unsigned long long)prod;
    unsigned long long hi = (unsigned long long)(prod >> 64);
    unsigned long long hi_hi = hi >> 32;
    unsigned long long hi_lo = hi & 0xFFFFFFFFULL;
    unsigned long long t0 = lo - hi_hi;
    if (lo < hi_hi) t0 -= EPS;
    unsigned long long t1 = hi_lo * EPS;
    unsigned long long res = t0 + t1;
    if (res < t0) res += EPS;
    return res;
}

// Tiled matmul: C = A*B, row-major, n x n, one 16x16 tile per block.
#define TS 16
__global__ void matmul_kernel(const unsigned long long* A, const unsigned long long* B,
                              unsigned long long* C, int n) {
    __shared__ unsigned long long As[TS][TS];
    __shared__ unsigned long long Bs[TS][TS];
    int row = blockIdx.y * TS + threadIdx.y;
    int col = blockIdx.x * TS + threadIdx.x;
    unsigned long long acc = 0;
    for (int t = 0; t < n; t += TS) {
        As[threadIdx.y][threadIdx.x] = A[row * n + (t + threadIdx.x)];
        Bs[threadIdx.y][threadIdx.x] = B[(t + threadIdx.y) * n + col];
        __syncthreads();
        #pragma unroll
        for (int k = 0; k < TS; k++) acc = gl_add(acc, gl_mul(As[threadIdx.y][k], Bs[k][threadIdx.x]));
        __syncthreads();
    }
    C[row * n + col] = acc;
}

static void cpu_matmul(const std::vector<unsigned long long>& A, const std::vector<unsigned long long>& B,
                       std::vector<unsigned long long>& C, int n) {
    for (int i = 0; i < n; i++)
        for (int j = 0; j < n; j++) {
            unsigned long long acc = 0;
            for (int k = 0; k < n; k++) acc = gl_add(acc, gl_mul_host(A[i * n + k], B[k * n + j]));
            C[i * n + j] = acc;
        }
}

static unsigned long long xorshift(unsigned long long& s) {
    s ^= s << 13; s ^= s >> 7; s ^= s << 17; return s;
}

int main(int argc, char** argv) {
    int n = argc > 1 ? atoi(argv[1]) : 1024;
    std::vector<unsigned long long> A(n * n), B(n * n), Cgpu(n * n), Ccpu(n * n);
    unsigned long long s = 0x123456789ABCDEFULL;
    for (auto& x : A) x = xorshift(s) % P;
    for (auto& x : B) x = xorshift(s) % P;

    // GPU
    unsigned long long *dA, *dB, *dC;
    cudaMalloc(&dA, n * n * 8); cudaMalloc(&dB, n * n * 8); cudaMalloc(&dC, n * n * 8);
    cudaMemcpy(dA, A.data(), n * n * 8, cudaMemcpyHostToDevice);
    cudaMemcpy(dB, B.data(), n * n * 8, cudaMemcpyHostToDevice);
    dim3 block(TS, TS), grid(n / TS, n / TS);
    matmul_kernel<<<grid, block>>>(dA, dB, dC, n);   // warmup
    cudaDeviceSynchronize();
    auto t0 = std::chrono::high_resolution_clock::now();
    matmul_kernel<<<grid, block>>>(dA, dB, dC, n);
    cudaDeviceSynchronize();
    auto t1 = std::chrono::high_resolution_clock::now();
    double gpu_s = std::chrono::duration<double>(t1 - t0).count();
    cudaMemcpy(Cgpu.data(), dC, n * n * 8, cudaMemcpyDeviceToHost);
    cudaError_t err = cudaGetLastError();

    // CPU (skip for large n to bound the run)
    double cpu_s = -1.0;
    bool cpu_ok = true;
    if (n <= 512) {
        auto c0 = std::chrono::high_resolution_clock::now();
        cpu_matmul(A, B, Ccpu, n);
        auto c1 = std::chrono::high_resolution_clock::now();
        cpu_s = std::chrono::duration<double>(c1 - c0).count();
        for (int i = 0; i < n * n; i++) if (Ccpu[i] != Cgpu[i]) { cpu_ok = false; break; }
    }

    double macs = (double)n * n * n;
    printf("{\n");
    printf("  \"n\": %d,\n", n);
    printf("  \"gpu_s\": %.6f,\n", gpu_s);
    printf("  \"gpu_GMAC_s\": %.2f,\n", macs / gpu_s / 1e9);
    if (cpu_s > 0) {
        printf("  \"cpu_s\": %.6f,\n", cpu_s);
        printf("  \"cpu_GMAC_s\": %.4f,\n", macs / cpu_s / 1e9);
        printf("  \"gpu_over_cpu\": %.1f,\n", cpu_s / gpu_s);
        printf("  \"cpu_matches_gpu\": %s,\n", cpu_ok ? "true" : "false");
    }
    printf("  \"cuda_error\": %s\n", err == cudaSuccess ? "\"\"" : cudaGetErrorString(err));
    printf("}\n");
    return 0;
}
