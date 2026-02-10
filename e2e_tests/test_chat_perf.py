#!/usr/bin/env python3
"""
Group Chat Performance Test — N messages, throughput measurement.

Tests message sync throughput with 3 peers (alice, bob, carol),
random sender selection, and optional burst testing.

Usage:
    python e2e_tests/test_chat_perf.py
    python e2e_tests/test_chat_perf.py --messages 100 --release
    python e2e_tests/test_chat_perf.py --release --flame-only -m 100 -o results/run.json
    python e2e_tests/test_chat_perf.py --release --heaptrack -m 100
"""

import sys
import time
import random
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario
from osvauld.perf import PerfReport

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

# Parse args (custom flags + standard flags)
parser = argparse.ArgumentParser(description="Group Chat Performance Test")
AppTestScenario.add_args(parser)
parser.add_argument("--messages", "-m", type=int, default=50,
                    help="Number of messages to send (default: 50)")
parser.add_argument("--burst", "-b", type=int, default=10,
                    help="Burst messages at end (default: 10)")
parser.add_argument("--delay", "-d", type=float, default=0.1,
                    help="Delay between messages in seconds (default: 0.1)")
parser.add_argument("--profile", action="store_true",
                    help="Enable tokio-console + flame profiling")
parser.add_argument("--flame-only", action="store_true",
                    help="Enable flame graphs only (no tokio-console overhead)")
parser.add_argument("--heaptrack", action="store_true",
                    help="Wrap binaries with heaptrack for heap profiling")
parser.add_argument("--output", "-o", type=str,
                    help="Save results to JSON file")
cli_args = parser.parse_args()


def escape_lua_string(s: str) -> str:
    """Escape a string for Lua."""
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "")


def generate_messages(count: int) -> list:
    """Generate chat messages."""
    try:
        from faker import Faker
        fake = Faker()
        return [fake.sentence(nb_words=random.randint(3, 12)) for _ in range(count)]
    except ImportError:
        templates = [
            "Hello everyone!", "Testing message sync",
            "Messages should sync automatically", "This is message {}",
            "Random thought: testing is important", "How is everyone doing?",
            "Just checking in!", "Great chat app!",
        ]
        return [t.format(i) if "{}" in t else t
                for i, t in enumerate(templates * (count // len(templates) + 1))][:count]


report = PerfReport()
report.build_profile = "release" if cli_args.release else "debug"

with AppTestScenario(
    name="chat_perf",
    app_path=str(DEMOS_APP),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
        "carol": {"role": "viewer", "app": "Group Chat"},
    },
    release=cli_args.release,
    profiling=cli_args.profile,
    flame_only=cli_args.flame_only,
    heaptrack=cli_args.heaptrack,
    keep=cli_args.keep,
    debug=cli_args.debug,
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")
    carol = s.peer("carol")

    peers = {"alice": alice, "bob": bob, "carol": carol}
    peer_list = list(peers.items())

    report.mark("setup_complete")

    # Get PIDs for resource monitoring
    pids = s._tm.get_all_pids()

    # --- Send messages ---
    print(f"Sending {cli_args.messages} messages...")
    report.mark("messages_start")

    messages = generate_messages(cli_args.messages)
    total_sent = 0

    for i, msg in enumerate(messages):
        name, peer = random.choice(peer_list)
        escaped = escape_lua_string(msg)
        try:
            peer.eval(f'send_message("{escaped}")')
            total_sent += 1
            if i < 5 or i % 10 == 0:
                display = msg[:40] + "..." if len(msg) > 40 else msg
                print(f"  [{i+1}/{cli_args.messages}] {name}: \"{display}\"")
            if i % 5 == 0:
                report.sample_processes(pids)
            time.sleep(cli_args.delay)
        except Exception as e:
            print(f"  [ERROR] {name} failed: {e}")

    print(f"  Sent {total_sent} messages")
    report.mark("messages_done")
    report.message_count = total_sent

    # Wait for sync
    time.sleep(2)
    report.mark("sync_wait_done")

    # Check message counts
    print("\nChecking message sync...")
    counts = {}
    for name, peer in peers.items():
        try:
            counts[name] = peer.eval("return get_message_count()")
        except Exception as e:
            counts[name] = f"error: {e}"

    for name, count in counts.items():
        status = "[OK]" if count == total_sent else f"[WARN expected {total_sent}]"
        print(f"  {name}: {count} messages {status}")

    if all(c == total_sent for c in counts.values() if isinstance(c, int)):
        print(f"  [PASS] All peers synced to {total_sent} messages")

    # --- Burst test ---
    if cli_args.burst > 0:
        print(f"\nBurst: {cli_args.burst} messages at 50ms...")
        burst_messages = generate_messages(cli_args.burst)
        burst_sent = 0
        for msg in burst_messages:
            name, peer = random.choice(peer_list)
            try:
                peer.eval(f'send_message("{escape_lua_string(msg)}")')
                burst_sent += 1
            except Exception:
                pass
            time.sleep(0.05)

        time.sleep(2)
        expected_total = total_sent + burst_sent
        for name, peer in peers.items():
            try:
                counts[name] = peer.eval("return get_message_count()")
            except Exception as e:
                counts[name] = f"error: {e}"

        print(f"  After burst (expected {expected_total}):")
        for name, count in counts.items():
            print(f"    {name}: {count}")

    report.mark("test_complete")
    report.print_report()

    if cli_args.output:
        report.save_json(cli_args.output)
