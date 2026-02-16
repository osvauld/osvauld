#!/usr/bin/env python3
"""Compare two performance test results.

Usage:
    python scripts/compare_perf.py results/baseline.json results/after_fix.json

Compares throughput, memory, CPU, and latency metrics between two runs,
highlighting regressions and improvements.
"""

import json
import sys
from pathlib import Path


def compare(before_path: str, after_path: str):
    """Compare two performance result files."""
    before = json.load(open(before_path))
    after = json.load(open(after_path))

    print("=" * 64)
    print("  PERFORMANCE COMPARISON")
    print("=" * 64)
    print(f"\n  Before: {before.get('git_commit', '?')} ({before.get('timestamp', '?')[:10]})")
    print(f"  After:  {after.get('git_commit', '?')} ({after.get('timestamp', '?')[:10]})")
    print(f"  Build:  {before.get('build_profile', '?')} -> {after.get('build_profile', '?')}")

    # Throughput comparison
    tp_before = before.get('throughput_msg_per_sec') or 0
    tp_after = after.get('throughput_msg_per_sec') or 0
    tp_change = ((tp_after - tp_before) / tp_before * 100) if tp_before else 0

    print(f"\n  Throughput:")
    print(f"    Before: {tp_before:.1f} msg/sec")
    print(f"    After:  {tp_after:.1f} msg/sec")
    status = 'BETTER' if tp_change > 5 else ('WORSE' if tp_change < -5 else 'SAME')
    print(f"    Change: {tp_change:+.1f}% [{status}]")

    # Memory comparison
    mem_before = before.get('memory_peak_mb') or {}
    mem_after = after.get('memory_peak_mb') or {}
    all_names = set(mem_before.keys()) | set(mem_after.keys())

    if all_names:
        print(f"\n  Memory (peak):")
        for name in sorted(all_names):
            mb = mem_before.get(name, 0)
            ma = mem_after.get(name, 0)
            if mb > 0:
                change = ((ma - mb) / mb * 100)
                status = 'BETTER' if change < -5 else ('WORSE' if change > 10 else 'SAME')
                print(f"    {name}: {mb:.0f}MB -> {ma:.0f}MB ({change:+.1f}%) [{status}]")
            else:
                print(f"    {name}: N/A -> {ma:.0f}MB")

    # Memory growth comparison
    growth_before = before.get('memory_growth') or {}
    growth_after = after.get('memory_growth') or {}
    all_growth_names = set(growth_before.keys()) | set(growth_after.keys())

    if all_growth_names:
        print(f"\n  Memory Growth Rate:")
        for name in sorted(all_growth_names):
            gb = growth_before.get(name, {})
            ga = growth_after.get(name, {})
            mb = gb.get('growth_per_msg_mb', 0)
            ma = ga.get('growth_per_msg_mb', 0)
            if mb > 0:
                change = ((ma - mb) / mb * 100)
                status = 'BETTER' if change < -5 else ('WORSE' if change > 10 else 'SAME')
                print(f"    {name}: {mb:.2f} -> {ma:.2f} MB/msg ({change:+.1f}%) [{status}]")
            elif ma > 0:
                print(f"    {name}: N/A -> {ma:.2f} MB/msg")

    # CPU comparison
    cpu_before = before.get('cpu_avg_percent') or {}
    cpu_after = after.get('cpu_avg_percent') or {}
    all_names = set(cpu_before.keys()) | set(cpu_after.keys())

    if all_names:
        print(f"\n  CPU (avg):")
        for name in sorted(all_names):
            cb = cpu_before.get(name, 0)
            ca = cpu_after.get(name, 0)
            if cb > 0:
                change = ((ca - cb) / cb * 100)
                status = 'BETTER' if change < -10 else ('WORSE' if change > 20 else 'SAME')
                print(f"    {name}: {cb:.1f}% -> {ca:.1f}% ({change:+.1f}%) [{status}]")
            else:
                print(f"    {name}: N/A -> {ca:.1f}%")

    # Latency comparison
    lat_before = before.get('latency') or {}
    lat_after = after.get('latency') or {}
    all_metrics = set(lat_before.keys()) | set(lat_after.keys())

    if all_metrics:
        print(f"\n  Latency / Idle Metrics:")
        for metric in sorted(all_metrics):
            lb = lat_before.get(metric)
            la = lat_after.get(metric)
            # Determine unit from metric name
            if 'memory' in metric or '_mb' in metric:
                unit = 'MB'
            elif 'cpu' in metric or '_pct' in metric:
                unit = '%'
            else:
                unit = 'ms'
            # For memory/CPU idle metrics, lower is better
            better_lower = unit in ('MB', '%', 'ms')
            if lb and la:
                change = ((la - lb) / lb * 100) if lb != 0 else 0
                if better_lower:
                    status = 'BETTER' if change < -5 else ('WORSE' if change > 10 else 'SAME')
                else:
                    status = 'BETTER' if change > 5 else ('WORSE' if change < -10 else 'SAME')
                print(f"    {metric}: {lb:.1f}{unit} -> {la:.1f}{unit} ({change:+.1f}%) [{status}]")
            elif la:
                print(f"    {metric}: N/A -> {la:.1f}{unit}")

    # Timeline comparison
    timeline_before = before.get('timeline') or {}
    timeline_after = after.get('timeline') or {}

    # Compare total duration
    if timeline_before and timeline_after:
        dur_before = max(timeline_before.values()) if timeline_before else 0
        dur_after = max(timeline_after.values()) if timeline_after else 0
        if dur_before > 0:
            change = ((dur_after - dur_before) / dur_before * 100)
            status = 'BETTER' if change < -5 else ('WORSE' if change > 10 else 'SAME')
            print(f"\n  Total Duration:")
            print(f"    Before: {dur_before:.1f}s")
            print(f"    After:  {dur_after:.1f}s")
            print(f"    Change: {change:+.1f}% [{status}]")

    print("\n" + "=" * 64)

    # Summary verdict
    regressions = 0
    improvements = 0

    if tp_before > 0 and tp_change < -5:
        regressions += 1
    elif tp_before > 0 and tp_change > 5:
        improvements += 1

    for name in all_names:
        mb = mem_before.get(name, 0)
        ma = mem_after.get(name, 0)
        if mb > 0:
            change = ((ma - mb) / mb * 100)
            if change > 10:
                regressions += 1
            elif change < -5:
                improvements += 1

    if regressions > 0:
        print(f"\n  VERDICT: {regressions} regression(s) detected")
        return 1
    elif improvements > 0:
        print(f"\n  VERDICT: {improvements} improvement(s), no regressions")
        return 0
    else:
        print(f"\n  VERDICT: No significant changes")
        return 0


def main():
    if len(sys.argv) != 3:
        print("Usage: compare_perf.py <before.json> <after.json>")
        print("\nCompare two performance test results to detect regressions.")
        sys.exit(1)

    before_path = sys.argv[1]
    after_path = sys.argv[2]

    if not Path(before_path).exists():
        print(f"Error: File not found: {before_path}")
        sys.exit(1)

    if not Path(after_path).exists():
        print(f"Error: File not found: {after_path}")
        sys.exit(1)

    sys.exit(compare(before_path, after_path))


if __name__ == "__main__":
    main()
