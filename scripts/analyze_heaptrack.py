#!/usr/bin/env python3
"""Analyze heaptrack .zst files from profiling runs.

Parses heaptrack_print output and produces:
- Peak memory breakdown by subsystem
- Leaked memory breakdown by subsystem
- Allocation statistics
- JSON export for historical comparison

Usage:
    # Analyze a single .zst file
    python scripts/analyze_heaptrack.py /tmp/chat_reactive_test/alice/alice.zst

    # Analyze all .zst files in a test run directory
    python scripts/analyze_heaptrack.py /tmp/chat_reactive_test/

    # Save analysis to JSON
    python scripts/analyze_heaptrack.py /tmp/chat_reactive_test/ -o results/heap_analysis.json

    # Compare two analyses
    python scripts/analyze_heaptrack.py --compare results/heap_before.json results/heap_after.json
"""

import json
import re
import subprocess
import sys
import argparse
from collections import defaultdict
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple


# Subsystems to check in heaptrack filter
SUBSYSTEMS = [
    "argon2",
    "loro",
    "slint",
    "femtovg",
    "redb",
    "iroh",
    "mlua",
    "tokio",
    "glutin",
    "fontique",
    "quinn",
    "ractor",
    "serde_json",
    "wayland",
]


def parse_size(s: str) -> float:
    """Parse heaptrack size string (e.g., '67.11M', '331.97K', '0B') to bytes."""
    s = s.strip()
    if s == "0B":
        return 0
    m = re.match(r'([\d.]+)([BKMGT])', s)
    if not m:
        return 0
    val = float(m.group(1))
    unit = m.group(2)
    multipliers = {'B': 1, 'K': 1024, 'M': 1024**2, 'G': 1024**3, 'T': 1024**4}
    return val * multipliers.get(unit, 1)


def format_size(bytes_val: float) -> str:
    """Format bytes to human-readable."""
    if bytes_val >= 1024**2:
        return f"{bytes_val / 1024**2:.2f} MB"
    elif bytes_val >= 1024:
        return f"{bytes_val / 1024:.2f} KB"
    return f"{bytes_val:.0f} B"


def run_heaptrack_print(zst_path: str, extra_args: List[str] = None) -> str:
    """Run heaptrack_print and return stdout."""
    cmd = ["heaptrack_print", "-f", zst_path, "-t"]
    if extra_args:
        cmd.extend(extra_args)
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        return result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        return ""
    except FileNotFoundError:
        print("ERROR: heaptrack_print not found. Install heaptrack.")
        sys.exit(1)


def parse_summary(output: str) -> dict:
    """Parse the summary statistics from heaptrack_print output."""
    summary = {}

    m = re.search(r'total runtime:\s+([\d.]+)s', output)
    if m:
        summary['runtime_sec'] = float(m.group(1))

    m = re.search(r'calls to allocation functions:\s+([\d]+)\s+\((\d+)/s\)', output)
    if m:
        summary['total_allocs'] = int(m.group(1))
        summary['allocs_per_sec'] = int(m.group(2))

    m = re.search(r'temporary memory allocations:\s+([\d]+)\s+\((\d+)/s\)', output)
    if m:
        summary['temp_allocs'] = int(m.group(1))
        summary['temp_per_sec'] = int(m.group(2))

    m = re.search(r'peak heap memory consumption:\s+([\d.]+[BKMGT])', output)
    if m:
        summary['peak_heap_bytes'] = parse_size(m.group(1))
        summary['peak_heap'] = m.group(1)

    m = re.search(r'peak RSS.*?:\s+([\d.]+[BKMGT])', output)
    if m:
        summary['peak_rss_bytes'] = parse_size(m.group(1))
        summary['peak_rss'] = m.group(1)

    m = re.search(r'total memory leaked:\s+([\d.]+[BKMGT])', output)
    if m:
        summary['leaked_bytes'] = parse_size(m.group(1))
        summary['leaked'] = m.group(1)

    return summary


def analyze_subsystem(zst_path: str, subsystem: str) -> dict:
    """Analyze peak and leaked memory for a subsystem using --filter-bt-function."""
    result = {"peak_bytes": 0, "leaked_bytes": 0, "peak_calls": 0, "leaked_calls": 0}

    # Peak
    output = run_heaptrack_print(zst_path, [
        "--filter-bt-function", subsystem, "-p", "-n", "1", "-s", "1"
    ])
    m = re.search(r'(\d+) calls to allocation functions with ([\d.]+[BKMGT]) peak consumption', output)
    if m:
        result['peak_calls'] = int(m.group(1))
        result['peak_bytes'] = parse_size(m.group(2))

    # Leaked
    output = run_heaptrack_print(zst_path, [
        "--filter-bt-function", subsystem, "-l", "-n", "1", "-s", "1"
    ])
    m = re.search(r'([\d.]+[BKMGT]) leaked over (\d+) calls', output)
    if m:
        result['leaked_bytes'] = parse_size(m.group(1))
        result['leaked_calls'] = int(m.group(2))

    return result


def analyze_file(zst_path: str) -> dict:
    """Full analysis of a single heaptrack .zst file."""
    path = Path(zst_path)
    instance_name = path.stem

    print(f"\n  Analyzing {instance_name}...")

    # Get summary
    output = run_heaptrack_print(zst_path, ["-p", "-n", "5", "-s", "1"])
    summary = parse_summary(output)
    summary['instance'] = instance_name
    summary['file'] = str(path)

    # Analyze each subsystem
    subsystems = {}
    for sub in SUBSYSTEMS:
        info = analyze_subsystem(zst_path, sub)
        if info['peak_bytes'] > 0 or info['leaked_bytes'] > 0:
            subsystems[sub] = info

    summary['subsystems'] = subsystems
    return summary


def print_analysis(analysis: dict):
    """Print formatted analysis for a single instance."""
    name = analysis.get('instance', '???')
    print(f"\n  {name}")
    print(f"  {'=' * 60}")
    print(f"  Peak heap: {analysis.get('peak_heap', '?')}")
    print(f"  Peak RSS:  {analysis.get('peak_rss', '?')}")
    print(f"  Leaked:    {analysis.get('leaked', '?')}")
    print(f"  Runtime:   {analysis.get('runtime_sec', 0):.1f}s")
    print(f"  Allocs:    {analysis.get('total_allocs', 0):,} ({analysis.get('allocs_per_sec', 0):,}/s)")

    subs = analysis.get('subsystems', {})
    if subs:
        # Sort by peak descending
        sorted_peak = sorted(subs.items(), key=lambda x: x[1]['peak_bytes'], reverse=True)
        sorted_leak = sorted(subs.items(), key=lambda x: x[1]['leaked_bytes'], reverse=True)

        print(f"\n  Peak Memory by Subsystem:")
        print(f"  {'Subsystem':<16} {'Peak':>12}  {'Calls':>8}")
        print(f"  {'-'*16} {'-'*12}  {'-'*8}")
        for sub, info in sorted_peak:
            if info['peak_bytes'] > 0:
                print(f"  {sub:<16} {format_size(info['peak_bytes']):>12}  {info['peak_calls']:>8}")

        print(f"\n  Leaked Memory by Subsystem:")
        print(f"  {'Subsystem':<16} {'Leaked':>12}  {'Calls':>8}")
        print(f"  {'-'*16} {'-'*12}  {'-'*8}")
        for sub, info in sorted_leak:
            if info['leaked_bytes'] > 0:
                print(f"  {sub:<16} {format_size(info['leaked_bytes']):>12}  {info['leaked_calls']:>8}")


def print_comparison(before: dict, after: dict):
    """Print comparison between two analysis runs."""
    print("\n" + "=" * 64)
    print("  HEAPTRACK COMPARISON")
    print("=" * 64)

    before_instances = {a['instance']: a for a in before.get('instances', [])}
    after_instances = {a['instance']: a for a in after.get('instances', [])}

    common = set(before_instances.keys()) & set(after_instances.keys())

    print(f"\n  Peak Heap Memory:")
    for name in sorted(common):
        b = before_instances[name].get('peak_heap_bytes', 0)
        a = after_instances[name].get('peak_heap_bytes', 0)
        if b > 0:
            pct = (a - b) / b * 100
            verdict = "BETTER" if pct < -5 else ("WORSE" if pct > 5 else "SAME")
            print(f"    {name:<12}: {format_size(b)} -> {format_size(a)} ({pct:+.1f}%) [{verdict}]")

    print(f"\n  Leaked Memory:")
    for name in sorted(common):
        b = before_instances[name].get('leaked_bytes', 0)
        a = after_instances[name].get('leaked_bytes', 0)
        if b > 0:
            pct = (a - b) / b * 100
            verdict = "BETTER" if pct < -5 else ("WORSE" if pct > 5 else "SAME")
            print(f"    {name:<12}: {format_size(b)} -> {format_size(a)} ({pct:+.1f}%) [{verdict}]")

    # Subsystem comparison for first shell instance
    for name in sorted(common):
        if name in ('alice', 'bob', 'carol'):
            b_subs = before_instances[name].get('subsystems', {})
            a_subs = after_instances[name].get('subsystems', {})
            all_subs = set(b_subs.keys()) | set(a_subs.keys())

            print(f"\n  Subsystem Peak ({name}):")
            print(f"  {'Subsystem':<16} {'Before':>12} {'After':>12} {'Change':>10}")
            print(f"  {'-'*16} {'-'*12} {'-'*12} {'-'*10}")
            for sub in sorted(all_subs):
                bp = b_subs.get(sub, {}).get('peak_bytes', 0)
                ap = a_subs.get(sub, {}).get('peak_bytes', 0)
                if bp > 0 or ap > 0:
                    if bp > 0:
                        pct = f"{(ap-bp)/bp*100:+.0f}%"
                    else:
                        pct = "NEW"
                    print(f"  {sub:<16} {format_size(bp):>12} {format_size(ap):>12} {pct:>10}")
            break  # Only show first shell

    print()


def main():
    parser = argparse.ArgumentParser(description="Analyze heaptrack profiling data")
    parser.add_argument("path", nargs="?", help="Path to .zst file or directory containing .zst files")
    parser.add_argument("-o", "--output", help="Save analysis to JSON file")
    parser.add_argument("--compare", nargs=2, metavar=("BEFORE", "AFTER"),
                        help="Compare two JSON analysis files")
    args = parser.parse_args()

    if args.compare:
        with open(args.compare[0]) as f:
            before = json.load(f)
        with open(args.compare[1]) as f:
            after = json.load(f)
        print_comparison(before, after)
        return

    if not args.path:
        parser.print_help()
        return

    path = Path(args.path)

    if path.is_file() and path.suffix == '.zst':
        zst_files = [path]
    elif path.is_dir():
        zst_files = sorted(path.rglob("*.zst"))
    else:
        print(f"ERROR: {path} is not a .zst file or directory")
        return

    if not zst_files:
        print(f"No .zst files found in {path}")
        return

    print("=" * 64)
    print("  HEAPTRACK ANALYSIS")
    print("=" * 64)

    results = []
    for zst in zst_files:
        analysis = analyze_file(str(zst))
        print_analysis(analysis)
        results.append(analysis)

    # Summary table
    print(f"\n  {'=' * 60}")
    print(f"  SUMMARY")
    print(f"  {'Instance':<12} {'Peak Heap':>12} {'Leaked':>12} {'Allocs':>12}")
    print(f"  {'-'*12} {'-'*12} {'-'*12} {'-'*12}")
    for r in results:
        print(f"  {r.get('instance','?'):<12} "
              f"{r.get('peak_heap','?'):>12} "
              f"{r.get('leaked','?'):>12} "
              f"{r.get('total_allocs',0):>12,}")
    print()

    if args.output:
        output_data = {
            'timestamp': datetime.now().isoformat(),
            'instances': results,
        }
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, 'w') as f:
            json.dump(output_data, f, indent=2, default=str)
        print(f"  Results saved to: {args.output}")


if __name__ == "__main__":
    main()
