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
import statistics
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "scripts"))

from osvauld.scenario import AppTestScenario
from osvauld.perf import PerfReport

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"

# Parse args (custom flags + standard flags)
parser = argparse.ArgumentParser(description="Group Chat Performance Test")
AppTestScenario.add_args(parser)
parser.add_argument(
    "--messages",
    "-m",
    type=int,
    default=50,
    help="Number of messages to send (default: 50)",
)
parser.add_argument(
    "--burst", "-b", type=int, default=10, help="Burst messages at end (default: 10)"
)
parser.add_argument(
    "--delay",
    "-d",
    type=float,
    default=0.1,
    help="Delay between messages in seconds (default: 0.1)",
)
parser.add_argument(
    "--profile", action="store_true", help="Enable tokio-console + flame profiling"
)
parser.add_argument("--output", "-o", type=str, help="Save results to JSON file")
parser.add_argument(
    "--send-only",
    action="store_true",
    help="Run only phase-1 send benchmark (skip distributed phase)",
)
parser.add_argument(
    "--sync-timeout",
    type=float,
    default=120.0,
    help="Max seconds to wait for sync convergence (default: 120)",
)
parser.add_argument(
    "--sync-poll",
    type=float,
    default=2.0,
    help="Seconds between sync convergence polls (default: 2.0)",
)
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


def wait_for_sync(peers, expected, timeout, poll_interval):
    """Poll peers until all reach expected count or timeout."""
    start = time.time()
    while time.time() - start < timeout:
        counts = {}
        for name, peer in peers.items():
            try:
                counts[name] = peer.eval("return get_message_count()")
            except Exception as e:
                counts[name] = -1

        int_counts = [c for c in counts.values() if isinstance(c, int) and c >= 0]
        if int_counts:
            lo, hi = min(int_counts), max(int_counts)
        else:
            lo, hi = 0, 0

        elapsed = time.time() - start
        if all(c == expected for c in int_counts) and len(int_counts) == len(peers):
            print(
                f"  Sync converged at {elapsed:.1f}s — all peers at {expected}",
                flush=True,
            )
            return counts, elapsed

        print(
            f"  Waiting sync... {elapsed:.0f}s  min={lo} max={hi} target={expected}",
            flush=True,
        )
        time.sleep(poll_interval)

    # Timeout — return whatever we have
    counts = {}
    for name, peer in peers.items():
        try:
            counts[name] = peer.eval("return get_message_count()")
        except Exception as e:
            counts[name] = f"error: {e}"
    elapsed = time.time() - start
    print(f"  Sync timeout after {elapsed:.1f}s", flush=True)
    return counts, elapsed


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

    # --- Phase 1: Pure send path (alice only, no delay) ---
    print(
        f"\n[Phase 1] Send-path benchmark: alice sends {cli_args.messages} messages (no delay)..."
    )
    report.mark("send_phase_start")

    send_messages = generate_messages(cli_args.messages)
    send_count = 0
    progress_interval = max(1, cli_args.messages // 20)  # Every ~5%
    send_eval_latencies_ms = []
    phase1_started_at = time.time()

    for i, msg in enumerate(send_messages):
        escaped = escape_lua_string(msg)
        try:
            t0 = time.perf_counter()
            alice.eval(f'send_message("{escaped}")')
            send_eval_latencies_ms.append((time.perf_counter() - t0) * 1000.0)
            send_count += 1
            # Show progress: first 3, then every ~5%
            if i < 3 or (i + 1) % progress_interval == 0:
                pct = ((i + 1) / cli_args.messages) * 100
                elapsed = time.time() - phase1_started_at
                rate = (send_count / elapsed) if elapsed > 0 else 0.0
                p50 = (
                    statistics.median(send_eval_latencies_ms[-100:])
                    if send_eval_latencies_ms
                    else 0.0
                )
                print(
                    f"  [{i + 1}/{cli_args.messages} | {pct:.0f}%] elapsed={elapsed:.1f}s rate={rate:.1f} msg/s eval_p50(last100)={p50:.1f}ms",
                    flush=True,
                )
        except Exception as e:
            print(f"  [ERROR] alice send failed: {e}", flush=True)

    report.mark("send_phase_done")
    send_duration = report.events["send_phase_done"] - report.events["send_phase_start"]
    send_throughput = send_count / send_duration if send_duration > 0 else 0
    print(
        f"  Sent {send_count} messages in {send_duration:.2f}s ({send_throughput:.1f} msg/s)"
    )

    if send_eval_latencies_ms:
        p50 = statistics.median(send_eval_latencies_ms)
        p95 = (
            statistics.quantiles(send_eval_latencies_ms, n=100)[94]
            if len(send_eval_latencies_ms) >= 2
            else send_eval_latencies_ms[0]
        )
        print(f"  Send eval latency: p50={p50:.1f}ms p95={p95:.1f}ms")
        report.latencies["send_eval_p50_ms"] = p50
        report.latencies["send_eval_p95_ms"] = p95

    report.latencies["send_throughput_msg_per_sec"] = send_throughput
    report.latencies["send_avg_latency_ms"] = (
        (send_duration * 1000 / send_count) if send_count > 0 else 0
    )

    # --- Phase 2: Distributed send with sync (original test) ---
    if cli_args.send_only:
        report.message_count = send_count
        report.events["messages_start"] = report.events["send_phase_start"]
        report.events["messages_done"] = report.events["send_phase_done"]
        report.mark("sync_wait_done")
    else:
        print(
            f"\n[Phase 2] Distributed send: {cli_args.messages} messages with {cli_args.delay}s delay..."
        )
        report.mark("messages_start")

        messages = generate_messages(cli_args.messages)
        total_sent = 0
        progress_interval = max(1, cli_args.messages // 20)  # Every ~5%

        for i, msg in enumerate(messages):
            name, peer = random.choice(peer_list)
            escaped = escape_lua_string(msg)
            try:
                peer.eval(f'send_message("{escaped}")')
                total_sent += 1
                # Show progress: first 5, then every 1% or every 100 messages
                if i < 5 or (i + 1) % progress_interval == 0 or (i + 1) % 100 == 0:
                    pct = ((i + 1) / cli_args.messages) * 100
                    display = msg[:40] + "..." if len(msg) > 40 else msg
                    print(
                        f'  [{i + 1}/{cli_args.messages} | {pct:.0f}%] {name}: "{display}"',
                        flush=True,
                    )
                if i % 5 == 0:
                    report.sample_processes(pids)
                time.sleep(cli_args.delay)
            except Exception as e:
                print(f"  [ERROR] {name} failed: {e}", flush=True)

        print(f"  Sent {total_sent} messages")
        report.mark("messages_done")
        report.message_count = total_sent + send_count

        # Wait for sync convergence
        expected_total = send_count + total_sent
        print(
            f"\nWaiting for sync convergence (expected {expected_total}, timeout {cli_args.sync_timeout}s)...",
            flush=True,
        )
        counts, sync_elapsed = wait_for_sync(
            peers, expected_total, cli_args.sync_timeout, cli_args.sync_poll
        )
        report.mark("sync_wait_done")
        report.latencies["sync_convergence_time_s"] = sync_elapsed

        for name, count in counts.items():
            status = (
                "[OK]"
                if count == expected_total
                else f"[WARN expected {expected_total}]"
            )
            print(f"  {name}: {count} messages {status}")

        if all(c == expected_total for c in counts.values() if isinstance(c, int)):
            print(f"  [PASS] All peers synced to {expected_total} messages")

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

            expected_burst_total = send_count + total_sent + burst_sent
            print(
                f"  Waiting for burst sync (expected {expected_burst_total})...",
                flush=True,
            )
            counts, _ = wait_for_sync(
                peers, expected_burst_total, 30.0, cli_args.sync_poll
            )

            print(f"  After burst (expected {expected_burst_total}):")
            for name, count in counts.items():
                print(f"    {name}: {count}")

    report.mark("test_complete")
    report.print_report()

    if cli_args.output:
        report.save_json(cli_args.output)
