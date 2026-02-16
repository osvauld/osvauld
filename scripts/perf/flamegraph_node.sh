#!/bin/bash
# Generate flamegraph for kunki node under load
#
# Prerequisites:
#   cargo install flamegraph
#   sudo apt install linux-perf  # or: sudo pacman -S perf
#
# Usage:
#   ./scripts/perf/flamegraph_node.sh [duration_seconds]
#
# Examples:
#   ./scripts/perf/flamegraph_node.sh        # Run for 30 seconds
#   ./scripts/perf/flamegraph_node.sh 60     # Run for 60 seconds

set -e

cd "$(dirname "$0")/../.."

DURATION="${1:-30}"
OUTPUT="flamegraph-node-$(date +%Y%m%d-%H%M%S).svg"
DATA_DIR="/tmp/flamegraph-kunki-$$"

echo "Building with profiling profile..."
cargo build --profile profiling --package kunki

echo "Initializing test node..."
mkdir -p "$DATA_DIR"
./target/profiling/kunki --db-path "$DATA_DIR/node" init \
    --username flamegraph_test \
    --passphrase test

echo "Generating flamegraph for kunki node (${DURATION}s)..."
echo "Output: $OUTPUT"

# Run kunki with timeout and generate flamegraph
timeout "${DURATION}s" cargo flamegraph \
    --profile profiling \
    --bin kunki \
    --root \
    -o "$OUTPUT" \
    -- --db-path "$DATA_DIR/node" start --passphrase test || true

# Cleanup
rm -rf "$DATA_DIR"

echo ""
echo "Generated: $OUTPUT"
echo "Open in browser to view the interactive flamegraph"
