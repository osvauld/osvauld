#!/bin/bash
# AI Interface Session Setup
#
# This script sets up the ai_interface for Claude to interact with.
# It creates named pipes for stdin/stdout and starts the ai_interface in the background.
#
# Usage:
#   ./scripts/setup_ai_session.sh [--instances owner,customer] [--node] [--db-dir /tmp/test]
#
# Claude can then:
#   - Send commands: echo '{"target": "owner", "action": "ui_get_screen", "id": 1}' > /tmp/ai_cmd
#   - Read responses: tail -1 /tmp/ai_out.log

set -e

# Default configuration
INSTANCES="${INSTANCES:-owner}"
SPAWN_NODE="${SPAWN_NODE:-false}"
DB_DIR="${DB_DIR:-/tmp/sthalam_ai_test}"
AI_CMD_PIPE="/tmp/ai_cmd"
AI_OUT_LOG="/tmp/ai_out.log"
AI_PID_FILE="/tmp/ai_pid"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --instances)
            INSTANCES="$2"
            shift 2
            ;;
        --node)
            SPAWN_NODE="true"
            shift
            ;;
        --db-dir)
            DB_DIR="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [--instances owner,customer] [--node] [--db-dir /tmp/test]"
            echo ""
            echo "Options:"
            echo "  --instances   Comma-separated list of instance names (default: owner)"
            echo "  --node        Also spawn kunki node"
            echo "  --db-dir      Directory for database files (default: /tmp/sthalam_ai_test)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Find ai_interface binary
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
AI_INTERFACE="$PROJECT_ROOT/target/debug/ai_interface"

if [[ ! -x "$AI_INTERFACE" ]]; then
    echo "Error: ai_interface not found at $AI_INTERFACE"
    echo "Please build it first: cargo build -p ai_interface"
    exit 1
fi

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    if [[ -f "$AI_PID_FILE" ]]; then
        kill "$(cat "$AI_PID_FILE")" 2>/dev/null || true
        rm -f "$AI_PID_FILE"
    fi
    rm -f "$AI_CMD_PIPE"
    # Don't remove the log file - might be useful for debugging
}

# Check if already running
if [[ -f "$AI_PID_FILE" ]] && kill -0 "$(cat "$AI_PID_FILE")" 2>/dev/null; then
    echo "AI Interface already running (PID: $(cat "$AI_PID_FILE"))"
    echo "To restart, run: kill \$(cat $AI_PID_FILE) && rm $AI_CMD_PIPE"
    exit 1
fi

# Cleanup old files
rm -f "$AI_CMD_PIPE" "$AI_PID_FILE"

# Create named pipe for commands
mkfifo "$AI_CMD_PIPE"

# Create/truncate output log
> "$AI_OUT_LOG"

# Build command args
CMD_ARGS="--instances $INSTANCES --db-dir $DB_DIR"
if [[ "$SPAWN_NODE" == "true" ]]; then
    CMD_ARGS="$CMD_ARGS --node"
fi

echo "Starting AI Interface..."
echo "  Instances: $INSTANCES"
echo "  Node: $SPAWN_NODE"
echo "  DB Dir: $DB_DIR"
echo ""

# Start ai_interface in background
# Use a subshell to keep the pipe open
(
    # Keep the write end open so the pipe doesn't close when Claude writes
    exec 3>"$AI_CMD_PIPE"

    # Run ai_interface reading from pipe, writing to log
    "$AI_INTERFACE" $CMD_ARGS < "$AI_CMD_PIPE" >> "$AI_OUT_LOG" 2>&1 &
    AI_PID=$!
    echo $AI_PID > "$AI_PID_FILE"

    # Wait for ai_interface to exit
    wait $AI_PID
) &

# Wait for it to start
sleep 1

if [[ -f "$AI_PID_FILE" ]] && kill -0 "$(cat "$AI_PID_FILE")" 2>/dev/null; then
    echo "AI Interface started successfully!"
    echo ""
    echo "Commands pipe: $AI_CMD_PIPE"
    echo "Output log:    $AI_OUT_LOG"
    echo "PID file:      $AI_PID_FILE"
    echo ""
    echo "Claude can now:"
    echo "  Send: echo '{\"target\": \"owner\", \"action\": \"ui_get_screen\", \"id\": 1}' > $AI_CMD_PIPE"
    echo "  Read: tail -1 $AI_OUT_LOG"
    echo ""
    echo "To stop: kill \$(cat $AI_PID_FILE)"

    # Show initial output
    echo ""
    echo "=== Initial Output ==="
    sleep 1
    cat "$AI_OUT_LOG"
else
    echo "Error: AI Interface failed to start"
    echo "Check $AI_OUT_LOG for details"
    cat "$AI_OUT_LOG"
    exit 1
fi
