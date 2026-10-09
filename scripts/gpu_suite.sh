#!/usr/bin/env bash
# GPU measurement suite: builds every CUDA bench and records warm, repeated measurements.
#
# Run on a CUDA host from a checkout (or an exported tree) of this repository:
#   bash scripts/gpu_suite.sh [out_dir] [arch]
# Output: <out_dir>/env.txt (GPU, driver, nvcc, source hashes) and one JSON line per run in
# <out_dir>/{matmul,gather,attempt}.jsonl. Defaults: out_dir=artifacts/gpu-<UTC date>, arch=sm_75.
# Every bench warms the GPU first and reports the median of repeated CUDA-event timings.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/artifacts/gpu-$(date -u +%Y%m%dT%H%M%SZ)}"
ARCH="${2:-sm_75}"
NVCC="${NVCC:-$(command -v nvcc || echo /usr/local/cuda/bin/nvcc)}"
BIN="$OUT/bin"
mkdir -p "$BIN"

{
  date -u +%FT%TZ
  nvidia-smi --query-gpu=name,driver_version,memory.total,clocks.max.sm,clocks.max.mem --format=csv,noheader
  "$NVCC" --version | tail -1
  (cd "$ROOT" && sha256sum cuda/*.cu scripts/gpu_suite.sh)
  (cd "$ROOT" && git rev-parse HEAD 2>/dev/null || echo "no git")
} > "$OUT/env.txt"

for b in goldilocks_matmul_bench gather_bench attempt_bench; do
  "$NVCC" -O3 -arch="$ARCH" "$ROOT/cuda/$b.cu" -o "$BIN/$b"
done

clocks() { nvidia-smi --query-gpu=clocks.sm,clocks.mem,power.draw,temperature.gpu --format=csv,noheader; }

: > "$OUT/matmul.jsonl"
for n in 256 512 1024 2048; do
  "$BIN/goldilocks_matmul_bench" "$n" 7 3 | tr -d '\n' >> "$OUT/matmul.jsonl"
  echo " # $(clocks)" >> "$OUT/matmul.jsonl"
done

: > "$OUT/gather.jsonl"
for seg in 8 32 256 2560 8192 65536; do
  "$BIN/gather_bench" 2048 "$seg" 7 3 | tr -d '\n' >> "$OUT/gather.jsonl"
  echo " # $(clocks)" >> "$OUT/gather.jsonl"
done

: > "$OUT/attempt.jsonl"
for cfg in "256 2560 2048 64" "256 65536 2048 16" "512 2560 2048 32"; do
  for fold in 0 1 2 3; do
    # shellcheck disable=SC2086
    "$BIN/attempt_bench" $cfg "$fold" 3 7 | tr -d '\n' >> "$OUT/attempt.jsonl"
    echo " # $(clocks)" >> "$OUT/attempt.jsonl"
  done
done

echo "$OUT"
