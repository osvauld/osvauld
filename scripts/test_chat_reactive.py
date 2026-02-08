#!/usr/bin/env python3
"""
Test Group Chat with reactive binding system.

Tests:
- Multiple users sending messages
- Message sync across all users
- Online user count updates (surgical updates via key binding)
- UI state verification

Usage:
    python scripts/test_chat_reactive.py [--messages N]
    python scripts/test_chat_reactive.py --release --messages 100
    python scripts/test_chat_reactive.py --release --profile --messages 100
    python scripts/test_chat_reactive.py --release --flame-only --messages 50
    python scripts/test_chat_reactive.py --release --messages 100 --output results/run1.json

Requirements:
    pip install faker
"""

import sys
import time
import random
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from osvauld.tmux import TmuxManager
from osvauld.perf import PerfReport
from analyze_flamegraph import analyze_file as analyze_flamegraph_file

# Try to import Faker, fall back to simple messages if not available
try:
    from faker import Faker
    fake = Faker()
    HAS_FAKER = True
except ImportError:
    HAS_FAKER = False
    print("Note: Install faker for more realistic messages: pip install faker")

DEMOS_APP = Path(__file__).parent.parent / "sample_apps" / "osvauld-demos"


def generate_messages(count: int) -> list:
    """Generate realistic chat messages using Faker."""
    if HAS_FAKER:
        message_types = [
            lambda: fake.sentence(nb_words=random.randint(3, 12)),
            lambda: fake.text(max_nb_chars=80),
            lambda: f"Hey, {fake.first_name()}! {fake.sentence()}",
            lambda: f"Anyone know about {fake.catch_phrase()}?",
            lambda: f"Just finished {fake.bs()}",
            lambda: f"LOL {fake.sentence(nb_words=5)}",
            lambda: f"@someone {fake.sentence(nb_words=6)}",
            lambda: fake.paragraph(nb_sentences=2),
            lambda: f"Check this out: {fake.url()}",
            lambda: f"{fake.emoji()} {fake.sentence(nb_words=4)}",
        ]
        return [random.choice(message_types)() for _ in range(count)]
    else:
        # Fallback messages without Faker
        templates = [
            "Hello everyone!",
            "Testing the reactive binding system",
            "Messages should sync automatically",
            "No manual refresh needed",
            "This uses scribe:bind() under the hood",
            "Surgical updates preserve scroll position",
            "Only changed items update, not the whole list",
            "The key option enables fine-grained updates",
            "How is everyone doing today?",
            "Just checking in!",
            "This is message number {}",
            "Random thought: testing is important",
            "Anyone here?",
            "Great chat app!",
            "The sync is working well",
        ]
        return [t.format(i) if "{}" in t else t for i, t in enumerate(templates * (count // len(templates) + 1))][:count]


def wait_for_sync(clients, check_code, expected, timeout=10, desc="sync"):
    """Wait for all clients to reach expected state."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            results = {}
            all_match = True
            for name, client in clients.items():
                result = client.eval(check_code)
                results[name] = result
                if result != expected:
                    all_match = False
            if all_match:
                return True, results
        except Exception as e:
            pass
        time.sleep(0.5)
    return False, results


def escape_lua_string(s: str) -> str:
    """Escape a string for Lua."""
    return s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\r", "")


def main():
    parser = argparse.ArgumentParser(description="Test reactive binding with chat")
    parser.add_argument("--messages", "-m", type=int, default=50,
                        help="Number of messages to generate (default: 50)")
    parser.add_argument("--burst", "-b", type=int, default=10,
                        help="Number of rapid burst messages (default: 10)")
    parser.add_argument("--delay", "-d", type=float, default=0.1,
                        help="Delay between messages in seconds (default: 0.1)")
    parser.add_argument("--release", "-r", action="store_true",
                        help="Use release builds (faster)")
    parser.add_argument("--profile", action="store_true",
                        help="Enable tokio-console + flame profiling")
    parser.add_argument("--flame-only", action="store_true",
                        help="Enable flame graphs only (no tokio-console overhead)")
    parser.add_argument("--heaptrack", action="store_true",
                        help="Wrap binaries with heaptrack for heap profiling")
    parser.add_argument("--output", "-o", type=str,
                        help="Save performance results to JSON file")
    args = parser.parse_args()

    # Initialize performance report
    report = PerfReport()
    report.build_profile = "release" if args.release else "debug"

    print("=" * 60)
    print("  Reactive Binding System Test - Group Chat")
    print("=" * 60)
    print(f"  Messages: {args.messages}, Burst: {args.burst}, Delay: {args.delay}s")
    profiling_mode = "flame-only" if args.flame_only else ("full" if args.profile else "disabled")
    print(f"  Build: {'release' if args.release else 'debug'}")
    print(f"  Profiling: {profiling_mode}")
    if args.heaptrack:
        print(f"  Heaptrack: enabled (output in /tmp/chat_reactive_test/<instance>/)")
    print(f"  Faker: {'available' if HAS_FAKER else 'not installed (using fallback)'}")
    print()

    tm = TmuxManager(
        session_name="chat_reactive_test",
        base_dir=Path("/tmp/chat_reactive_test"),
        release=args.release,
        profiling=args.profile,
        flame_only=args.flame_only,
        heaptrack=args.heaptrack,
    )
    tm.add_node("node")
    tm.add_shell("alice")
    tm.add_shell("bob")
    tm.add_shell("carol")
    tm.start()

    # Print tokio-console info if full profiling (not flame-only)
    if args.profile and not args.flame_only:
        tm.print_console_info()

    report.mark("instances_started")

    node = tm.get_client("node")
    alice = tm.get_client("alice")
    bob = tm.get_client("bob")
    carol = tm.get_client("carol")

    clients = {"alice": alice, "bob": bob, "carol": carol}
    print("[OK] All instances ready")
    report.mark("clients_ready")

    # Get PIDs for resource monitoring
    pids = tm.get_all_pids()

    # Debug: verify PIDs are correct
    print(f"\n  PID map: {pids}")
    for name, pid in pids.items():
        try:
            import subprocess as sp
            cmdline = sp.run(['cat', f'/proc/{pid}/cmdline'], capture_output=True, text=True, timeout=2)
            comm = sp.run(['cat', f'/proc/{pid}/comm'], capture_output=True, text=True, timeout=2)
            status = sp.run(['grep', 'VmRSS', f'/proc/{pid}/status'], capture_output=True, text=True, timeout=2)
            print(f"  {name} (PID {pid}): comm={comm.stdout.strip()}, rss={status.stdout.strip()}")
            print(f"    cmdline: {cmdline.stdout.replace(chr(0), ' ')[:120]}")
        except Exception as e:
            print(f"  {name} (PID {pid}): error reading proc: {e}")

    # === Setup Phase ===
    print("\n--- Setup Phase ---")

    # Alice creates space
    print("Alice: signup, create space...")
    alice.signup_or_login("alice")
    space = alice.create_space_with_pages(str(DEMOS_APP))
    space_id = space["id"]
    pages = alice.list_pages(space_id)
    page_id = pages[0]["id"]
    print(f"  Space: {space_id[:12]}... Page: {page_id[:12]}...")

    # Connect and publish
    print("Alice: connect to node, publish...")
    alice.connect_to_node(node)

    # Wait for node to appear in list
    node_id = None
    for i in range(20):
        time.sleep(0.25)
        nodes = alice.list_nodes()
        if nodes:
            node_id = nodes[0].get("node_id")
            break
    if not node_id:
        print("ERROR: No nodes after connect")
        return 1
    print(f"  Waiting for authentication with node {node_id[:12]}...")

    for i in range(60):
        time.sleep(0.5)
        if alice.is_node_authenticated(node_id):
            print(f"  Authenticated in {(i+1)*0.5:.1f}s")
            break
        if i % 10 == 0 and i > 0:
            print(f"  Still waiting for auth... ({(i+1)*0.5:.1f}s)")
    else:
        print("ERROR: Authentication timeout after 30s")
        return 1

    # Now publish (auth is complete)
    alice.publish_to_node(space_id)
    time.sleep(0.5)

    # Verify publish by getting shareable link
    for i in range(20):
        try:
            link = alice.get_viewer_link(space_id)
            if link:
                print(f"  Published and verified")
                break
        except RuntimeError as e:
            if "not found" in str(e).lower():
                time.sleep(0.5)
                continue
            raise
    else:
        print("ERROR: Failed to get shareable link after publish")
        return 1

    # Bob and Carol subscribe
    for name, client in [("Bob", bob), ("Carol", carol)]:
        print(f"{name}: signup, subscribe...")
        client.signup_or_login(name.lower())
        alice.add_viewer(client, space_id)

        # Wait for sync
        for i in range(30):
            time.sleep(0.5)
            spaces = client.list_spaces()
            if spaces:
                pages_list = client.list_pages(spaces[0]["id"])
                if pages_list:
                    apps = client.list_apps(pages_list[0]["id"])
                    if any(a.get("name") == "Group Chat" for a in apps):
                        print(f"  {name} synced in {(i+1)*0.5:.1f}s")
                        break
        else:
            print(f"ERROR: {name} failed to sync")
            return 1

    # Open apps for all users
    print("\nOpening Group Chat for all users...")
    alice.open_app(page_id, "Group Chat")
    time.sleep(1)

    for name, client in [("bob", bob), ("carol", carol)]:
        spaces = client.list_spaces()
        if spaces:
            pages_list = client.list_pages(spaces[0]["id"])
            if pages_list:
                client.open_app(pages_list[0]["id"], "Group Chat")
    time.sleep(2)

    # Set usernames
    for name, client in clients.items():
        client.eval(f'USERNAME = "{name.capitalize()}"')
        client.eval(f'my_name = USERNAME')

    # === Test 1: Online Count (SKIPPED - presence disabled) ===
    print("\n--- Test 1: Online Count (SKIPPED - presence disabled for debugging) ---")

    # === Test 2: Send Messages ===
    print("\n--- Test 2: Message Sending and Sync ---")
    report.mark("messages_start")

    # Generate messages using Faker
    messages = generate_messages(args.messages)
    client_list = list(clients.items())

    total_sent = 0
    print(f"  Sending {args.messages} messages...")
    for i, msg in enumerate(messages):
        name, client = random.choice(client_list)
        escaped_msg = escape_lua_string(msg)
        try:
            client.eval(f'send_message("{escaped_msg}")')
            total_sent += 1
            display_msg = msg[:40] + "..." if len(msg) > 40 else msg
            if i < 10 or i % 10 == 0:  # Show first 10, then every 10th
                print(f"  [{i+1}/{args.messages}] {name}: \"{display_msg}\"")
            # Sample resources every 5 messages for better growth curves
            if i % 5 == 0:
                report.sample_processes(pids)
            time.sleep(args.delay)
        except Exception as e:
            print(f"  [ERROR] {name} failed to send: {e}")

    print(f"\n  Sent {total_sent} messages total")
    report.mark("messages_done")
    report.message_count = total_sent

    # Wait for sync
    time.sleep(2)
    report.mark("sync_wait_done")

    # Check message counts
    print("\n  Checking message sync...")
    counts = {}
    for name, client in clients.items():
        try:
            count = client.eval('return get_message_count()')
            counts[name] = count
        except Exception as e:
            counts[name] = f"error: {e}"

    print(f"  Alice message count: {counts.get('alice', '?')}")
    print(f"  Bob message count:   {counts.get('bob', '?')}")
    print(f"  Carol message count: {counts.get('carol', '?')}")

    if all(c == total_sent for c in counts.values() if isinstance(c, int)):
        print(f"  [PASS] All users have {total_sent} messages")
    else:
        print(f"  [WARN] Message counts don't match expected {total_sent}")

    # === Test 3: Rapid Message Burst ===
    print("\n--- Test 3: Rapid Message Burst (Stress Test) ---")

    burst_messages = generate_messages(args.burst)
    burst_sent = 0
    for i, msg in enumerate(burst_messages):
        name, client = random.choice(client_list)
        escaped_msg = escape_lua_string(msg)
        try:
            client.eval(f'send_message("{escaped_msg}")')
            burst_sent += 1
            display_msg = msg[:40] + "..." if len(msg) > 40 else msg
            print(f"  [{i+1}/{args.burst}] {name}: {display_msg}")
        except Exception as e:
            print(f"  [ERROR] {e}")
        time.sleep(0.05)  # Very fast burst (50ms)

    time.sleep(2)  # Wait for sync

    expected_total = total_sent + burst_sent
    counts = {}
    for name, client in clients.items():
        try:
            count = client.eval('return get_message_count()')
            counts[name] = count
        except Exception as e:
            counts[name] = f"error: {e}"

    print(f"\n  After burst:")
    for name, count in counts.items():
        print(f"    {name}: {count} messages")

    if all(c == expected_total for c in counts.values() if isinstance(c, int)):
        print(f"  [PASS] All users synced to {expected_total} messages after burst")
    else:
        print(f"  [WARN] Expected {expected_total} messages after burst")

    # === Test 4: Verify Final State ===
    print("\n--- Test 4: Final State Verification ---")

    # Skip online users check - presence disabled for debugging

    # Final message count check
    print("\n  Final message counts:")
    for name, client in clients.items():
        try:
            count = client.eval('return get_message_count()')
            status = "[OK]" if count == expected_total else f"[WARN expected {expected_total}]"
            print(f"    {name}: {count} messages {status}")
        except Exception as e:
            print(f"    {name}: [ERROR] {e}")

    # === Test 5: Idle Resource Usage ===
    print("\n--- Test 5: Idle Resource Measurement (10s) ---")
    report.mark("idle_start")

    # Sample resources at idle for 10 seconds (1 sample/sec)
    idle_samples = []
    for i in range(10):
        time.sleep(1)
        report.sample_processes(pids)
        if report.samples:
            idle_samples.append(report.samples[-1])
        if i == 0:
            print("  Sampling idle memory/CPU", end="", flush=True)
        print(".", end="", flush=True)
    print()

    # Calculate idle averages from the last 10 samples
    if idle_samples:
        idle_mem = {}
        idle_cpu = {}
        for name in idle_samples[0].memory_mb:
            mems = [s.memory_mb.get(name, 0) for s in idle_samples if name in s.memory_mb]
            cpus = [s.cpu_percent.get(name, 0) for s in idle_samples if name in s.cpu_percent]
            if mems:
                idle_mem[name] = sum(mems) / len(mems)
            if cpus:
                idle_cpu[name] = sum(cpus) / len(cpus)

        print("\n  Idle Memory (avg over 10s):")
        for name in sorted(idle_mem.keys()):
            print(f"    {name:12s}: {idle_mem[name]:6.0f} MB")

        print("\n  Idle CPU (avg over 10s):")
        for name in sorted(idle_cpu.keys()):
            print(f"    {name:12s}: {idle_cpu[name]:5.1f}%")

        report.latencies['idle_memory_alice_mb'] = round(idle_mem.get('alice', 0), 1)
        report.latencies['idle_memory_node_mb'] = round(idle_mem.get('node', 0), 1)
        report.latencies['idle_cpu_alice_pct'] = round(idle_cpu.get('alice', 0), 1)
        report.latencies['idle_cpu_node_pct'] = round(idle_cpu.get('node', 0), 1)

    report.mark("idle_done")

    # === Summary ===
    report.mark("test_complete")

    # === Flamegraph Analysis (auto-run if .folded files exist) ===
    if args.flame_only or args.profile:
        print("\n--- Flamegraph Analysis ---")
        folded_dir = Path("/tmp/chat_reactive_test")
        folded_files = list(folded_dir.rglob("*.folded"))
        folded_files = [f for f in folded_files if f.stat().st_size > 0]

        if folded_files:
            flame_results = []
            for f in sorted(folded_files):
                result = analyze_flamegraph_file(str(f))
                if "error" not in result:
                    flame_results.append(result)
                    # Print top hotspot summary
                    name = f.stem
                    hotspots = result.get("hotspots", [])[:3]
                    if hotspots:
                        parts = []
                        for h in hotspots:
                            fn = h["function"].split("::")[-1]
                            pct = h["percent"]
                            parts.append(f"{fn} ({pct:.1f}%)")
                        print(f"  {name}: {', '.join(parts)}")

            if flame_results:
                report.flamegraph_analysis = {
                    "instances": flame_results,
                    "folded_files": [str(f) for f in folded_files],
                }
                print(f"  Analyzed {len(flame_results)} instances")
        else:
            print("  No .folded files found")

    print("\n" + "=" * 60)
    print("  Test Summary")
    print("=" * 60)
    print(f"  Total messages sent: {expected_total}")
    print(f"  Regular messages: {total_sent}")
    print(f"  Burst messages: {burst_sent}")
    print(f"  Users tested: {len(clients)}")
    print()
    print("  Reactive binding features tested:")
    print("  - scribe:bind('messages', 'messages') for auto-sync")
    print("  - Messages sync without on_loro_change handlers")
    print("  NOTE: Presence disabled to isolate message sync issues")
    print()

    # Print performance report
    report.print_report()

    # Save to JSON if requested
    if args.output:
        report.save_json(args.output)

    # Heaptrack analysis (auto-run if .zst files exist)
    if args.heaptrack:
        print("\n--- Heaptrack Analysis ---")
        heaptrack_dir = Path("/tmp/chat_reactive_test")
        heaptrack_files = sorted(heaptrack_dir.rglob("*.zst"))
        if heaptrack_files:
            try:
                from analyze_heaptrack import analyze_file as analyze_heap_file, print_analysis as print_heap_analysis
                heap_results = []
                for f in heaptrack_files:
                    result = analyze_heap_file(str(f))
                    print_heap_analysis(result)
                    heap_results.append(result)

                if heap_results:
                    report.latencies['heaptrack_peak_heap_alice'] = heap_results[0].get('peak_heap', '')
                    report.latencies['heaptrack_leaked_alice'] = heap_results[0].get('leaked', '')
            except ImportError:
                # Fallback: just list files
                for f in heaptrack_files:
                    size_mb = f.stat().st_size / (1024 * 1024)
                    print(f"  {f.parent.name}: {f} ({size_mb:.1f} MB)")
                print(f"\n  Analyze with: python scripts/analyze_heaptrack.py {heaptrack_dir}")
        else:
            print("  No heaptrack output found. Is heaptrack installed?")
            print("  Install: sudo pacman -S heaptrack")

    print()
    print("  Attach to tmux to interact: tmux attach -t chat_reactive_test")
    print("  Kill session: tmux kill-session -t chat_reactive_test")
    print()

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nExiting (session still running)")

    return 0


if __name__ == "__main__":
    sys.exit(main() or 0)
