// NTT throughput bench for the A8 proof-cost model (ADR 0012, test 1): how fast can a GPU encode the
// product C into a polynomial commitment, compared with the int8 GEMM that produced it?
//
// Radix-2 decimation-in-frequency NTT of length L = 2^logL (output in bit-reversed order, which is
// what a Merkle-committed codeword needs), over two fields:
//   - Goldilocks  P = 2^64 - 2^32 + 1 (64-bit), generator 7;
//   - BabyBear    p = 2^31 - 2^27 + 1 (31-bit, Montgomery form), generator 31.
// Two schedules: "naive" = one kernel launch per stage (logL passes over global memory) and "fused" =
// stages with stride >= 1024 per launch, the last 10 stages fused in shared memory (one more pass).
// Correctness: the fused GPU output for L = 1024 is compared with an O(L^2) CPU DFT (bit-reversed).
// Timing: warm-up `warm_s` seconds, then the median of `reps` CUDA-event timings.
//
// Build: nvcc -O3 -arch=sm_75 ntt_bench.cu -o ntt_bench
// Run:   ./ntt_bench [logL] [reps] [warm_s]

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>
#include <cuda_runtime.h>

// ---------------- Goldilocks ----------------
#define GLP 0xFFFFFFFF00000001ULL
#define EPS 0xFFFFFFFFULL
struct GL {
    using T = uint64_t;
    static __host__ __device__ __forceinline__ T add(T a, T b) {
        T s = a + b;
        if (s < a) s += EPS;
        if (s >= GLP) s -= GLP;
        return s;
    }
    static __host__ __device__ __forceinline__ T sub(T a, T b) { return a >= b ? a - b : a + (GLP - b); }
    static __device__ __forceinline__ T mul(T a, T b) {
        T lo = a * b, hi = __umul64hi(a, b);
        T hh = hi >> 32, hl = hi & 0xFFFFFFFFULL;
        T t0 = lo - hh;
        if (lo < hh) t0 -= EPS;
        T t1 = hl * EPS;
        T r = t0 + t1;
        if (r < t0) r += EPS;
        return r >= GLP ? r - GLP : r;
    }
    static T hmul(T a, T b) { return (T)((unsigned __int128)a * b % GLP); }
    static T hpow(T a, uint64_t e) {
        T r = 1;
        while (e) { if (e & 1) r = hmul(r, a); a = hmul(a, a); e >>= 1; }
        return r;
    }
    static T root(int logL) { return hpow(7, (GLP - 1) >> logL); }
    static T to_mont(T x) { return x % GLP; }   // Goldilocks is used in plain form
    static T from_mont(T x) { return x; }
    static const char* name() { return "goldilocks"; }
    static const int bytes = 8;
};

// ---------------- BabyBear (Montgomery, R = 2^32) ----------------
#define BBP 0x78000001u
#define BBPINV 0x77ffffffu  // -p^-1 mod 2^32
struct BB {
    using T = uint32_t;
    static __host__ __device__ __forceinline__ T add(T a, T b) { T s = a + b; return s >= BBP ? s - BBP : s; }
    static __host__ __device__ __forceinline__ T sub(T a, T b) { return a >= b ? a - b : a + BBP - b; }
    static __host__ __device__ __forceinline__ T mul(T a, T b) {  // Montgomery: a b R^-1 mod p
        uint64_t t = (uint64_t)a * b;
        uint32_t m = (uint32_t)t * BBPINV;
        uint64_t u = (t + (uint64_t)m * BBP) >> 32;
        return (T)(u >= BBP ? u - BBP : u);
    }
    static T hmul_plain(T a, T b) { return (T)((uint64_t)a * b % BBP); }
    static T hpow_plain(T a, uint64_t e) {
        T r = 1;
        while (e) { if (e & 1) r = hmul_plain(r, a); a = hmul_plain(a, a); e >>= 1; }
        return r;
    }
    static T to_mont(T x) { return (T)(((uint64_t)x << 32) % BBP); }
    static T from_mont(T x) { return mul(x, 1); }
    static T root(int logL) { return to_mont(hpow_plain(31, (uint64_t)(BBP - 1) >> logL)); }
    static T hmul(T a, T b) { return mul(a, b); }  // Montgomery on host
    static const char* name() { return "babybear"; }
    static const int bytes = 4;
};

// DIF butterfly stage with half-length h: for blocks of 2h, x[j], x[j+h] -> (x+y, (x-y) w^(j * L/2h)).
template <class F>
__global__ void stage(typename F::T* x, const typename F::T* tw, int logL, int logh) {
    const size_t t = (size_t)blockIdx.x * blockDim.x + threadIdx.x;
    const size_t half = (size_t)1 << (logL - 1);
    if (t >= half) return;
    const size_t h = (size_t)1 << logh;
    const size_t blk = t >> logh, j = t & (h - 1);
    const size_t i0 = (blk << (logh + 1)) + j, i1 = i0 + h;
    const typename F::T a = x[i0], b = x[i1];
    x[i0] = F::add(a, b);
    x[i1] = F::mul(F::sub(a, b), tw[j << (logL - 1 - logh)]);
}

// Last 10 stages (h = 512 .. 1) on a contiguous 1024-element tile in shared memory.
template <class F>
__global__ void fused_tail(typename F::T* x, const typename F::T* tw, int logL) {
    __shared__ typename F::T s[1024];
    const size_t base = (size_t)blockIdx.x * 1024;
    for (int i = threadIdx.x; i < 1024; i += blockDim.x) s[i] = x[base + i];
    __syncthreads();
    for (int logh = 9; logh >= 0; --logh) {
        const int h = 1 << logh;
        for (int t = threadIdx.x; t < 512; t += blockDim.x) {
            const int blk = t >> logh, j = t & (h - 1);
            const int i0 = (blk << (logh + 1)) + j, i1 = i0 + h;
            const typename F::T a = s[i0], b = s[i1];
            s[i0] = F::add(a, b);
            s[i1] = F::mul(F::sub(a, b), tw[(size_t)j << (logL - 1 - logh)]);
        }
        __syncthreads();
    }
    for (int i = threadIdx.x; i < 1024; i += blockDim.x) x[base + i] = s[i];
}

template <class F>
static void run_ntt(typename F::T* d, const typename F::T* dtw, int logL, bool fused) {
    const size_t half = (size_t)1 << (logL - 1);
    const unsigned blocks = (unsigned)((half + 255) / 256);
    const int stop = fused && logL >= 10 ? 10 : 0;
    for (int logh = logL - 1; logh >= stop; --logh) stage<F><<<blocks, 256>>>(d, dtw, logL, logh);
    if (stop) fused_tail<F><<<(unsigned)(((size_t)1 << logL) / 1024), 256>>>(d, dtw, logL);
}

static size_t bitrev(size_t x, int bits) {
    size_t r = 0;
    for (int i = 0; i < bits; ++i) r |= ((x >> i) & 1) << (bits - 1 - i);
    return r;
}

template <class F>
static bool check_small() {  // L = 1024, fused path vs CPU DFT (bit-reversed output)
    using T = typename F::T;
    const int logL = 10;
    const size_t L = 1024;
    std::vector<T> x(L), tw(L / 2), out(L);
    for (size_t i = 0; i < L; ++i) x[i] = F::to_mont((T)(i * 2654435761u + 7));
    const T w = F::root(logL);
    T acc = F::to_mont(1);
    for (size_t i = 0; i < L / 2; ++i) { tw[i] = acc; acc = F::hmul(acc, w); }
    T *dx, *dtw;
    cudaMalloc(&dx, L * sizeof(T)); cudaMalloc(&dtw, L / 2 * sizeof(T));
    cudaMemcpy(dx, x.data(), L * sizeof(T), cudaMemcpyHostToDevice);
    cudaMemcpy(dtw, tw.data(), L / 2 * sizeof(T), cudaMemcpyHostToDevice);
    run_ntt<F>(dx, dtw, logL, true);
    cudaMemcpy(out.data(), dx, L * sizeof(T), cudaMemcpyDeviceToHost);
    cudaFree(dx); cudaFree(dtw);
    // y_k = sum_i x_i w^(ik); DIF output index bitrev(k)
    std::vector<T> wp(L);
    T a = F::to_mont(1);
    for (size_t i = 0; i < L; ++i) { wp[i] = a; a = F::hmul(a, w); }
    for (size_t k = 0; k < L; ++k) {
        T y = F::to_mont(0);
        for (size_t i = 0; i < L; ++i) y = F::add(y, F::hmul(x[i], wp[(i * k) % L]));
        if (F::from_mont(y) != F::from_mont(out[bitrev(k, logL)])) return false;
    }
    return true;
}

template <class F>
static void bench(int logL, int reps, double warm_s) {
    using T = typename F::T;
    const size_t L = (size_t)1 << logL;
    std::vector<T> tw(L / 2);
    const T w = F::root(logL);
    T acc = F::to_mont(1);
    for (size_t i = 0; i < L / 2; ++i) { tw[i] = acc; acc = F::hmul(acc, w); }
    T *d, *dtw;
    if (cudaMalloc(&d, L * sizeof(T)) != cudaSuccess || cudaMalloc(&dtw, L / 2 * sizeof(T)) != cudaSuccess) {
        printf("{\"field\": \"%s\", \"logL\": %d, \"error\": \"cudaMalloc\"}\n", F::name(), logL);
        return;
    }
    cudaMemset(d, 0x11, L * sizeof(T));
    cudaMemcpy(dtw, tw.data(), L / 2 * sizeof(T), cudaMemcpyHostToDevice);
    auto w0 = std::chrono::high_resolution_clock::now();
    do {
        run_ntt<F>(d, dtw, logL, true);
        cudaDeviceSynchronize();
    } while (std::chrono::duration<double>(std::chrono::high_resolution_clock::now() - w0).count() < warm_s);
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0); cudaEventCreate(&e1);
    std::vector<double> tn, tf;
    for (int r = 0; r < reps; ++r) {
        float ms = 0;
        cudaEventRecord(e0); run_ntt<F>(d, dtw, logL, false); cudaEventRecord(e1); cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1); tn.push_back(ms / 1e3);
        cudaEventRecord(e0); run_ntt<F>(d, dtw, logL, true); cudaEventRecord(e1); cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1); tf.push_back(ms / 1e3);
    }
    std::sort(tn.begin(), tn.end()); std::sort(tf.begin(), tf.end());
    const double n_s = tn[tn.size() / 2], f_s = tf[tf.size() / 2];
    printf("{\"field\": \"%s\", \"logL\": %d, \"reps\": %d, \"naive_s\": %.6f, \"fused_s\": %.6f, \"fused_s_min\": %.6f, "
           "\"fused_s_max\": %.6f, \"fused_Melem_s\": %.1f, \"fused_GB_s_per_pass\": %.1f, \"cuda_error\": \"%s\"}\n",
           F::name(), logL, reps, n_s, f_s, tf.front(), tf.back(), L / f_s / 1e6,
           (double)L * F::bytes * 2 * (logL - 10 + 1) / f_s / 1e9, cudaGetErrorString(cudaGetLastError()));
    cudaFree(d); cudaFree(dtw);
}

int main(int argc, char** argv) {
    const int logL = argc > 1 ? atoi(argv[1]) : 24;
    const int reps = argc > 2 ? atoi(argv[2]) : 7;
    const double warm_s = argc > 3 ? atof(argv[3]) : 3.0;
    if (logL < 10 || logL > 27) { fprintf(stderr, "logL must be in [10, 27]\n"); return 1; }
    printf("{\"check_goldilocks_L1024\": %s, \"check_babybear_L1024\": %s}\n", check_small<GL>() ? "true" : "false",
           check_small<BB>() ? "true" : "false");
    bench<GL>(logL, reps, warm_s);
    bench<BB>(logL, reps, warm_s);
    return 0;
}
