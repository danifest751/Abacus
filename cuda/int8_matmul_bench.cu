// int8 matmul bench for candidate A8 (spec/05, ADR 0012): exact C = A * B, int8 inputs, int32 result.
//
// Two GPU paths on the same random int8 matrices:
//   - cublas: cublasGemmEx with CUDA_R_8I inputs, CUDA_R_32I output, CUBLAS_COMPUTE_32I (tensor-core
//     IMMA on sm_75; the tuned honest miner);
//   - simt:   a plain 16x16 tiled kernel on the CUDA cores (int32 multiply-add), for the tensor-core
//     contribution.
// Both are checked exactly against a CPU product for n <= 512. The GPU is warmed up for `warm_s`
// seconds, then each path is timed with CUDA events `reps` times; medians are reported.
//
// cuBLAS is column-major: with op(A) = A^T, C_cm(i, j) = sum_k Abuf[k + i n] * Bbuf[k + j n].
// The CPU reference uses the same indexing, so the comparison is exact.
//
// Build: nvcc -O3 -arch=sm_75 int8_matmul_bench.cu -lcublas -o int8_matmul_bench
// Run:   ./int8_matmul_bench [n] [reps] [warm_s]

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>
#include <cublas_v2.h>
#include <cuda_runtime.h>

// ---- score hashing cost: SHA-256 of C in independent 1 KiB chunks (one thread per chunk) ----
__constant__ uint32_t cK[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
    0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
    0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
    0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
    0xc67178f2};
__device__ __forceinline__ uint32_t rotr(uint32_t x, int s) { return (x >> s) | (x << (32 - s)); }
__device__ void sha_compress(uint32_t h[8], const uint32_t* blk) {  // blk: 16 big-endian words
    uint32_t w[64];
#pragma unroll
    for (int i = 0; i < 16; ++i) w[i] = blk[i];
#pragma unroll
    for (int i = 16; i < 64; ++i)
        w[i] = w[i - 16] + (rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3)) + w[i - 7] +
               (rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10));
    uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
#pragma unroll
    for (int i = 0; i < 64; ++i) {
        uint32_t t1 = hh + (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ (~e & g)) + cK[i] + w[i];
        uint32_t t2 = (rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c));
        hh = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
    }
    h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
}
// Each thread hashes one 1 KiB chunk (16 compressions; padding block omitted: cost model only).
__global__ void hash_chunks(const uint32_t* __restrict__ data, size_t words, uint32_t* __restrict__ out) {
    const size_t t = (size_t)blockIdx.x * blockDim.x + threadIdx.x;
    const size_t base = t * 256;  // 256 words = 1 KiB
    if (base >= words) return;
    uint32_t h[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    for (int b = 0; b < 16; ++b) sha_compress(h, data + base + 16 * b);
#pragma unroll
    for (int i = 0; i < 8; ++i) out[t * 8 + i] = h[i];
}

#define TS 16
// C_cm(i, j) = sum_k A[k + i n] * B[k + j n]  (same semantics as the cuBLAS call below)
__global__ void simt_i8(const int8_t* __restrict__ A, const int8_t* __restrict__ B, int32_t* __restrict__ C, int n) {
    __shared__ int32_t As[TS][TS], Bs[TS][TS];
    const int i = blockIdx.y * TS + threadIdx.y, j = blockIdx.x * TS + threadIdx.x;
    int32_t acc = 0;
    for (int t = 0; t < n; t += TS) {
        As[threadIdx.y][threadIdx.x] = A[(t + threadIdx.x) + (size_t)i * n];
        Bs[threadIdx.y][threadIdx.x] = B[(t + threadIdx.y) + (size_t)j * n];
        __syncthreads();
#pragma unroll
        for (int k = 0; k < TS; ++k) acc += As[threadIdx.y][k] * Bs[k][threadIdx.x];
        __syncthreads();
    }
    C[i + (size_t)j * n] = acc;
}

// Freivalds check of the GPU result on the host over Goldilocks (all n): C r == A (B r) mod P, with the
// same column-major semantics. Integer errors are < 2^32 < P, so a wrong C fails with prob. >= 1 - 2^-63.
static const unsigned long long GP = 0xFFFFFFFF00000001ULL;
static unsigned long long fmod_i(long long x) {
    return x >= 0 ? (unsigned long long)x % GP : GP - ((unsigned long long)(-x) % GP);
}
static bool freivalds_host(const std::vector<int8_t>& A, const std::vector<int8_t>& B, const std::vector<int32_t>& C,
                           int n, unsigned long long seed) {
    std::vector<unsigned long long> r(n), br(n), abr(n), cr(n);
    for (int j = 0; j < n; ++j) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; r[j] = seed % GP; }
    for (int k = 0; k < n; ++k) {  // br_k = sum_j B[k + j n] r_j
        unsigned __int128 acc = 0;
        for (int j = 0; j < n; ++j) acc = (acc + (unsigned __int128)fmod_i(B[k + (size_t)j * n]) * r[j]) % GP;
        br[k] = (unsigned long long)acc;
    }
    for (int i = 0; i < n; ++i) {  // abr_i = sum_k A[k + i n] br_k ; cr_i = sum_j C[i + j n] r_j
        unsigned __int128 acc = 0, acc2 = 0;
        for (int k = 0; k < n; ++k) acc = (acc + (unsigned __int128)fmod_i(A[k + (size_t)i * n]) * br[k]) % GP;
        for (int j = 0; j < n; ++j) acc2 = (acc2 + (unsigned __int128)fmod_i(C[i + (size_t)j * n]) * r[j]) % GP;
        abr[i] = (unsigned long long)acc;
        cr[i] = (unsigned long long)acc2;
    }
    return abr == cr;
}

static double median(std::vector<double> v) {
    std::sort(v.begin(), v.end());
    return v[v.size() / 2];
}

int main(int argc, char** argv) {
    const int n = argc > 1 ? atoi(argv[1]) : 1024;
    const int reps = argc > 2 ? atoi(argv[2]) : 7;
    const double warm_s = argc > 3 ? atof(argv[3]) : 3.0;
    if (n <= 0 || n % 16) { fprintf(stderr, "n must be a positive multiple of 16\n"); return 1; }
    const size_t nn = (size_t)n * n;

    std::vector<int8_t> A(nn), B(nn);
    unsigned long long s = 0x9E3779B97F4A7C15ULL;
    auto rnd = [&]() { s ^= s << 13; s ^= s >> 7; s ^= s << 17; return (int8_t)(s >> 24); };
    for (auto& x : A) x = rnd();
    for (auto& x : B) x = rnd();

    int8_t *dA, *dB; int32_t *dC, *dC2;
    cudaMalloc(&dA, nn); cudaMalloc(&dB, nn); cudaMalloc(&dC, nn * 4); cudaMalloc(&dC2, nn * 4);
    cudaMemcpy(dA, A.data(), nn, cudaMemcpyHostToDevice);
    cudaMemcpy(dB, B.data(), nn, cudaMemcpyHostToDevice);
    cublasHandle_t h;
    if (cublasCreate(&h) != CUBLAS_STATUS_SUCCESS) { fprintf(stderr, "cublasCreate failed\n"); return 1; }
    const int32_t alpha = 1, beta = 0;
    auto gemm = [&]() {
        return cublasGemmEx(h, CUBLAS_OP_T, CUBLAS_OP_N, n, n, n, &alpha, dA, CUDA_R_8I, n, dB, CUDA_R_8I, n, &beta,
                            dC, CUDA_R_32I, n, CUBLAS_COMPUTE_32I, CUBLAS_GEMM_DEFAULT);
    };
    dim3 block(TS, TS), grid(n / TS, n / TS);
    if (gemm() != CUBLAS_STATUS_SUCCESS) { fprintf(stderr, "cublasGemmEx failed\n"); return 1; }

    auto w0 = std::chrono::high_resolution_clock::now();
    do {
        gemm();
        simt_i8<<<grid, block>>>(dA, dB, dC2, n);
        cudaDeviceSynchronize();
    } while (std::chrono::duration<double>(std::chrono::high_resolution_clock::now() - w0).count() < warm_s);

    cudaEvent_t e0, e1;
    cudaEventCreate(&e0); cudaEventCreate(&e1);
    const size_t chunks = nn * 4 / 1024;
    uint32_t* dH; cudaMalloc(&dH, (chunks + 1) * 32);
    std::vector<double> tc, ts, th;
    for (int r = 0; r < reps; ++r) {
        float ms = 0;
        cudaEventRecord(e0); gemm(); cudaEventRecord(e1); cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1); tc.push_back(ms / 1e3);
        cudaEventRecord(e0); simt_i8<<<grid, block>>>(dA, dB, dC2, n); cudaEventRecord(e1); cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1); ts.push_back(ms / 1e3);
        cudaEventRecord(e0);
        hash_chunks<<<(unsigned)((chunks + 255) / 256), 256>>>((const uint32_t*)dC, nn, dH);
        cudaEventRecord(e1); cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1); th.push_back(ms / 1e3);
    }

    bool exact_cublas = true, exact_simt = true, checked = false;
    double cpu_s = -1;
    if (n <= 512) {
        std::vector<int32_t> C(nn), C2(nn), R(nn);
        cudaMemcpy(C.data(), dC, nn * 4, cudaMemcpyDeviceToHost);
        cudaMemcpy(C2.data(), dC2, nn * 4, cudaMemcpyDeviceToHost);
        auto c0 = std::chrono::high_resolution_clock::now();
        for (int j = 0; j < n; ++j)
            for (int i = 0; i < n; ++i) {
                int32_t acc = 0;
                for (int k = 0; k < n; ++k) acc += (int32_t)A[k + (size_t)i * n] * (int32_t)B[k + (size_t)j * n];
                R[i + (size_t)j * n] = acc;
            }
        cpu_s = std::chrono::duration<double>(std::chrono::high_resolution_clock::now() - c0).count();
        exact_cublas = C == R;
        exact_simt = C2 == R;
        checked = true;
    }

    std::vector<int32_t> Cg(nn);
    cudaMemcpy(Cg.data(), dC, nn * 4, cudaMemcpyDeviceToHost);
    const bool freivalds_cublas = freivalds_host(A, B, Cg, n, 0x1234567ULL) && freivalds_host(A, B, Cg, n, 0xABCDEFULL);
    std::vector<int32_t> Cbad = Cg;  // self-test: one corrupted entry must be caught
    Cbad[nn / 3] += 1;
    const bool freivalds_catches = !freivalds_host(A, B, Cbad, n, 0x1234567ULL);

    const double macs = (double)n * n * n, t_tc = median(tc), t_s = median(ts), t_h = median(th);
    printf("{\"n\": %d, \"reps\": %d, \"warm_s\": %.1f, "
           "\"cublas_s\": %.7f, \"cublas_s_min\": %.7f, \"cublas_s_max\": %.7f, \"cublas_TMAC_s\": %.2f, "
           "\"simt_s\": %.7f, \"simt_TMAC_s\": %.3f, \"tensor_over_simt\": %.1f, ",
           n, reps, warm_s, t_tc, *std::min_element(tc.begin(), tc.end()), *std::max_element(tc.begin(), tc.end()),
           macs / t_tc / 1e12, t_s, macs / t_s / 1e12, t_s / t_tc);
    printf("\"hash_C_s\": %.7f, \"hash_C_GB_s\": %.1f, \"hash_over_cublas\": %.3f, ", t_h, nn * 4 / t_h / 1e9,
           t_h / t_tc);
    if (checked)
        printf("\"cpu_naive_s\": %.4f, \"cpu_naive_GMAC_s\": %.3f, \"exact_cublas\": %s, \"exact_simt\": %s, ", cpu_s,
               macs / cpu_s / 1e9, exact_cublas ? "true" : "false", exact_simt ? "true" : "false");
    printf("\"freivalds_cublas\": %s, \"freivalds_catches_error\": %s, \"cuda_error\": \"%s\"}\n",
           freivalds_cublas ? "true" : "false", freivalds_catches ? "true" : "false",
           cudaGetErrorString(cudaGetLastError()));
    cublasDestroy(h);
    return 0;
}
