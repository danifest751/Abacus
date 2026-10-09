// Gathered-read bandwidth bench (candidate A' premise): random 8 KiB segment reads vs sequential.
//
// If gathered reads reach high bandwidth on the GPU, a memory-hard layer (header-random gather of
// operands) can shift the bottleneck from multiply throughput to memory bandwidth, which is what
// makes an ASIC need expensive high-bandwidth memory. Not a miner.
//
// Build: nvcc -O3 -arch=sm_75 gather_bench.cu -o gather_bench
// Run:   ./gather_bench [dataset_MiB] [seg_kib] [iters]

#include <cstdio>
#include <cstdlib>
#include <cuda_runtime.h>
#include <chrono>

// Each block reads `iters` random 8 KiB segments, coalesced within the block.
__global__ void gather_random(const unsigned* __restrict__ data, unsigned long long* __restrict__ sink,
                              unsigned long long nseg, unsigned seg_words, unsigned iters, unsigned long long base_seed) {
    unsigned long long acc = 0;
    const unsigned tid = threadIdx.x;
    const unsigned bd = blockDim.x;
    unsigned long long base = base_seed + (unsigned long long)blockIdx.x * 0x9E3779B97F4A7C15ull;
    for (unsigned it = 0; it < iters; ++it) {
        unsigned long long s = (base + (unsigned long long)it * 0xBF58476D1CE4E5B9ull) % nseg;
        const unsigned* p = data + s * (unsigned long long)seg_words;
        for (unsigned i = tid; i < seg_words; i += bd) acc += p[i];
    }
    sink[blockIdx.x * bd + tid] = acc;
}

// Each block reads a contiguous region (streaming reference).
__global__ void seq_read(const unsigned* __restrict__ data, unsigned long long* __restrict__ sink,
                         unsigned long long words_per_block) {
    unsigned long long acc = 0;
    const unsigned tid = threadIdx.x;
    const unsigned bd = blockDim.x;
    unsigned long long off = (unsigned long long)blockIdx.x * words_per_block;
    for (unsigned long long i = tid; i < words_per_block; i += bd) acc += data[off + i];
    sink[blockIdx.x * bd + tid] = acc;
}

int main(int argc, char** argv) {
    int mib = argc > 1 ? atoi(argv[1]) : 256;
    int seg_kib = argc > 2 ? atoi(argv[2]) : 8;
    int iters = argc > 3 ? atoi(argv[3]) : 256;

    unsigned long long bytes_total = (unsigned long long)mib * 1024 * 1024;
    unsigned long long words = bytes_total / 4;
    unsigned seg_words = (unsigned long long)seg_kib * 1024 / 4;
    unsigned long long nseg = words / seg_words;

    unsigned* d;
    cudaMalloc(&d, bytes_total);
    cudaMemset(d, 0xA5, bytes_total);

    int block = 256;
    int grid = 2048;
    unsigned long long* sink;
    cudaMalloc(&sink, (size_t)grid * block * 8);

    // warmup
    gather_random<<<grid, block>>>(d, sink, nseg, seg_words, 8, 12345);
    cudaDeviceSynchronize();

    auto t0 = std::chrono::high_resolution_clock::now();
    gather_random<<<grid, block>>>(d, sink, nseg, seg_words, iters, 99991);
    cudaDeviceSynchronize();
    auto t1 = std::chrono::high_resolution_clock::now();
    double g_s = std::chrono::duration<double>(t1 - t0).count();
    double g_bytes = (double)grid * iters * seg_words * 4;

    unsigned long long wpg = words / grid;
    seq_read<<<grid, block>>>(d, sink, wpg);
    cudaDeviceSynchronize();
    auto t2 = std::chrono::high_resolution_clock::now();
    seq_read<<<grid, block>>>(d, sink, wpg);
    cudaDeviceSynchronize();
    auto t3 = std::chrono::high_resolution_clock::now();
    double s_s = std::chrono::duration<double>(t3 - t2).count();
    double s_bytes = (double)grid * wpg * 4;

    printf("{\n");
    printf("  \"dataset_MiB\": %d, \"seg_kib\": %d, \"iters\": %d,\n", mib, seg_kib, iters);
    printf("  \"gather_GB_s\": %.1f,\n", g_bytes / g_s / 1e9);
    printf("  \"sequential_GB_s\": %.1f,\n", s_bytes / s_s / 1e9);
    printf("  \"gather_over_seq\": %.3f,\n", (g_bytes / g_s) / (s_bytes / s_s));
    printf("  \"cuda_error\": \"%s\"\n}\n", cudaGetErrorString(cudaGetLastError()));
    return 0;
}
