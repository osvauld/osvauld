#!/bin/bash
# Generate flamegraph for sync integration tests
#
# Prerequisites:
#   cargo install flamegraph
#   sudo apt install linux-perf  # or: sudo pacman -S perf
#
# Usage:
#   ./scripts/perf/flamegraph_sync.sh [test_name]
#
# Examples:
#   ./scripts/perf/flamegraph_sync.sh              # Run all dst_sync tests
#   ./scripts/perf/flamegraph_sync.sh handshake    # Run specific test

set -e

cd "$(dirname "$0")/../.."

TEST_NAME="${1:-dst_sync}"
OUTPUT="flamegraph-sync-$(date +%Y%m%d-%H%M%S).svg"

echo "Building with profiling profile..."
cargo build --profile profiling --package integration_tests

echo "Generating flamegraph for test: $TEST_NAME"
echo "Output: $OUTPUT"

# Use cargo flamegraph with profiling profile
cargo flamegraph \
    --profile profiling \
    --package integration_tests \
    --test "$TEST_NAME" \
    --root \
    -o "$OUTPUT" \
    -- --test-threads=1 --nocapture

echo ""
echo "Generated: $OUTPUT"
echo "Open in browser to view the interactive flamegraph"
