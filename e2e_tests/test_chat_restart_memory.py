#!/usr/bin/env python3
"""
Chat Restart Memory Test — Persistence/restart memory performance scenario.

Tests memory usage after sending many messages, then quitting and reopening
from persisted data. Measures memory at restart checkpoints.

Usage:
    python e2e_tests/test_chat_restart_memory.py
    python e2e_tests/test_chat_restart_memory.py --messages 5000 --release
    python e2e_tests/test_chat_restart_memory.py --release -m 1000 -o results/restart.json
"""

import sys
import time
import json
import random
import argparse
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

# Parse args
parser = argparse.ArgumentParser(description="Chat Restart Memory Test")
AppTestScenario.add_args(parser)
parser.add_argument(
    "--messages",
    "-m",
    type=int,
    default=100,
    help="Number of messages to send (default: 100)",
)
parser.add_argument("--output", "-o", type=str, help="Save results to JSON file")
cli_args = parser.parse_args()


def escape_lua_string(s: str) -> str:
    """Escape a string for Lua."""
    return (
        s.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", "\\n")
        .replace("\r", "")
    )


def generate_messages(count: int) -> list:
    """Generate chat messages."""
    try:
        from faker import Faker

        fake = Faker()
        return [fake.sentence(nb_words=random.randint(3, 12)) for _ in range(count)]
    except ImportError:
        templates = [
            "Hello everyone!",
            "Testing message sync",
            "Messages should sync automatically",
            "This is message {}",
            "Random thought: testing is important",
            "How is everyone doing?",
            "Just checking in!",
            "Great chat app!",
        ]
        return [
            t.format(i) if "{}" in t else t
            for i, t in enumerate(templates * (count // len(templates) + 1))
        ][:count]


def get_memory_usage(pids: dict) -> dict:
    """Get current memory usage for processes using ps."""
    if not pids:
        return {}

    pid_list = ",".join(str(p) for p in pids.values() if p)
    if not pid_list:
        return {}

    try:
        result = subprocess.run(
            ["ps", "-o", "pid=,rss=", "-p", pid_list],
            capture_output=True,
            text=True,
            timeout=2,
        )

        memory = {}
        for line in result.stdout.strip().split("\n"):
            if not line.strip():
                continue
            parts = line.split()
            if len(parts) >= 2:
                try:
                    pid = int(parts[0])
                    rss_kb = int(parts[1])
                    # Find name for this PID
                    for name, p in pids.items():
                        if p == pid:
                            memory[name] = round(rss_kb / 1024, 1)
                            break
                except (ValueError, IndexError):
                    pass
        return memory
    except Exception:
        return {}


# ============================================================
# PHASE A: Initial run with messages
# ============================================================

print("\n" + "=" * 64)
print("  PHASE A: Initial run with messages")
print("=" * 64)

base_dir = Path(f"/tmp/chat_restart_{cli_args.messages}")

# Phase A: send messages and capture end-of-phase memory
with AppTestScenario(
    name=f"chat_restart_{cli_args.messages}",
    app_path=str(DEMOS_APP),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    base_dir=str(base_dir),
    release=cli_args.release,
    keep=False,
    debug=cli_args.debug,
    fresh=True,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    # Save space/page IDs for reuse
    space_id = s.space_id
    page_id = s.page_id

    print(f"\nSending {cli_args.messages} messages...")
    messages = generate_messages(cli_args.messages)
    sent_count = 0

    for i, msg in enumerate(messages):
        escaped = escape_lua_string(msg)
        try:
            alice.eval(f'send_message("{escaped}")')
            sent_count += 1
            if i < 3 or i % 50 == 0:
                display = msg[:40] + "..." if len(msg) > 40 else msg
                print(f'  [{i + 1}/{cli_args.messages}] alice: "{display}"')
        except Exception as e:
            print(f"  [ERROR] alice send failed: {e}")

    print(f"  Sent {sent_count} messages")

    # Wait for sync
    time.sleep(2)

    # Verify sync
    alice_count = alice.eval("return get_message_count()")
    bob_count = bob.eval("return get_message_count()")
    print(f"\n  alice: {alice_count} messages")
    print(f"  bob:   {bob_count} messages")

    if alice_count == sent_count and bob_count == sent_count:
        print(f"  [PASS] All peers synced to {sent_count} messages")
    else:
        print(f"  [WARN] Sync mismatch (expected {sent_count})")

    # Capture end-of-phase memory
    pids = s._tm.get_all_pids()
    phase_a_memory = get_memory_usage(pids)
    print(f"\n  Phase A end memory:")
    for name in sorted(phase_a_memory.keys()):
        print(f"    {name:12s}: {phase_a_memory[name]:6.0f} MB")

# Phase A context exits here, stopping all processes

print("\n  Phase A complete. Processes stopped.")
print(f"  Data persisted in: {base_dir}")

# ============================================================
# PHASE B: Restart and measure memory
# ============================================================

print("\n" + "=" * 64)
print("  PHASE B: Restart from persisted data")
print("=" * 64)

# Wait a bit before restart
time.sleep(1)

restart_checkpoints = [0, 5, 15, 30]  # seconds after restart
restart_memory = {}

with AppTestScenario(
    name=f"chat_restart_{cli_args.messages}",
    app_path=str(DEMOS_APP),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    base_dir=str(base_dir),
    release=cli_args.release,
    keep=cli_args.keep,
    debug=cli_args.debug,
    fresh=False,
    reuse_space_id=space_id,
    reuse_page_id=page_id,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    print(f"\n  Measuring memory at restart checkpoints: {restart_checkpoints}s")

    pids = s._tm.get_all_pids()

    for checkpoint in restart_checkpoints:
        if checkpoint > 0:
            time.sleep(
                checkpoint
                - (
                    restart_checkpoints[restart_checkpoints.index(checkpoint) - 1]
                    if restart_checkpoints.index(checkpoint) > 0
                    else 0
                )
            )

        memory = get_memory_usage(pids)
        restart_memory[f"t{checkpoint}s"] = memory
        print(f"\n  Memory at t={checkpoint}s:")
        for name in sorted(memory.keys()):
            print(f"    {name:12s}: {memory[name]:6.0f} MB")

    # Verify message count after restart
    alice_count = alice.eval("return get_message_count()")
    bob_count = bob.eval("return get_message_count()")
    print(f"\n  After restart:")
    print(f"    alice: {alice_count} messages")
    print(f"    bob:   {bob_count} messages")

    if alice_count == sent_count and bob_count == sent_count:
        print(f"  [PASS] Messages persisted correctly ({sent_count})")
    else:
        print(f"  [WARN] Message count mismatch after restart")

# ============================================================
# Report and save results
# ============================================================

print("\n" + "=" * 64)
print("  RESTART MEMORY REPORT")
print("=" * 64)

print(f"\n  Messages: {sent_count}")
print(f"  Build: {'release' if cli_args.release else 'debug'}")

print(f"\n  Phase A (end of initial run):")
for name in sorted(phase_a_memory.keys()):
    print(f"    {name:12s}: {phase_a_memory[name]:6.0f} MB")

print(f"\n  Phase B (restart checkpoints):")
for checkpoint_label in sorted(restart_memory.keys(), key=lambda x: int(x[1:-1])):
    print(f"  {checkpoint_label}:")
    for name in sorted(restart_memory[checkpoint_label].keys()):
        print(f"    {name:12s}: {restart_memory[checkpoint_label][name]:6.0f} MB")

# Calculate memory growth from restart to final checkpoint
if restart_memory:
    first_checkpoint = f"t{restart_checkpoints[0]}s"
    last_checkpoint = f"t{restart_checkpoints[-1]}s"

    print(f"\n  Memory growth (restart to t={restart_checkpoints[-1]}s):")
    for name in sorted(restart_memory[first_checkpoint].keys()):
        if name in restart_memory[last_checkpoint]:
            start = restart_memory[first_checkpoint][name]
            end = restart_memory[last_checkpoint][name]
            growth = end - start
            print(f"    {name:12s}: {start:.0f} -> {end:.0f} MB (+{growth:.0f} MB)")

print("\n" + "=" * 64)

# Save JSON output
if cli_args.output:
    result = {
        "messages": sent_count,
        "build_profile": "release" if cli_args.release else "debug",
        "phase_a_memory_mb": phase_a_memory,
        "restart_memory_mb": restart_memory,
        "restart_checkpoints_s": restart_checkpoints,
    }

    output_path = Path(cli_args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        json.dump(result, f, indent=2)
    print(f"\n  Results saved to: {cli_args.output}")
