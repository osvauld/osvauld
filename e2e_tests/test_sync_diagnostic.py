#!/usr/bin/env python3
"""
Sync Diagnostic — alice sends, bob receives. Measure where time is spent.

Isolates: sync protocol latency, UI delivery, convergence speed.
Sends from alice only, polls bob's message count at intervals to see
how quickly bob's CRDT converges vs how quickly bob's UI updates.
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

parser = argparse.ArgumentParser(description="Sync Diagnostic: alice→bob")
AppTestScenario.add_args(parser)
parser.add_argument(
    "--messages",
    "-m",
    type=int,
    default=500,
    help="Messages alice sends (default: 500)",
)
parser.add_argument("--output", "-o", type=str, help="Save results to JSON file")
cli_args = parser.parse_args()


def escape_lua_string(s):
    return (
        s.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", "\\n")
        .replace("\r", "")
    )


def generate_messages(count):
    templates = [
        "Hello everyone!",
        "Testing message sync",
        "Messages should sync automatically",
        "This is message {}",
        "Random thought: testing is important",
        "How is everyone doing?",
    ]
    return [
        t.format(i) if "{}" in t else t
        for i, t in enumerate(templates * (count // len(templates) + 1))
    ][:count]


report = PerfReport()
report.build_profile = "release" if cli_args.release else "debug"

with AppTestScenario(
    name="sync_diag",
    app_path=str(DEMOS_APP),
    peers={
        "alice": {"role": "owner", "app": "Group Chat"},
        "bob": {"role": "viewer", "app": "Group Chat"},
    },
    release=cli_args.release,
    keep=cli_args.keep,
    debug=cli_args.debug,
    flame_only=getattr(cli_args, "flame_only", False),
    heaptrack=getattr(cli_args, "heaptrack", False),
) as s:
    alice = s.peer("alice")
    bob = s.peer("bob")

    report.mark("setup_complete")
    pids = s._tm.get_all_pids()

    messages = generate_messages(cli_args.messages)

    # --- Phase 1: Send all from alice, track bob's count periodically ---
    print(
        f"\n[Diagnostic] alice sends {cli_args.messages} messages, polling bob...",
        flush=True,
    )
    report.mark("send_start")

    sent = 0
    bob_samples = []  # (elapsed_s, alice_sent, bob_count)
    sample_interval = max(1, cli_args.messages // 20)
    send_latencies_ms = []
    phase_start = time.time()

    for i, msg in enumerate(messages):
        escaped = escape_lua_string(msg)
        try:
            t0 = time.perf_counter()
            alice.eval(f'send_message("{escaped}")')
            send_latencies_ms.append((time.perf_counter() - t0) * 1000.0)
            sent += 1
        except Exception as e:
            print(f"  [ERROR] alice send failed at {i}: {e}", flush=True)

        # Sample bob's count periodically
        if (i + 1) % sample_interval == 0 or i < 5:
            try:
                bob_count = bob.eval("return get_message_count()")
            except Exception:
                bob_count = -1
            elapsed = time.time() - phase_start
            bob_samples.append((round(elapsed, 2), sent, bob_count))
            lag = sent - (bob_count if isinstance(bob_count, int) else 0)
            pct = ((i + 1) / cli_args.messages) * 100
            p50 = statistics.median(send_latencies_ms[-50:]) if send_latencies_ms else 0
            print(
                f"  [{i + 1}/{cli_args.messages} | {pct:.0f}%] "
                f"alice_sent={sent} bob_has={bob_count} lag={lag} "
                f"eval_p50={p50:.1f}ms",
                flush=True,
            )
            report.sample_processes(pids)

    report.mark("send_done")
    send_duration = report.events["send_done"] - report.events["send_start"]
    print(
        f"\n  Alice sent {sent} in {send_duration:.1f}s ({sent / send_duration:.0f} msg/s)"
    )

    if send_latencies_ms:
        p50 = statistics.median(send_latencies_ms)
        p95 = (
            statistics.quantiles(send_latencies_ms, n=100)[94]
            if len(send_latencies_ms) >= 2
            else send_latencies_ms[0]
        )
        print(f"  Send eval: p50={p50:.1f}ms p95={p95:.1f}ms")
        report.latencies["send_eval_p50_ms"] = p50
        report.latencies["send_eval_p95_ms"] = p95

    # --- Phase 2: Wait for bob to converge ---
    print(f"\n[Convergence] Waiting for bob to reach {sent}...", flush=True)
    report.mark("convergence_start")

    convergence_samples = []
    conv_start = time.time()
    poll_interval = 1.0
    timeout = 120.0

    while time.time() - conv_start < timeout:
        try:
            alice_count = alice.eval("return get_message_count()")
            bob_count = bob.eval("return get_message_count()")
        except Exception:
            alice_count, bob_count = -1, -1

        elapsed = time.time() - conv_start
        convergence_samples.append((round(elapsed, 1), alice_count, bob_count))
        print(
            f"  t={elapsed:.0f}s  alice={alice_count}  bob={bob_count}  target={sent}",
            flush=True,
        )

        if isinstance(bob_count, int) and bob_count >= sent:
            print(f"  Bob converged at {elapsed:.1f}s", flush=True)
            break

        time.sleep(poll_interval)

    report.mark("convergence_done")
    conv_time = report.events["convergence_done"] - report.events["convergence_start"]
    report.latencies["convergence_time_s"] = conv_time

    # --- Summary ---
    print(f"\n{'=' * 64}")
    print(f"  SYNC DIAGNOSTIC SUMMARY")
    print(f"{'=' * 64}")
    print(f"  Messages: {sent}")
    print(f"  Send duration: {send_duration:.1f}s ({sent / send_duration:.0f} msg/s)")
    print(f"  Convergence time: {conv_time:.1f}s")
    if bob_samples:
        # Lag during send
        lags = [s[1] - s[2] for s in bob_samples if isinstance(s[2], int)]
        if lags:
            print(
                f"  Lag during send: min={min(lags)} max={max(lags)} avg={sum(lags) / len(lags):.0f}"
            )
    print(f"\n  Bob count samples during send:")
    for elapsed, alice_sent, bob_count in bob_samples:
        print(f"    t={elapsed:6.2f}s  alice={alice_sent:5d}  bob={bob_count}")
    print(f"\n  Convergence polling:")
    for elapsed, ac, bc in convergence_samples:
        print(f"    t={elapsed:5.1f}s  alice={ac}  bob={bc}")
    print(f"{'=' * 64}")

    report.message_count = sent
    report.mark("test_complete")
    report.print_report()

    if cli_args.output:
        report.save_json(cli_args.output)
