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
// (docs/research/attempt-rate-v1.md); v2 results are in docs/research/attempt-rate-v2.md. Modes 1/2
// reproduce the v1 fold and the prefix-sum attack on it. Timing uses CUDA events after a warm-up.
//
// Build: nvcc -O3 -arch=sm_75 attempt_bench.cu -o attempt_bench
// Run:   ./attempt_bench [n] [seg_bytes] [dataset_MiB] [attempts] [fold 0=mix|1=sum|2=prefix] [warm_s]

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

// Fold modes: 0 = sequential nonlinear mix (v2, every word must be read);
//             1 = plain u64 sum (v1, linear);
//             2 = the prefix-sum attack on mode 1: two reads per entry from a prefix table.
enum { FOLD_MIX = 0, FOLD_SUM = 1, FOLD_PREFIX = 2 };

// Gather one field element per A entry from a random `seg`-byte dataset segment.
__global__ void gather_A(const unsigned long long* __restrict__ D, unsigned long long dwords,
                         unsigned long long* __restrict__ A, int n, int seg, unsigned long long seed, int fold) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    int total = n * n;
    if (i >= total) return;
    unsigned long long h = (seed + 1) * 0x9E3779B97F4A7C15ULL + (unsigned long long)i * 0xD1B54A32D192ED03ULL;
    h ^= h >> 29;
    unsigned long long segw = (unsigned long long)(seg / 8);
    unsigned long long off = (h % (dwords - segw - 1)) & ~1ULL;
    const unsigned long long* p = D + off;
    unsigned long long acc = 0;
    if (fold == FOLD_MIX) {
        for (unsigned long long k = 0; k < segw; ++k) acc = mix64(acc ^ p[k]);
    } else if (fold == FOLD_SUM) {
        for (unsigned long long k = 0; k < segw; ++k) acc += p[k];
    } else {
        acc = p[segw] - p[0]; // D read as the per-epoch prefix table: sum(D[off..off+segw))
    }
    A[i] = acc % P;
}

// Fill the dataset with pseudo-random words (v1 used a constant 0x5A fill).
__global__ void fill(unsigned long long* D, unsigned long long dwords) {
    unsigned long long i = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
    for (; i < dwords; i += (unsigned long long)gridDim.x * blockDim.x) D[i] = mix64(i + 0x5851F42D4C957F2DULL);
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

static const char* fold_name(int f) { return f == FOLD_MIX ? "mix" : f == FOLD_SUM ? "sum" : "prefix"; }

int main(int argc, char** argv) {
    int n = argc > 1 ? atoi(argv[1]) : 256;
    int seg = argc > 2 ? atoi(argv[2]) : 2560;
    int ds_mib = argc > 3 ? atoi(argv[3]) : 2048;
    int attempts = argc > 4 ? atoi(argv[4]) : 64;
    int fold = argc > 5 ? atoi(argv[5]) : FOLD_MIX;
    double warm_s = argc > 6 ? atof(argv[6]) : 3.0;

    unsigned long long db = (unsigned long long)ds_mib * 1024 * 1024;
    unsigned long long dwords = db / 8;
    unsigned long long* D; cudaMalloc(&D, db);
    fill<<<1024, 256>>>(D, dwords);
    unsigned long long *A, *B, *C;
    cudaMalloc(&A, (size_t)n * n * 8); cudaMalloc(&B, (size_t)n * n * 8); cudaMalloc(&C, (size_t)n * n * 8);
    cudaMemset(B, 0x11, (size_t)n * n * 8);

    int total = n * n;
    int block = 256, grid = (total + block - 1) / block;
    dim3 mblock(TS, TS), mgrid(n / TS, n / TS);

    // Warm-up: keep the GPU busy for warm_s seconds so the idle governor has raised the clocks.
    auto w0 = std::chrono::high_resolution_clock::now();
    for (unsigned long long a = 0;; ++a) {
        gather_A<<<grid, block>>>(D, dwords, A, n, seg, a, fold);
        matmul<<<mgrid, mblock>>>(A, B, C, n);
        cudaDeviceSynchronize();
        if (std::chrono::duration<double>(std::chrono::high_resolution_clock::now() - w0).count() >= warm_s) break;
    }

    cudaEvent_t e0, e1;
    cudaEventCreate(&e0); cudaEventCreate(&e1);
    float ms_gather = 0, ms_matmul = 0, ms_total = 0;
    cudaEventRecord(e0);
    for (int a = 0; a < attempts; ++a) gather_A<<<grid, block>>>(D, dwords, A, n, seg, 1000 + a, fold);
    cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms_gather, e0, e1);
    cudaEventRecord(e0);
    for (int a = 0; a < attempts; ++a) matmul<<<mgrid, mblock>>>(A, B, C, n);
    cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms_matmul, e0, e1);
    cudaEventRecord(e0);
    for (int a = 0; a < attempts; ++a) {
        gather_A<<<grid, block>>>(D, dwords, A, n, seg, 5000 + a, fold);
        matmul<<<mgrid, mblock>>>(A, B, C, n);
    }
    cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms_total, e0, e1);

    double read_bytes = fold == FOLD_PREFIX ? 16.0 : (double)seg; // bytes actually read per entry
    double s_total = ms_total / 1e3, s_gather = ms_gather / 1e3;
    printf("{\"n\": %d, \"seg_bytes\": %d, \"dataset_MiB\": %d, \"attempts\": %d, \"fold\": \"%s\", "
           "\"attempts_per_s\": %.1f, \"gather_ms\": %.3f, \"matmul_ms\": %.3f, "
           "\"gather_over_matmul\": %.2f, \"read_MB_per_attempt\": %.1f, \"gather_read_GB_s\": %.1f, "
           "\"cuda_error\": \"%s\"}\n",
           n, seg, ds_mib, attempts, fold_name(fold), attempts / s_total, ms_gather / attempts,
           ms_matmul / attempts, ms_gather / ms_matmul, total * read_bytes / 1e6,
           total * read_bytes * attempts / s_gather / 1e9, cudaGetErrorString(cudaGetLastError()));
    return 0;
}
