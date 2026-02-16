#!/usr/bin/env python3
"""Analyze .folded flamegraph files from profiling runs.

Parses tracing-flame .folded output and produces:
- Category breakdown (iroh, scribe, courier, butler, loro, gurkha)
- Application-level hotspots (excluding networking idle loops)
- Drill-down into scribe actor internals
- Optional SVG flamegraph generation via inferno-flamegraph
- JSON export for historical comparison

Usage:
    # Analyze a single .folded file
    python scripts/analyze_flamegraph.py /tmp/chat_reactive_test/node/node.folded

    # Analyze all .folded files in a test run directory
    python scripts/analyze_flamegraph.py /tmp/chat_reactive_test/

    # Save analysis to JSON + generate SVGs
    python scripts/analyze_flamegraph.py /tmp/chat_reactive_test/ -o results/flame_analysis.json --svg

    # Compare two analyses
    python scripts/analyze_flamegraph.py --compare results/flame_before.json results/flame_after.json
"""

import json
import shutil
import subprocess
import sys
import argparse
from collections import defaultdict
from datetime import datetime
from pathlib import Path


# Idle networking loops that dominate CPU but aren't application work
IDLE_PATTERNS = [
    "iroh::socket::actor",
    "iroh::socket::transports::relay::relay-actor",
    "portmapper::portmapper.service",
    "iroh::net_report::reportgen::reportgen-actor",
]

# Category mapping: prefix -> category name
CATEGORIES = [
    ("iroh::", "iroh"),
    ("portmapper", "portmapper"),
    ("scribe::", "scribe"),
    ("courier::", "courier"),
    ("butler::", "butler"),
    ("loro_internal::", "loro"),
    ("gurkha::", "gurkha"),
    ("ractor::", "ractor"),
    ("herald::", "herald"),
]


def parse_folded(path: str) -> list:
    """Parse a .folded file into (stack, count) tuples."""
    stacks = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            # Format: "frame1;frame2;frame3 count"
            parts = line.rsplit(" ", 1)
            if len(parts) == 2:
                try:
                    stacks.append((parts[0], int(parts[1])))
                except ValueError:
                    continue
    return stacks


def clean_frame(frame: str) -> str:
    """Extract function path from a frame, removing file paths."""
    # Frames look like: "module::func:path/to/file.rs:line"
    # We want just "module::func"
    # Find the first / which starts the file path
    slash_idx = frame.find("/")
    if slash_idx > 0:
        result = frame[:slash_idx].rstrip(":")
        return result
    return frame


def get_leaf(stack: str) -> str:
    """Get the deepest (leaf) frame from a stack."""
    frames = stack.split(";")
    return clean_frame(frames[-1])


def get_child_of(stack: str, parent_pattern: str) -> str | None:
    """Get the frame immediately after a parent pattern in the stack."""
    frames = stack.split(";")
    for i, frame in enumerate(frames):
        if parent_pattern in frame:
            if i + 1 < len(frames):
                return clean_frame(frames[i + 1])
            return None
    return None


def is_multi_frame(stack: str) -> bool:
    """Check if stack has more than just a thread name."""
    return ";" in stack


def matches_idle(stack: str) -> bool:
    """Check if this stack is an idle networking loop."""
    leaf = get_leaf(stack)
    return any(p in leaf for p in IDLE_PATTERNS)


def categorize_stack(stack: str) -> list:
    """Return all categories a stack belongs to."""
    cats = []
    for prefix, name in CATEGORIES:
        if prefix in stack:
            cats.append(name)
    return cats


def analyze_file(path: str) -> dict:
    """Analyze a single .folded file and return results dict."""
    stacks = parse_folded(path)
    if not stacks:
        return {"error": f"No stacks found in {path}"}

    # Multi-frame stacks only (exclude bare thread entries)
    app_stacks = [(s, c) for s, c in stacks if is_multi_frame(s)]

    total_time = sum(c for _, c in app_stacks)
    if total_time == 0:
        return {"error": f"No application stacks in {path}"}

    # 1. Category breakdown
    category_time = defaultdict(int)
    for stack, count in app_stacks:
        cats = categorize_stack(stack)
        for cat in cats:
            category_time[cat] += count
        if not cats:
            category_time["other"] += count

    # 2. Application hotspots (excluding idle loops)
    non_idle = [(s, c) for s, c in app_stacks if not matches_idle(s)]
    non_idle_total = sum(c for _, c in non_idle)

    leaf_time = defaultdict(int)
    for stack, count in non_idle:
        leaf = get_leaf(stack)
        leaf_time[leaf] += count

    hotspots = sorted(leaf_time.items(), key=lambda x: -x[1])[:25]

    # 3. Scribe actor drill-down
    scribe_children = defaultdict(int)
    for stack, count in app_stacks:
        child = get_child_of(stack, "scribe::actor::handle")
        if child is not None:
            scribe_children[child] += count
        elif "scribe::actor::handle" in stack and get_leaf(stack).startswith("scribe::actor::handle"):
            scribe_children["(actor overhead)"] += count

    # 4. handle_flush drill-down
    flush_children = defaultdict(int)
    for stack, count in app_stacks:
        child = get_child_of(stack, "handle_flush")
        if child is not None:
            flush_children[child] += count

    # 5. Loro breakdown
    loro_funcs = defaultdict(int)
    for stack, count in non_idle:
        leaf = get_leaf(stack)
        if "loro_internal" in leaf:
            loro_funcs[leaf] += count

    return {
        "file": str(path),
        "total_stacks": len(stacks),
        "app_stacks": len(app_stacks),
        "total_time": total_time,
        "non_idle_time": non_idle_total,
        "categories": {
            cat: {"time": t, "percent": round(t / total_time * 100, 1)}
            for cat, t in sorted(category_time.items(), key=lambda x: -x[1])
        },
        "hotspots": [
            {
                "function": fn,
                "time": t,
                "percent": round(t / non_idle_total * 100, 1) if non_idle_total else 0,
            }
            for fn, t in hotspots
        ],
        "scribe_breakdown": [
            {
                "function": fn,
                "time": t,
                "percent": round(t / sum(scribe_children.values()) * 100, 1) if scribe_children else 0,
            }
            for fn, t in sorted(scribe_children.items(), key=lambda x: -x[1])
        ],
        "flush_breakdown": [
            {
                "function": fn,
                "time": t,
                "percent": round(t / sum(flush_children.values()) * 100, 1) if flush_children else 0,
            }
            for fn, t in sorted(flush_children.items(), key=lambda x: -x[1])
        ],
        "loro_breakdown": [
            {
                "function": fn,
                "time": t,
                "percent": round(t / non_idle_total * 100, 1) if non_idle_total else 0,
            }
            for fn, t in sorted(loro_funcs.items(), key=lambda x: -x[1])
        ],
    }


def print_analysis(name: str, result: dict):
    """Print formatted analysis for one instance."""
    if "error" in result:
        print(f"\n  [{name}] {result['error']}")
        return

    print(f"\n{'=' * 64}")
    print(f"  {name.upper()}")
    print(f"{'=' * 64}")
    print(f"  Stacks: {result['app_stacks']:,} application ({result['total_stacks']:,} total)")

    # Category breakdown
    print(f"\n  Category Breakdown:")
    for cat, data in result["categories"].items():
        bar = "#" * max(1, int(data["percent"] / 2))
        print(f"    {cat:12s}  {data['percent']:5.1f}%  {bar}")

    # Application hotspots
    print(f"\n  Application Hotspots (excluding idle loops):")
    for i, h in enumerate(result["hotspots"][:15]):
        fn = h["function"]
        # Shorten long function names
        if len(fn) > 55:
            fn = "..." + fn[-52:]
        print(f"    {h['percent']:5.1f}%  {fn}")

    # Scribe breakdown
    if result["scribe_breakdown"]:
        print(f"\n  Scribe Actor Internals:")
        for s in result["scribe_breakdown"]:
            fn = s["function"]
            if len(fn) > 55:
                fn = "..." + fn[-52:]
            print(f"    {s['percent']:5.1f}%  {fn}")

    # Flush breakdown
    if result["flush_breakdown"]:
        print(f"\n  handle_flush Breakdown:")
        for f in result["flush_breakdown"]:
            fn = f["function"]
            if len(fn) > 55:
                fn = "..." + fn[-52:]
            print(f"    {f['percent']:5.1f}%  {fn}")

    # Loro breakdown
    if result["loro_breakdown"]:
        print(f"\n  Loro CRDT Breakdown:")
        for l in result["loro_breakdown"]:
            fn = l["function"]
            if len(fn) > 55:
                fn = "..." + fn[-52:]
            print(f"    {l['percent']:5.1f}%  {fn}")


def generate_svgs(folded_files: dict, output_dir: Path):
    """Generate SVG flamegraphs using inferno-flamegraph."""
    if not shutil.which("inferno-flamegraph"):
        print("\n  [WARN] inferno-flamegraph not found. Install: cargo install inferno")
        return {}

    output_dir.mkdir(parents=True, exist_ok=True)
    svg_paths = {}

    for name, path in folded_files.items():
        svg_path = output_dir / f"{name}_flame.svg"
        try:
            result = subprocess.run(
                ["inferno-flamegraph", str(path)],
                capture_output=True, timeout=30,
            )
            if result.returncode == 0:
                svg_path.write_bytes(result.stdout)
                svg_paths[name] = str(svg_path)
                print(f"  SVG: {svg_path}")
            else:
                print(f"  [WARN] Failed to generate SVG for {name}: {result.stderr.decode()[:100]}")
        except Exception as e:
            print(f"  [WARN] SVG generation failed for {name}: {e}")

    return svg_paths


def compare_analyses(before_path: str, after_path: str):
    """Compare two flame analysis JSON files."""
    before = json.load(open(before_path))
    after = json.load(open(after_path))

    print("=" * 64)
    print("  FLAMEGRAPH COMPARISON")
    print("=" * 64)
    print(f"\n  Before: {before.get('git_commit', '?')} ({before.get('timestamp', '?')[:10]})")
    print(f"  After:  {after.get('git_commit', '?')} ({after.get('timestamp', '?')[:10]})")

    # Compare each instance that appears in both
    before_instances = {r["file"].split("/")[-1].replace(".folded", ""): r for r in before.get("instances", [])}
    after_instances = {r["file"].split("/")[-1].replace(".folded", ""): r for r in after.get("instances", [])}

    for name in sorted(set(before_instances.keys()) & set(after_instances.keys())):
        b = before_instances[name]
        a = after_instances[name]

        print(f"\n  --- {name.upper()} ---")

        # Compare top hotspots
        b_hotspots = {h["function"]: h for h in b.get("hotspots", [])}
        a_hotspots = {h["function"]: h for h in a.get("hotspots", [])}

        all_fns = list(dict.fromkeys(
            [h["function"] for h in a.get("hotspots", [])[:10]] +
            [h["function"] for h in b.get("hotspots", [])[:10]]
        ))

        print(f"  {'Function':<45s}  {'Before':>7s}  {'After':>7s}  {'Change':>8s}")
        for fn in all_fns[:15]:
            bp = b_hotspots.get(fn, {}).get("percent", 0)
            ap = a_hotspots.get(fn, {}).get("percent", 0)
            diff = ap - bp
            arrow = "+" if diff > 0.5 else ("-" if diff < -0.5 else " ")
            short_fn = fn if len(fn) <= 45 else "..." + fn[-42:]
            print(f"  {short_fn:<45s}  {bp:6.1f}%  {ap:6.1f}%  {arrow}{abs(diff):5.1f}%")

    print("\n" + "=" * 64)


def get_git_commit() -> str:
    """Get current git commit hash."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, timeout=5,
        )
        return result.stdout.strip()
    except Exception:
        return "unknown"


def main():
    parser = argparse.ArgumentParser(description="Analyze flamegraph .folded files")
    parser.add_argument("path", nargs="?",
                        help="Path to .folded file or directory containing them")
    parser.add_argument("-o", "--output", type=str,
                        help="Save analysis to JSON file")
    parser.add_argument("--svg", action="store_true",
                        help="Generate SVG flamegraphs (requires inferno)")
    parser.add_argument("--svg-dir", type=str, default="results",
                        help="Directory for SVG output (default: results)")
    parser.add_argument("--compare", nargs=2, metavar=("BEFORE", "AFTER"),
                        help="Compare two analysis JSON files")
    args = parser.parse_args()

    if args.compare:
        compare_analyses(args.compare[0], args.compare[1])
        return 0

    if not args.path:
        parser.print_help()
        return 1

    path = Path(args.path)

    # Collect .folded files
    folded_files = {}
    if path.is_file() and path.suffix == ".folded":
        name = path.stem
        folded_files[name] = path
    elif path.is_dir():
        # Look for .folded files in subdirectories (test run layout)
        for f in sorted(path.rglob("*.folded")):
            if f.stat().st_size > 0:
                name = f.stem
                folded_files[name] = f
    else:
        print(f"Error: {path} is not a .folded file or directory")
        return 1

    if not folded_files:
        print(f"Error: No .folded files found in {path}")
        return 1

    print("=" * 64)
    print("  FLAMEGRAPH ANALYSIS")
    print("=" * 64)
    print(f"  Files: {', '.join(folded_files.keys())}")
    print(f"  Git: {get_git_commit()}")

    # Analyze each file
    results = {}
    for name, fpath in folded_files.items():
        results[name] = analyze_file(str(fpath))
        print_analysis(name, results[name])

    # Generate SVGs
    svg_paths = {}
    if args.svg:
        print(f"\n  Generating SVGs...")
        svg_paths = generate_svgs(folded_files, Path(args.svg_dir))

    # Save JSON
    if args.output:
        report = {
            "timestamp": datetime.now().isoformat(),
            "git_commit": get_git_commit(),
            "instances": list(results.values()),
            "svg_paths": svg_paths,
        }
        Path(args.output).parent.mkdir(parents=True, exist_ok=True)
        with open(args.output, "w") as f:
            json.dump(report, f, indent=2)
        print(f"\n  Analysis saved to: {args.output}")

    print()
    return 0


if __name__ == "__main__":
    sys.exit(main() or 0)
