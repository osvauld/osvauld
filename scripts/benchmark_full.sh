#!/usr/bin/env bash
set -euo pipefail

MESSAGE_COUNT="${1:-5000}"
BASELINE_DIR="${2:-}"
SET_BASELINE="false"

if (( MESSAGE_COUNT > 1000 )); then
  HEAPTRACK_MESSAGE_COUNT="${HEAPTRACK_MESSAGE_COUNT:-1000}"
else
  HEAPTRACK_MESSAGE_COUNT="${HEAPTRACK_MESSAGE_COUNT:-${MESSAGE_COUNT}}"
fi

if (( MESSAGE_COUNT >= 1000 )); then
  CHAT_DELAY="${CHAT_DELAY:-0.0}"
else
  CHAT_DELAY="${CHAT_DELAY:-0.1}"
fi

ENABLE_RESTART_MEMORY="${ENABLE_RESTART_MEMORY:-auto}"
if [[ "${ENABLE_RESTART_MEMORY}" == "auto" ]]; then
  if (( MESSAGE_COUNT >= 5000 )); then
    ENABLE_RESTART_MEMORY="true"
  else
    ENABLE_RESTART_MEMORY="false"
  fi
fi

for arg in "${@:3}"; do
  case "$arg" in
    --set-baseline)
      SET_BASELINE="true"
      ;;
    *)
      echo "Unknown option: $arg"
      echo "Usage: scripts/benchmark_full.sh [messages] [baseline_dir] [--set-baseline]"
      echo "Env: HEAPTRACK_MESSAGE_COUNT=1000 ENABLE_RESTART_MEMORY=auto|true|false"
      exit 1
      ;;
  esac
done

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
ROOT_DIR="$(git rev-parse --show-toplevel)"
RUN_DIR="${ROOT_DIR}/results/benchmark_${TIMESTAMP}"
LOG_DIR="${RUN_DIR}/logs"
BASELINE_TARGET="${ROOT_DIR}/results/baseline_cff2502a"

mkdir -p "${RUN_DIR}" "${LOG_DIR}"

TOTAL_STEPS=7
if [[ "${ENABLE_RESTART_MEMORY}" == "true" ]]; then
  TOTAL_STEPS=8
fi

step_label() {
  local idx="$1"
  local pct=$(( idx * 100 / TOTAL_STEPS ))
  printf "[%d/%d | %d%%]" "$idx" "$TOTAL_STEPS" "$pct"
}

echo "== Osvauld benchmark run =="
echo "run_dir: ${RUN_DIR}"
echo "messages: ${MESSAGE_COUNT}"
echo "heaptrack_messages: ${HEAPTRACK_MESSAGE_COUNT}"
echo "chat_delay: ${CHAT_DELAY}s"
echo "restart_memory: ${ENABLE_RESTART_MEMORY}"
echo "baseline_dir: ${BASELINE_DIR:-none}"
echo "target_profile: 5K default workload"

git rev-parse --short HEAD > "${RUN_DIR}/git_commit.txt"
git rev-parse --abbrev-ref HEAD > "${RUN_DIR}/git_branch.txt"

echo ""
echo "================================================================"
echo "  BENCHMARK PIPELINE - ${TOTAL_STEPS} STEPS"
echo "================================================================"
echo ""

echo "$(step_label 1) Build release binaries (2-3 min)"
cargo build --release -p kunki -p sthalam 2>&1 | tee "${LOG_DIR}/build_release.log"
echo "  ✓ Release binaries ready"

echo ""
echo "$(step_label 2) Run release performance test (${MESSAGE_COUNT} messages, delay=${CHAT_DELAY}s)"
python -u e2e_tests/test_chat_perf.py --release -m "${MESSAGE_COUNT}" --delay "${CHAT_DELAY}" -o "${RUN_DIR}/perf_release.json" \
  2>&1 | tee "${LOG_DIR}/test_release.log"
echo "  ✓ Performance data captured: ${RUN_DIR}/perf_release.json"

echo ""
echo "$(step_label 3) Run heaptrack capture (${HEAPTRACK_MESSAGE_COUNT} messages, delay=${CHAT_DELAY}s)"
python -u e2e_tests/test_chat_perf.py --release --heaptrack -m "${HEAPTRACK_MESSAGE_COUNT}" --delay "${CHAT_DELAY}" \
  2>&1 | tee "${LOG_DIR}/test_heaptrack.log"
echo "  ✓ Heaptrack traces captured: /tmp/chat_perf/*.heaptrack.zst"

echo ""
echo "$(step_label 4) Analyze heaptrack output (4 files × ~29 invocations each, ~5-10 min)"
echo "  Note: This step processes 116 heaptrack_print calls - please be patient"
python -u scripts/analyze_heaptrack.py /tmp/chat_perf/ -o "${RUN_DIR}/heap_analysis.json" \
  2>&1 | tee "${LOG_DIR}/analyze_heaptrack.log"
echo "  ✓ Heap analysis complete: ${RUN_DIR}/heap_analysis.json"

echo ""
echo "$(step_label 5) Build profiling binaries (2-3 min)"
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile profiling --features profiling -p kunki -p sthalam \
  2>&1 | tee "${LOG_DIR}/build_profiling.log"
echo "  ✓ Profiling binaries ready"

echo ""
echo "$(step_label 6) Run flamegraph capture (${MESSAGE_COUNT} messages, delay=${CHAT_DELAY}s)"
python -u e2e_tests/test_chat_perf.py --flame-only -m "${MESSAGE_COUNT}" --delay "${CHAT_DELAY}" \
  2>&1 | tee "${LOG_DIR}/test_flame.log"
echo "  ✓ Flamegraph data captured: /tmp/chat_perf/*.svg"

echo ""
echo "$(step_label 7) Analyze flamegraph output (~1 min)"
python -u scripts/analyze_flamegraph.py /tmp/chat_perf/ -o "${RUN_DIR}/flame_analysis.json" \
  2>&1 | tee "${LOG_DIR}/analyze_flame.log"
echo "  ✓ Flamegraph analysis complete: ${RUN_DIR}/flame_analysis.json"

echo ""
echo "Ractor CPU summary (from flame analysis):"
python - "${RUN_DIR}/flame_analysis.json" <<'PY'
import json
import pathlib
import statistics
import sys

path = pathlib.Path(sys.argv[1])
if not path.exists():
    print("  (flame analysis missing; skipping)")
    raise SystemExit(0)

data = json.loads(path.read_text())
instances = data.get("instances", [])
if not instances:
    print("  (no flame instances found)")
    raise SystemExit(0)

rows = []
for inst in instances:
    file_path = pathlib.Path(inst.get("file", "unknown"))
    name = file_path.parent.name
    ractor = inst.get("categories", {}).get("ractor", {}).get("percent")
    if ractor is None:
        continue
    rows.append((name, float(ractor)))

if not rows:
    print("  (no ractor category data found)")
    raise SystemExit(0)

for name, pct in rows:
    print(f"  - {name:<8} ractor={pct:5.1f}%")

avg = statistics.mean(pct for _, pct in rows)
mx_name, mx_pct = max(rows, key=lambda x: x[1])
print(f"  - average ractor share={avg:.1f}%")
print(f"  - highest ractor share={mx_pct:.1f}% ({mx_name})")
PY

if [[ "${ENABLE_RESTART_MEMORY}" == "true" ]]; then
  echo ""
  echo "$(step_label 8) Run restart memory capture (${MESSAGE_COUNT} messages, ~3-8 min)"
  python -u e2e_tests/test_chat_restart_memory.py --release -m "${MESSAGE_COUNT}" -o "${RUN_DIR}/restart_memory.json" \
    2>&1 | tee "${LOG_DIR}/test_restart_memory.log"
  echo "  ✓ Restart memory data captured: ${RUN_DIR}/restart_memory.json"
fi

echo ""
echo "================================================================"
echo "  CONSOLIDATING RESULTS"
echo "================================================================"
echo ""
if [[ -n "${BASELINE_DIR}" ]]; then
  python -u scripts/generate_report.py --run-dir "${RUN_DIR}" --messages "${MESSAGE_COUNT}" --baseline-dir "${BASELINE_DIR}" \
    2>&1 | tee "${LOG_DIR}/generate_report.log"
else
  python -u scripts/generate_report.py --run-dir "${RUN_DIR}" --messages "${MESSAGE_COUNT}" \
    2>&1 | tee "${LOG_DIR}/generate_report.log"
fi
echo "  ✓ Consolidated report: ${RUN_DIR}/report.json"

if [[ "${SET_BASELINE}" == "true" ]]; then
  echo ""
  echo "Setting this run as new baseline: ${BASELINE_TARGET}"
  rm -rf "${BASELINE_TARGET}"
  mkdir -p "${BASELINE_TARGET}"
  cp "${RUN_DIR}/perf_release.json" "${BASELINE_TARGET}/perf_release.json"
  cp "${RUN_DIR}/heap_analysis.json" "${BASELINE_TARGET}/heap_analysis.json"
  cp "${RUN_DIR}/flame_analysis.json" "${BASELINE_TARGET}/flame_analysis.json"
  cp "${RUN_DIR}/report.json" "${BASELINE_TARGET}/report.json"
  cp "${RUN_DIR}/git_commit.txt" "${BASELINE_TARGET}/git_commit.txt"
  cp "${RUN_DIR}/git_branch.txt" "${BASELINE_TARGET}/git_branch.txt"
  echo "  ✓ Baseline updated"
fi

echo ""
echo "================================================================"
echo "  BENCHMARK COMPLETE"
echo "================================================================"
echo ""
echo "Results directory: ${RUN_DIR}"
echo ""
echo "JSON outputs:"
echo "  • Performance:  ${RUN_DIR}/perf_release.json"
echo "  • Heap:         ${RUN_DIR}/heap_analysis.json"
echo "  • Flamegraph:   ${RUN_DIR}/flame_analysis.json"
echo "  • Consolidated: ${RUN_DIR}/report.json"
if [[ -f "${RUN_DIR}/comparison.json" ]]; then
  echo "  • Comparison:   ${RUN_DIR}/comparison.json"
fi
if [[ -f "${RUN_DIR}/restart_memory.json" ]]; then
  echo "  • Restart Mem:  ${RUN_DIR}/restart_memory.json"
fi
echo ""
echo "Logs: ${LOG_DIR}/"
echo ""
