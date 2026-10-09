// Attempt-rate bench for candidate A' (memory-hard): gather operands from a dataset, matmul, score.
//
// Per attempt: n^2 gathered field elements (each folded from a `seg`-byte dataset segment), then a
// Goldilocks n x n matmul. Measures attempts/s and effective gather bandwidth at the designed gather
// volume (default n=256, seg=2560 B -> ~168 MB/attempt, dataset 2 GiB).
//
// v2 (ADR 0010): the segment fold is a **nonlinear, sequential** mix. v1 summed the segment, which a
// miner answers with two prefix-sum reads (precomputed once per epoch), so v1 did not measure a
// memory-hard gather. The gather is still one thread per entry (uncoalesced); a warp-cooperative
// gather is the honest-miner baseline and is not implemented here. v1 numbers are withdrawn
// (docs/research/attempt-rate-v1.md); v2 has not been compiled or measured yet.
//
// Build: nvcc -O3 -arch=sm_75 attempt_bench.cu -o attempt_bench
// Run:   ./attempt_bench [n] [seg_bytes] [dataset_MiB] [attempts]

#include <cstdio>
#include <cstdlib>
#include <cuda_runtime.h>
#include <chrono>

#define P 0xFFFFFFFF00000001ULL
#define EPS 0xFFFFFFFFULL

__host__ __device__ __forceinline__ unsigned long long gl_add(unsigned long long a, unsigned long long b) {
    unsigned long long s = a + b;
    if (s < a) s += EPS;
    if (s >= P) s -= P;
    return s;
}
__device__ __forceinline__ unsigned long long gl_mul(unsigned long long a, unsigned long long b) {
    unsigned long long lo = a * b, hi = __umul64hi(a, b);
    unsigned long long hh = hi >> 32, hl = hi & 0xFFFFFFFFULL;
    unsigned long long t0 = lo - hh; if (lo < hh) t0 -= EPS;
    unsigned long long t1 = hl * EPS;
    unsigned long long r = t0 + t1; if (r < t0) r += EPS;
    return r;
}

// SplitMix64 finalizer: a bijective nonlinear mix, so the fold below is not decomposable.
__device__ __forceinline__ unsigned long long mix64(unsigned long long x) {
    x ^= x >> 30; x *= 0xBF58476D1CE4E5B9ULL;
    x ^= x >> 27; x *= 0x94D049BB133111EBULL;
    return x ^ (x >> 31);
}

// Gather one field element per A entry by folding a random `seg`-byte dataset segment with a
// sequential nonlinear mix (no prefix-sum shortcut; every word of the segment must be read).
__global__ void gather_A(const unsigned char* __restrict__ D, unsigned long long dwords,
                         unsigned long long* __restrict__ A, int n, int seg, unsigned long long seed) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    int total = n * n;
    if (i >= total) return;
    unsigned long long h = (seed + 1) * 0x9E3779B97F4A7C15ULL + (unsigned long long)i * 0xD1B54A32D192ED03ULL;
    h ^= h >> 29;
    unsigned long long segw = (unsigned long long)(seg / 8);
    unsigned long long off = (h % (dwords - segw)) & ~1ULL;
    const unsigned long long* p = reinterpret_cast<const unsigned long long*>(D) + off;
    unsigned long long acc = 0;
    for (unsigned long long k = 0; k < segw; ++k) acc = mix64(acc ^ p[k]);
    A[i] = acc % P;
}

#define TS 16
__global__ void matmul(const unsigned long long* __restrict__ A, const unsigned long long* __restrict__ B,
                       unsigned long long* __restrict__ C, int n) {
    __shared__ unsigned long long As[TS][TS], Bs[TS][TS];
    int row = blockIdx.y * TS + threadIdx.y, col = blockIdx.x * TS + threadIdx.x;
    unsigned long long acc = 0;
    for (int t = 0; t < n; t += TS) {
        As[threadIdx.y][threadIdx.x] = A[row * n + (t + threadIdx.x)];
        Bs[threadIdx.y][threadIdx.x] = B[(t + threadIdx.y) * n + col];
        __syncthreads();
        #pragma unroll
        for (int k = 0; k < TS; ++k) acc = gl_add(acc, gl_mul(As[threadIdx.y][k], Bs[k][threadIdx.x]));
        __syncthreads();
    }
    C[row * n + col] = acc;
}

int main(int argc, char** argv) {
    int n = argc > 1 ? atoi(argv[1]) : 256;
    int seg = argc > 2 ? atoi(argv[2]) : 2560;
    int ds_mib = argc > 3 ? atoi(argv[3]) : 2048;
    int attempts = argc > 4 ? atoi(argv[4]) : 64;

    unsigned long long db = (unsigned long long)ds_mib * 1024 * 1024;
    unsigned long long dwords = db / 8;
    unsigned char* D; cudaMalloc(&D, db); cudaMemset(D, 0x5A, db);
    unsigned long long *A, *B, *C;
    cudaMalloc(&A, (size_t)n * n * 8); cudaMalloc(&B, (size_t)n * n * 8); cudaMalloc(&C, (size_t)n * n * 8);
    cudaMemset(B, 0x11, (size_t)n * n * 8);

    int total = n * n;
    int block = 256, grid = (total + block - 1) / block;
    dim3 mblock(TS, TS), mgrid(n / TS, n / TS);

    // warmup
    gather_A<<<grid, block>>>(D, dwords, A, n, seg, 1);
    matmul<<<mgrid, mblock>>>(A, B, C, n);
    cudaDeviceSynchronize();

    double bytes_per_attempt = (double)total * seg;
    auto t0 = std::chrono::high_resolution_clock::now();
    for (int a = 0; a < attempts; ++a) {
        gather_A<<<grid, block>>>(D, dwords, A, n, seg, (unsigned long long)a);
        matmul<<<mgrid, mblock>>>(A, B, C, n);
    }
    cudaDeviceSynchronize();
    auto t1 = std::chrono::high_resolution_clock::now();
    double s = std::chrono::duration<double>(t1 - t0).count();

    printf("{\n");
    printf("  \"n\": %d, \"seg_bytes\": %d, \"dataset_MiB\": %d, \"attempts\": %d,\n", n, seg, ds_mib, attempts);
    printf("  \"attempts_per_s\": %.1f,\n", attempts / s);
    printf("  \"gather_MB_per_attempt\": %.1f,\n", bytes_per_attempt / 1e6);
    printf("  \"effective_gather_GB_s\": %.1f,\n", bytes_per_attempt * attempts / s / 1e9);
    printf("  \"cuda_error\": \"%s\"\n}\n", cudaGetErrorString(cudaGetLastError()));
    return 0;
}
