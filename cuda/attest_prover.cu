// GPU prover for the interactive proof of tensor throughput (spec/06, crates/abacus-attest).
//
// Serves the line protocol of `abacus-attest`:
//   CHAL <seed hex> <n> <m>  ->  derive A_j, B_j (SHA-256 counter mode on the device), C_j = A_j * B_j with
//                                cuBLAS int8 (tensor cores), hash every row of every C_j and build the
//                                Merkle tree on the device  ->  ROOT <hex>
//   OPEN j:i ...             ->  ROW <j> <i> <row hex> <path hex> ... END
// Byte-for-byte the same derivations as the Rust reference (product seeds, expansion, leaves, tree).
// Per-phase timings are printed to stderr as JSON.
//
// --cheat-rows C computes only the first n - C rows of every product and commits zeros for the rest
// (a prover skipping work), to measure detection. --pipelined uses two CUDA streams so the expansion of
// one product can overlap the GEMM of the other (measured: +16% at n = 4096, no gain at n >= 8192 on a
// CMP 50HX, because the GEMM occupies every SM); the default runs one stream and reports per-phase times.
//
// Build: nvcc -O3 -arch=sm_75 attest_prover.cu -lcublas -o attest_prover
// Run:   ./attest_prover --listen 9600 [--cheat-rows C] [--pipelined]

#include <algorithm>
#include <array>
#include <arpa/inet.h>
#include <chrono>
#include <csignal>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <string>
#include <sys/socket.h>
#include <unistd.h>
#include <vector>
#include <cublas_v2.h>
#include <cuda_runtime.h>

// ---------------- SHA-256 (host and device) ----------------
#define K256_INIT                                                                                                  \
    {0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,  \
     0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,  \
     0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,  \
     0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,  \
     0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,  \
     0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,  \
     0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,  \
     0xc67178f2}
static const uint32_t hK[64] = K256_INIT;
__constant__ uint32_t dK[64] = K256_INIT;

__host__ __device__ __forceinline__ uint32_t rotr(uint32_t x, int s) { return (x >> s) | (x << (32 - s)); }

template <bool DEV>
__host__ __device__ __forceinline__ void compress(uint32_t h[8], const uint8_t* blk) {
    uint32_t w[64];
    for (int i = 0; i < 16; ++i)
        w[i] = ((uint32_t)blk[4 * i] << 24) | ((uint32_t)blk[4 * i + 1] << 16) | ((uint32_t)blk[4 * i + 2] << 8) |
               blk[4 * i + 3];
    for (int i = 16; i < 64; ++i)
        w[i] = w[i - 16] + (rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3)) + w[i - 7] +
               (rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10));
    uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
    for (int i = 0; i < 64; ++i) {
#ifdef __CUDA_ARCH__
        const uint32_t k = dK[i];
#else
        const uint32_t k = hK[i];
#endif
        uint32_t t1 = hh + (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ (~e & g)) + k + w[i];
        uint32_t t2 = (rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c));
        hh = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
    }
    h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
}

// SHA-256 of prefix[0..plen) || body[0..blen) (body may live in device memory on the device).
template <bool DEV>
__host__ __device__ void sha256_two(const uint8_t* prefix, int plen, const uint8_t* body, size_t blen, uint8_t out[32]) {
    uint32_t h[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    const size_t total = (size_t)plen + blen, padded = ((total + 9 + 63) / 64) * 64;
    const uint64_t bits = (uint64_t)total * 8;
    uint8_t blk[64];
    for (size_t off = 0; off < padded; off += 64) {
        for (int t = 0; t < 64; ++t) {
            const size_t p = off + t;
            uint8_t v;
            if (p < (size_t)plen) v = prefix[p];
            else if (p < total) v = body[p - plen];
            else if (p == total) v = 0x80;
            else if (p >= padded - 8) v = (uint8_t)(bits >> (8 * (padded - 1 - p)));
            else v = 0;
            blk[t] = v;
        }
        compress<DEV>(h, blk);
    }
    for (int i = 0; i < 8; ++i) {
        out[4 * i] = h[i] >> 24; out[4 * i + 1] = h[i] >> 16; out[4 * i + 2] = h[i] >> 8; out[4 * i + 3] = h[i];
    }
}

static void hsha(const std::vector<uint8_t>& m, uint8_t out[32]) { sha256_two<false>(m.data(), (int)m.size(), nullptr, 0, out); }

// ---------------- device kernels ----------------
// Word-level SHA-256 on the device: w0 holds 16 big-endian message words.
__device__ __forceinline__ void compress_w(uint32_t h[8], const uint32_t w0[16]) {
    uint32_t w[64];
#pragma unroll
    for (int i = 0; i < 16; ++i) w[i] = w0[i];
#pragma unroll
    for (int i = 16; i < 64; ++i)
        w[i] = w[i - 16] + (rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3)) + w[i - 7] +
               (rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10));
    uint32_t a = h[0], b = h[1], c = h[2], d = h[3], e = h[4], f = h[5], g = h[6], hh = h[7];
#pragma unroll
    for (int i = 0; i < 64; ++i) {
        uint32_t t1 = hh + (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ (~e & g)) + dK[i] + w[i];
        uint32_t t2 = (rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c));
        hh = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
    }
    h[0] += a; h[1] += b; h[2] += c; h[3] += d; h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
}
__device__ __forceinline__ void sha_init(uint32_t h[8]) {
    h[0] = 0x6a09e667; h[1] = 0xbb67ae85; h[2] = 0x3c6ef372; h[3] = 0xa54ff53a;
    h[4] = 0x510e527f; h[5] = 0x9b05688c; h[6] = 0x1f83d9ab; h[7] = 0x5be0cd19;
}
__device__ __forceinline__ void store_digest(const uint32_t h[8], uint8_t* out) {
#pragma unroll
    for (int i = 0; i < 8; ++i) {
        out[4 * i] = h[i] >> 24; out[4 * i + 1] = h[i] >> 16; out[4 * i + 2] = h[i] >> 8; out[4 * i + 3] = h[i];
    }
}
__device__ __forceinline__ uint32_t be_word(const uint8_t* b) {
    return ((uint32_t)b[0] << 24) | ((uint32_t)b[1] << 16) | ((uint32_t)b[2] << 8) | b[3];
}

// out[32 c .. 32 c + 32) = SHA256("abacus/expand" || seed || LE32(c)), c in [0, count).
// pw: the first 44 message bytes ("abacus/expand" || seed[0..31]) as 11 words; b44 = seed[31].
struct ExpandPrefix { uint32_t pw[11]; uint32_t b44; };
__global__ void expand_kernel(ExpandPrefix px, size_t count, uint8_t* __restrict__ out) {
    const size_t c = (size_t)blockIdx.x * blockDim.x + threadIdx.x;
    if (c >= count) return;
    const uint32_t cc = (uint32_t)c;
    uint32_t w[16], h[8];
#pragma unroll
    for (int i = 0; i < 11; ++i) w[i] = px.pw[i];
    w[11] = (px.b44 << 24) | ((cc & 0xff) << 16) | (((cc >> 8) & 0xff) << 8) | ((cc >> 16) & 0xff);
    w[12] = ((cc >> 24) << 24) | (0x80u << 16);
    w[13] = 0; w[14] = 0; w[15] = 49 * 8;
    sha_init(h);
    compress_w(h, w);
    store_digest(h, out + 32 * c);
}

// BT[a n + k] = B[k n + a]
__global__ void transpose_i8(const int8_t* __restrict__ B, int8_t* __restrict__ BT, int n) {
    __shared__ int8_t tile[32][33];
    const int bx = blockIdx.x * 32, by = blockIdx.y * 32;
    for (int y = threadIdx.y; y < 32; y += 8) tile[y][threadIdx.x] = B[(size_t)(by + y) * n + bx + threadIdx.x];
    __syncthreads();
    for (int y = threadIdx.y; y < 32; y += 8) BT[(size_t)(bx + y) * n + by + threadIdx.x] = tile[threadIdx.x][y];
}

// leaf[i] = SHA256(C row i as LE int32 || 0x00 || LE32(j) || LE32(i)); n % 16 == 0, so the row is
// n / 16 whole blocks read with aligned 16-byte loads, followed by one suffix/padding block.
__global__ void leaf_kernel(const int32_t* __restrict__ C, int n, uint32_t j, uint8_t* __restrict__ leaves) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= n) return;
    const uint4* row = reinterpret_cast<const uint4*>(C + (size_t)i * n);
    uint32_t h[8], w[16];
    sha_init(h);
    for (int blk = 0; blk < n / 16; ++blk) {
#pragma unroll
        for (int q = 0; q < 4; ++q) {
            const uint4 v = row[blk * 4 + q];
            w[4 * q] = __byte_perm(v.x, 0, 0x0123); w[4 * q + 1] = __byte_perm(v.y, 0, 0x0123);
            w[4 * q + 2] = __byte_perm(v.z, 0, 0x0123); w[4 * q + 3] = __byte_perm(v.w, 0, 0x0123);
        }
        compress_w(h, w);
    }
    const uint32_t ui = (uint32_t)i;
    const uint64_t bits = ((uint64_t)n * 4 + 9) * 8;
    w[0] = ((j & 0xff) << 16) | (((j >> 8) & 0xff) << 8) | ((j >> 16) & 0xff);
    w[1] = ((j >> 24) << 24) | ((ui & 0xff) << 16) | (((ui >> 8) & 0xff) << 8) | ((ui >> 16) & 0xff);
    w[2] = ((ui >> 24) << 24) | (0x80u << 16);
#pragma unroll
    for (int t = 3; t < 14; ++t) w[t] = 0;
    w[14] = (uint32_t)(bits >> 32); w[15] = (uint32_t)bits;
    compress_w(h, w);
    store_digest(h, leaves + 32 * (size_t)i);
}

// out[t] = SHA256(0x01 || in[2t] || in[2t+1]) (65 bytes, two blocks)
__global__ void node_kernel(const uint8_t* __restrict__ in, size_t count, uint8_t* __restrict__ out) {
    const size_t t = (size_t)blockIdx.x * blockDim.x + threadIdx.x;
    if (t >= count) return;
    uint8_t m[128];
    m[0] = 1;
    for (int q = 0; q < 64; ++q) m[1 + q] = in[64 * t + q];
    m[65] = 0x80;
    for (int q = 66; q < 126; ++q) m[q] = 0;
    m[126] = (65 * 8) >> 8; m[127] = (65 * 8) & 0xff;
    uint32_t h[8], w[16];
    sha_init(h);
    for (int b = 0; b < 2; ++b) {
#pragma unroll
        for (int q = 0; q < 16; ++q) w[q] = be_word(m + 64 * b + 4 * q);
        compress_w(h, w);
    }
    store_digest(h, out + 32 * t);
}

// ---------------- host helpers ----------------
static std::string tohex(const uint8_t* p, size_t n) {
    static const char* H = "0123456789abcdef";
    std::string s(2 * n, '0');
    for (size_t i = 0; i < n; ++i) { s[2 * i] = H[p[i] >> 4]; s[2 * i + 1] = H[p[i] & 15]; }
    return s;
}
static bool fromhex32(const std::string& s, uint8_t out[32]) {
    if (s.size() != 64) return false;
    for (int i = 0; i < 32; ++i) {
        unsigned v;
        if (sscanf(s.c_str() + 2 * i, "%2x", &v) != 1) return false;
        out[i] = (uint8_t)v;
    }
    return true;
}
static bool recv_line(int fd, std::string& out, size_t max) {
    out.clear();
    char c;
    while (out.size() <= max) {
        ssize_t r = recv(fd, &c, 1, 0);
        if (r <= 0) return false;
        if (c == '\n') return true;
        out.push_back(c);
    }
    return false;
}
static bool send_all(int fd, const std::string& s) {
    size_t off = 0;
    while (off < s.size()) {
        ssize_t r = send(fd, s.data() + off, s.size() - off, 0);
        if (r <= 0) return false;
        off += (size_t)r;
    }
    return true;
}
static double ms_since(std::chrono::high_resolution_clock::time_point t) {
    return std::chrono::duration<double, std::milli>(std::chrono::high_resolution_clock::now() - t).count();
}

struct Tree {
    std::vector<std::vector<std::array<uint8_t, 32>>> levels;
};

static void serve(int fd, cublasHandle_t h, int cheat_rows, bool sequential) {
    std::string line;
    if (!recv_line(fd, line, 256)) return;
    char seedhex[80];
    int n = 0, m = 0;
    if (sscanf(line.c_str(), "CHAL %70s %d %d", seedhex, &n, &m) != 3) return;
    uint8_t seed[32];
    if (!fromhex32(seedhex, seed) || n <= 0 || n % 32 || n > 16384 || m <= 0 || m > 64) return;
    const size_t nn = (size_t)n * n;
    if ((size_t)m * nn * 4 > (size_t)12 << 30) return;  // keep C for all products under 12 GiB
    const int rows_done = std::max(0, n - std::max(0, std::min(cheat_rows, n)));
    auto T0 = std::chrono::high_resolution_clock::now();

    uint8_t *dStream, *dTree;
    int8_t* dBT;
    int32_t* dC;
    const size_t hashes = (2 * nn + 31) / 32;
    size_t width = 1;
    while (width < (size_t)m * n) width <<= 1;
    cudaMalloc(&dStream, hashes * 32);
    cudaMalloc(&dBT, nn);
    // Pipelined mode: two streams with their own expansion/transpose buffers, so the SHA-256 expansion
    // of one product (CUDA cores) overlaps the GEMM of the other (tensor cores).
    uint8_t* dStream2 = nullptr;
    int8_t* dBT2 = nullptr;
    cudaStream_t streams[2] = {0, 0};
    if (!sequential) {
        cudaMalloc(&dStream2, hashes * 32);
        cudaMalloc(&dBT2, nn);
        cudaStreamCreate(&streams[0]);
        cudaStreamCreate(&streams[1]);
    }
    cudaMalloc(&dC, (size_t)m * nn * 4);
    cudaMalloc(&dTree, 2 * width * 32);  // level 0 at offset 0 (width leaves), then each next level
    const int32_t alpha = 1, beta = 0;
    double t_expand = 0, t_gemm = 0, t_hash = 0;
    float ms = 0;
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0); cudaEventCreate(&e1);
    auto prefix_of = [&](int j) {
        std::vector<uint8_t> msg((const uint8_t*)"abacus/attest-seed", (const uint8_t*)"abacus/attest-seed" + 18);
        msg.insert(msg.end(), seed, seed + 32);
        for (int b = 0; b < 4; ++b) msg.push_back((uint8_t)(j >> (8 * b)));
        uint8_t ps[32];
        hsha(msg, ps);
        uint8_t pre[44];
        memcpy(pre, "abacus/expand", 13);
        memcpy(pre + 13, ps, 31);
        ExpandPrefix px;
        for (int q = 0; q < 11; ++q)
            px.pw[q] = ((uint32_t)pre[4 * q] << 24) | ((uint32_t)pre[4 * q + 1] << 16) | ((uint32_t)pre[4 * q + 2] << 8) |
                       pre[4 * q + 3];
        px.b44 = ps[31];
        return px;
    };
    if (!sequential) {
        for (int j = 0; j < m; ++j) {
            const int sidx = j & 1;
            cudaStream_t st = streams[sidx];
            uint8_t* buf = sidx ? dStream2 : dStream;
            int8_t* bt = sidx ? dBT2 : dBT;
            const ExpandPrefix px = prefix_of(j);
            expand_kernel<<<(unsigned)((hashes + 255) / 256), 256, 0, st>>>(px, hashes, buf);
            transpose_i8<<<dim3(n / 32, n / 32), dim3(32, 8), 0, st>>>((const int8_t*)buf + nn, bt, n);
            int32_t* Cj = dC + (size_t)j * nn;
            if (rows_done < n) cudaMemsetAsync(Cj + (size_t)rows_done * n, 0, (size_t)(n - rows_done) * n * 4, st);
            if (rows_done > 0) {
                cublasSetStream(h, st);
                cublasStatus_t stt = cublasGemmEx(h, CUBLAS_OP_T, CUBLAS_OP_N, n, rows_done, n, &alpha, bt, CUDA_R_8I, n,
                                                  (const int8_t*)buf, CUDA_R_8I, n, &beta, Cj, CUDA_R_32I, n,
                                                  CUBLAS_COMPUTE_32I, CUBLAS_GEMM_DEFAULT);
                if (stt != CUBLAS_STATUS_SUCCESS) fprintf(stderr, "cublasGemmEx failed %d\n", (int)stt);
            }
            leaf_kernel<<<(n + 127) / 128, 128, 0, st>>>(Cj, n, (uint32_t)j, dTree + (size_t)j * n * 32);
        }
        cudaDeviceSynchronize();
        cublasSetStream(h, 0);
        t_expand = t_gemm = t_hash = -1;  // overlapped; per-phase times only in --sequential mode
    } else
    for (int j = 0; j < m; ++j) {
        // product seed = SHA256("abacus/attest-seed" || seed || LE32(j)); expansion prefix words
        std::vector<uint8_t> msg((const uint8_t*)"abacus/attest-seed", (const uint8_t*)"abacus/attest-seed" + 18);
        msg.insert(msg.end(), seed, seed + 32);
        for (int b = 0; b < 4; ++b) msg.push_back((uint8_t)(j >> (8 * b)));
        uint8_t ps[32];
        hsha(msg, ps);
        uint8_t pre[44];
        memcpy(pre, "abacus/expand", 13);
        memcpy(pre + 13, ps, 31);
        ExpandPrefix px;
        for (int q = 0; q < 11; ++q)
            px.pw[q] = ((uint32_t)pre[4 * q] << 24) | ((uint32_t)pre[4 * q + 1] << 16) | ((uint32_t)pre[4 * q + 2] << 8) |
                       pre[4 * q + 3];
        px.b44 = ps[31];
        cudaEventRecord(e0);
        expand_kernel<<<(unsigned)((hashes + 255) / 256), 256>>>(px, hashes, dStream);
        const int8_t* dA = (const int8_t*)dStream;       // A row-major: bytes [0, n^2)
        const int8_t* dB = (const int8_t*)dStream + nn;  // B row-major: bytes [n^2, 2 n^2)
        transpose_i8<<<dim3(n / 32, n / 32), dim3(32, 8)>>>(dB, dBT, n);
        cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms, e0, e1); t_expand += ms;
        int32_t* Cj = dC + (size_t)j * nn;
        cudaEventRecord(e0);
        if (rows_done < n) cudaMemset(Cj + (size_t)rows_done * n, 0, (size_t)(n - rows_done) * n * 4);
        if (rows_done > 0) {
            // D (column-major n x rows_done) = BT^T-op * A  ==  rows [0, rows_done) of C, row-major
            cublasStatus_t st = cublasGemmEx(h, CUBLAS_OP_T, CUBLAS_OP_N, n, rows_done, n, &alpha, dBT, CUDA_R_8I, n,
                                             dA, CUDA_R_8I, n, &beta, Cj, CUDA_R_32I, n, CUBLAS_COMPUTE_32I,
                                             CUBLAS_GEMM_DEFAULT);
            if (st != CUBLAS_STATUS_SUCCESS) fprintf(stderr, "cublasGemmEx failed %d\n", (int)st);
        }
        cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms, e0, e1); t_gemm += ms;
        cudaEventRecord(e0);
        leaf_kernel<<<(n + 127) / 128, 128>>>(Cj, n, (uint32_t)j, dTree + (size_t)j * n * 32);
        cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms, e0, e1); t_hash += ms;
    }
    // Merkle tree on the device: pad with SHA256("abacus/attest-empty"), node = SHA256(0x01 || l || r)
    cudaEventRecord(e0);
    {
        std::vector<uint8_t> e((const uint8_t*)"abacus/attest-empty", (const uint8_t*)"abacus/attest-empty" + 19);
        uint8_t empty[32];
        hsha(e, empty);
        const size_t pad = width - (size_t)m * n;
        if (pad) {
            std::vector<uint8_t> fill(pad * 32);
            for (size_t q = 0; q < pad; ++q) memcpy(fill.data() + 32 * q, empty, 32);
            cudaMemcpy(dTree + (size_t)m * n * 32, fill.data(), pad * 32, cudaMemcpyHostToDevice);
        }
    }
    size_t off = 0;
    for (size_t cnt = width; cnt > 1; cnt >>= 1) {
        node_kernel<<<(unsigned)((cnt / 2 + 255) / 256), 256>>>(dTree + off * 32, cnt / 2, dTree + (off + cnt) * 32);
        off += cnt;
    }
    std::vector<uint8_t> htree(2 * width * 32);
    cudaMemcpy(htree.data(), dTree, (2 * width - 1) * 32, cudaMemcpyDeviceToHost);
    cudaEventRecord(e1); cudaEventSynchronize(e1); cudaEventElapsedTime(&ms, e0, e1);
    const double t_merkle = ms, t_total = ms_since(T0);
    Tree tree;
    off = 0;
    for (size_t cnt = width; cnt >= 1; cnt >>= 1) {
        std::vector<std::array<uint8_t, 32>> level(cnt);
        for (size_t q = 0; q < cnt; ++q) memcpy(level[q].data(), htree.data() + (off + q) * 32, 32);
        tree.levels.push_back(std::move(level));
        off += cnt;
        if (cnt == 1) break;
    }
    send_all(fd, "ROOT " + tohex(tree.levels.back()[0].data(), 32) + "\n");
    fprintf(stderr,
            "{\"mode\": \"%s\", \"n\": %d, \"m\": %d, \"rows_done\": %d, \"expand_ms\": %.3f, \"gemm_ms\": %.3f, \"leaf_hash_ms\": %.3f, "
            "\"merkle_ms\": %.3f, \"total_ms\": %.3f, \"gemm_TMAC_s\": %.2f}\n",
            sequential ? "sequential" : "pipelined", n, m, rows_done, t_expand, t_gemm, t_hash, t_merkle, t_total,
            t_gemm > 0 ? (double)m * rows_done * nn / (t_gemm / 1e3) / 1e12 : 0.0);

    if (recv_line(fd, line, 1 << 20) && line.rfind("OPEN", 0) == 0) {
        std::vector<int32_t> row(n);
        size_t pos = 5;
        while (pos < line.size()) {
            size_t sp = line.find(' ', pos);
            std::string item = line.substr(pos, sp == std::string::npos ? std::string::npos : sp - pos);
            pos = sp == std::string::npos ? line.size() : sp + 1;
            int j, i;
            if (sscanf(item.c_str(), "%d:%d", &j, &i) != 2 || j < 0 || j >= m || i < 0 || i >= n) continue;
            cudaMemcpy(row.data(), dC + (size_t)j * nn + (size_t)i * n, (size_t)n * 4, cudaMemcpyDeviceToHost);
            std::string path;
            size_t idx = (size_t)j * n + i;
            for (size_t l = 0; l + 1 < tree.levels.size(); ++l) {
                path += tohex(tree.levels[l][idx ^ 1].data(), 32);
                idx >>= 1;
            }
            send_all(fd, "ROW " + std::to_string(j) + " " + std::to_string(i) + " " +
                             tohex((const uint8_t*)row.data(), (size_t)n * 4) + " " + path + "\n");
        }
        send_all(fd, "END\n");
    }
    cudaFree(dStream); cudaFree(dBT); cudaFree(dC); cudaFree(dTree);
    if (!sequential) {
        cudaFree(dStream2); cudaFree(dBT2);
        cudaStreamDestroy(streams[0]); cudaStreamDestroy(streams[1]);
    }
    cudaEventDestroy(e0); cudaEventDestroy(e1);
}

int main(int argc, char** argv) {
    int port = 9600, cheat = 0;
    bool sequential = true;
    for (int i = 1; i < argc; ++i)
        if (!strcmp(argv[i], "--pipelined")) sequential = false;
    for (int i = 1; i + 1 < argc; ++i) {
        if (!strcmp(argv[i], "--listen")) port = atoi(argv[i + 1]);
        if (!strcmp(argv[i], "--cheat-rows")) cheat = atoi(argv[i + 1]);
    }
    signal(SIGPIPE, SIG_IGN);
    cublasHandle_t h;
    if (cublasCreate(&h) != CUBLAS_STATUS_SUCCESS) { fprintf(stderr, "cublasCreate failed\n"); return 1; }
    int ls = socket(AF_INET, SOCK_STREAM, 0), one = 1;
    setsockopt(ls, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_ANY);
    addr.sin_port = htons(port);
    if (bind(ls, (sockaddr*)&addr, sizeof(addr)) || listen(ls, 4)) { perror("bind/listen"); return 1; }
    fprintf(stderr, "attest_prover listening on %d (cheat_rows=%d)\n", port, cheat);
    for (;;) {
        int fd = accept(ls, nullptr, nullptr);
        if (fd < 0) continue;
        setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof(one));
        serve(fd, h, cheat, sequential);
        close(fd);
    }
}
