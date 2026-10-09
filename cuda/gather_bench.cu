// Gathered-read bandwidth bench (candidate A' premise): random segment reads vs a sequential stream.
//
// Random segments of `seg` bytes are read from a `dataset_MiB` buffer. Each segment is read by a
// group of `g = min(32, seg/8)` threads (coalesced 8-byte words, a warp-cooperative read for
// segments >= 256 B), and every group draws a fresh pseudo-random segment per iteration. The
// sequential reference streams the whole buffer. The GPU is kept busy for `warm_s` seconds first
// (the CMP idle governor otherwise under-clocks the first runs ~20x), then each kernel is timed with
// CUDA events `reps` times and the median is reported. Not a miner.
//
// Build: nvcc -O3 -arch=sm_75 gather_bench.cu -o gather_bench
// Run:   ./gather_bench [dataset_MiB] [seg_bytes] [reps] [warm_s]

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <vector>
#include <cuda_runtime.h>

__device__ __forceinline__ unsigned long long mix64(unsigned long long x) {
    x ^= x >> 30; x *= 0xBF58476D1CE4E5B9ULL;
    x ^= x >> 27; x *= 0x94D049BB133111EBULL;
    return x ^ (x >> 31);
}

__global__ void fill(unsigned long long* d, unsigned long long words) {
    for (unsigned long long i = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; i < words;
         i += (unsigned long long)gridDim.x * blockDim.x)
        d[i] = mix64(i);
}

// Groups of g threads; group j reads `iters` random segments of seg_words 8-byte words.
__global__ void gather_random(const unsigned long long* __restrict__ d, unsigned long long* __restrict__ sink,
                              unsigned long long nseg, unsigned seg_words, unsigned g, unsigned iters,
                              unsigned long long seed) {
    const unsigned long long tid = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x;
    const unsigned long long group = tid / g;
    const unsigned lane = (unsigned)(tid % g);
    unsigned long long acc = 0;
    for (unsigned it = 0; it < iters; ++it) {
        const unsigned long long s = mix64(seed ^ (group * 0x9E3779B97F4A7C15ULL + it)) % nseg;
        const unsigned long long* p = d + s * seg_words;
        for (unsigned w = lane; w < seg_words; w += g) acc += p[w];
    }
    sink[tid] = acc;
}

__global__ void seq_read(const unsigned long long* __restrict__ d, unsigned long long* __restrict__ sink,
                         unsigned long long words) {
    unsigned long long acc = 0;
    for (unsigned long long i = (unsigned long long)blockIdx.x * blockDim.x + threadIdx.x; i < words;
         i += (unsigned long long)gridDim.x * blockDim.x)
        acc += d[i];
    sink[(unsigned long long)blockIdx.x * blockDim.x + threadIdx.x] = acc;
}

static double median(std::vector<double> v) {
    std::sort(v.begin(), v.end());
    return v[v.size() / 2];
}

int main(int argc, char** argv) {
    const int mib = argc > 1 ? atoi(argv[1]) : 2048;
    const int seg = argc > 2 ? atoi(argv[2]) : 65536;
    const int reps = argc > 3 ? atoi(argv[3]) : 5;
    const double warm_s = argc > 4 ? atof(argv[4]) : 3.0;
    if (seg < 8 || seg % 8) { fprintf(stderr, "seg_bytes must be a positive multiple of 8\n"); return 1; }

    const unsigned long long words = (unsigned long long)mib * 1024 * 1024 / 8;
    const unsigned seg_words = (unsigned)(seg / 8);
    const unsigned long long nseg = words / seg_words;
    const unsigned g = seg_words < 32 ? seg_words : 32;
    const int block = 256, grid = 4096;
    const unsigned long long threads = (unsigned long long)grid * block;
    const unsigned long long groups = threads / g;
    // Read about 4 GiB per gather launch (at least one segment per group).
    unsigned iters = (unsigned)std::max<unsigned long long>(1, (4ULL << 30) / (groups * (unsigned long long)seg));

    unsigned long long *d, *sink;
    if (cudaMalloc(&d, words * 8) != cudaSuccess || cudaMalloc(&sink, threads * 8) != cudaSuccess) {
        fprintf(stderr, "cudaMalloc failed\n");
        return 1;
    }
    fill<<<1024, 256>>>(d, words);

    auto w0 = std::chrono::high_resolution_clock::now();
    for (unsigned long long r = 0;; ++r) {
        gather_random<<<grid, block>>>(d, sink, nseg, seg_words, g, iters, r);
        seq_read<<<grid, block>>>(d, sink, words);
        cudaDeviceSynchronize();
        if (std::chrono::duration<double>(std::chrono::high_resolution_clock::now() - w0).count() >= warm_s) break;
    }

    cudaEvent_t e0, e1;
    cudaEventCreate(&e0);
    cudaEventCreate(&e1);
    std::vector<double> gs, ss;
    for (int r = 0; r < reps; ++r) {
        float ms = 0;
        cudaEventRecord(e0);
        gather_random<<<grid, block>>>(d, sink, nseg, seg_words, g, iters, 1000 + r);
        cudaEventRecord(e1);
        cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1);
        gs.push_back((double)groups * iters * seg / (ms / 1e3) / 1e9);
        cudaEventRecord(e0);
        seq_read<<<grid, block>>>(d, sink, words);
        cudaEventRecord(e1);
        cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms, e0, e1);
        ss.push_back((double)words * 8 / (ms / 1e3) / 1e9);
    }
    const double g_med = median(gs), s_med = median(ss);
    printf("{\"dataset_MiB\": %d, \"seg_bytes\": %d, \"group_threads\": %u, \"reps\": %d, "
           "\"gather_GB_s\": %.1f, \"gather_GB_s_min\": %.1f, \"gather_GB_s_max\": %.1f, "
           "\"sequential_GB_s\": %.1f, \"gather_over_seq\": %.3f, \"segments_per_s_M\": %.1f, \"cuda_error\": \"%s\"}\n",
           mib, seg, g, reps, g_med, *std::min_element(gs.begin(), gs.end()), *std::max_element(gs.begin(), gs.end()),
           s_med, g_med / s_med, g_med * 1e9 / seg / 1e6, cudaGetErrorString(cudaGetLastError()));
    return 0;
}
