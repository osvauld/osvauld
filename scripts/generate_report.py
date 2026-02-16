#!/usr/bin/env python3
"""Generate unified JSON performance report for one benchmark run."""

from __future__ import annotations

import argparse
import json
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Any, Dict


def read_json(path: Path) -> Dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def write_json(path: Path, payload: Dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2)


def git_value(args: list[str], default: str = "unknown") -> str:
    try:
        out = subprocess.run(
            args, capture_output=True, text=True, timeout=5, check=False
        )
        value = out.stdout.strip()
        return value or default
    except Exception:
        return default


def normalize_heap_instance(name: str) -> str:
    # analyze_heaptrack uses Path.stem on *.heaptrack.zst, resulting in "alice.heaptrack".
    return name.replace(".heaptrack", "")


def flame_instance_name(file_path: str) -> str:
    path = Path(file_path)
    return path.name.replace(".folded", "")


def perf_totals(perf: Dict[str, Any]) -> Dict[str, Any]:
    timeline = perf.get("timeline") or {}
    total_duration_sec = max(timeline.values()) if timeline else None
    return {
        "message_count": perf.get("message_count"),
        "throughput_msg_per_sec": perf.get("throughput_msg_per_sec"),
        "total_duration_sec": total_duration_sec,
        "memory_peak_mb": perf.get("memory_peak_mb") or {},
        "memory_growth": perf.get("memory_growth") or {},
        "cpu_avg_percent": perf.get("cpu_avg_percent") or {},
        "build_profile": perf.get("build_profile"),
    }


def heap_totals(heap: Dict[str, Any]) -> Dict[str, Any]:
    instances: Dict[str, Any] = {}
    for item in heap.get("instances", []):
        name = normalize_heap_instance(item.get("instance", "unknown"))
        instances[name] = {
            "peak_heap_bytes": item.get("peak_heap_bytes", 0),
            "peak_rss_bytes": item.get("peak_rss_bytes", 0),
            "leaked_bytes": item.get("leaked_bytes", 0),
            "total_allocs": item.get("total_allocs", 0),
            "subsystems": item.get("subsystems", {}),
        }
    return {"instances": instances}


def flame_totals(flame: Dict[str, Any]) -> Dict[str, Any]:
    instances: Dict[str, Any] = {}
    for item in flame.get("instances", []):
        name = flame_instance_name(item.get("file", ""))
        instances[name] = {
            "total_time": item.get("total_time", 0),
            "non_idle_time": item.get("non_idle_time", 0),
            "categories": item.get("categories", {}),
            "hotspots": item.get("hotspots", []),
            "scribe_breakdown": item.get("scribe_breakdown", []),
            "flush_breakdown": item.get("flush_breakdown", []),
            "loro_breakdown": item.get("loro_breakdown", []),
        }
    return {"instances": instances}


def pct_delta(before: float | int | None, after: float | int | None) -> float | None:
    if before is None or after is None:
        return None
    if before == 0:
        return None
    return ((after - before) / before) * 100.0


def compare_maps(before: Dict[str, Any], after: Dict[str, Any]) -> Dict[str, Any]:
    keys = sorted(set(before.keys()) | set(after.keys()))
    out: Dict[str, Any] = {}
    for key in keys:
        b = before.get(key)
        a = after.get(key)
        if isinstance(b, (int, float)) and isinstance(a, (int, float)):
            out[key] = {
                "before": b,
                "after": a,
                "delta": a - b,
                "pct_delta": pct_delta(b, a),
            }
    return out


def compare_perf(
    baseline_perf: Dict[str, Any], current_perf: Dict[str, Any]
) -> Dict[str, Any]:
    b = perf_totals(baseline_perf)
    c = perf_totals(current_perf)
    return {
        "message_count": {
            "before": b.get("message_count"),
            "after": c.get("message_count"),
        },
        "throughput_msg_per_sec": {
            "before": b.get("throughput_msg_per_sec"),
            "after": c.get("throughput_msg_per_sec"),
            "delta": (c.get("throughput_msg_per_sec") or 0)
            - (b.get("throughput_msg_per_sec") or 0),
            "pct_delta": pct_delta(
                b.get("throughput_msg_per_sec"), c.get("throughput_msg_per_sec")
            ),
        },
        "total_duration_sec": {
            "before": b.get("total_duration_sec"),
            "after": c.get("total_duration_sec"),
            "delta": (c.get("total_duration_sec") or 0)
            - (b.get("total_duration_sec") or 0),
            "pct_delta": pct_delta(
                b.get("total_duration_sec"), c.get("total_duration_sec")
            ),
        },
        "memory_peak_mb": compare_maps(
            b.get("memory_peak_mb", {}), c.get("memory_peak_mb", {})
        ),
        "cpu_avg_percent": compare_maps(
            b.get("cpu_avg_percent", {}), c.get("cpu_avg_percent", {})
        ),
    }


def compare_heap(
    baseline_heap: Dict[str, Any], current_heap: Dict[str, Any]
) -> Dict[str, Any]:
    b_instances = heap_totals(baseline_heap).get("instances", {})
    c_instances = heap_totals(current_heap).get("instances", {})
    names = sorted(set(b_instances.keys()) | set(c_instances.keys()))

    out: Dict[str, Any] = {"instances": {}}
    for name in names:
        b = b_instances.get(name, {})
        c = c_instances.get(name, {})
        out["instances"][name] = {
            "peak_heap_bytes": {
                "before": b.get("peak_heap_bytes", 0),
                "after": c.get("peak_heap_bytes", 0),
                "delta": c.get("peak_heap_bytes", 0) - b.get("peak_heap_bytes", 0),
                "pct_delta": pct_delta(
                    b.get("peak_heap_bytes", 0), c.get("peak_heap_bytes", 0)
                ),
            },
            "leaked_bytes": {
                "before": b.get("leaked_bytes", 0),
                "after": c.get("leaked_bytes", 0),
                "delta": c.get("leaked_bytes", 0) - b.get("leaked_bytes", 0),
                "pct_delta": pct_delta(
                    b.get("leaked_bytes", 0), c.get("leaked_bytes", 0)
                ),
            },
            "total_allocs": {
                "before": b.get("total_allocs", 0),
                "after": c.get("total_allocs", 0),
                "delta": c.get("total_allocs", 0) - b.get("total_allocs", 0),
                "pct_delta": pct_delta(
                    b.get("total_allocs", 0), c.get("total_allocs", 0)
                ),
            },
        }
    return out


def compare_flame(
    baseline_flame: Dict[str, Any], current_flame: Dict[str, Any]
) -> Dict[str, Any]:
    b_instances = flame_totals(baseline_flame).get("instances", {})
    c_instances = flame_totals(current_flame).get("instances", {})
    names = sorted(set(b_instances.keys()) | set(c_instances.keys()))

    out: Dict[str, Any] = {"instances": {}}
    for name in names:
        b = b_instances.get(name, {})
        c = c_instances.get(name, {})

        b_cats = {
            k: v.get("percent", 0) for k, v in (b.get("categories") or {}).items()
        }
        c_cats = {
            k: v.get("percent", 0) for k, v in (c.get("categories") or {}).items()
        }
        out["instances"][name] = {
            "non_idle_time": {
                "before": b.get("non_idle_time", 0),
                "after": c.get("non_idle_time", 0),
                "delta": c.get("non_idle_time", 0) - b.get("non_idle_time", 0),
                "pct_delta": pct_delta(
                    b.get("non_idle_time", 0), c.get("non_idle_time", 0)
                ),
            },
            "category_percent": compare_maps(b_cats, c_cats),
        }
    return out


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate consolidated benchmark JSON report"
    )
    parser.add_argument(
        "--run-dir",
        required=True,
        help="Directory containing perf/heap/flame JSON files",
    )
    parser.add_argument(
        "--messages", type=int, required=True, help="Message count used for this run"
    )
    parser.add_argument(
        "--baseline-dir", help="Optional baseline directory for comparison"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    run_dir = Path(args.run_dir)
    perf_path = run_dir / "perf_release.json"
    heap_path = run_dir / "heap_analysis.json"
    flame_path = run_dir / "flame_analysis.json"

    if not perf_path.exists() or not heap_path.exists() or not flame_path.exists():
        missing = [str(p) for p in [perf_path, heap_path, flame_path] if not p.exists()]
        raise SystemExit(f"Missing required JSON files: {', '.join(missing)}")

    perf = read_json(perf_path)
    heap = read_json(heap_path)
    flame = read_json(flame_path)

    metadata = {
        "generated_at": datetime.now().isoformat(),
        "git_commit": git_value(["git", "rev-parse", "--short", "HEAD"]),
        "git_branch": git_value(["git", "rev-parse", "--abbrev-ref", "HEAD"]),
        "message_count": args.messages,
        "run_dir": str(run_dir),
    }

    report: Dict[str, Any] = {
        "metadata": metadata,
        "perf": perf_totals(perf),
        "heap": heap_totals(heap),
        "flame": flame_totals(flame),
        "sources": {
            "perf_release": str(perf_path),
            "heap_analysis": str(heap_path),
            "flame_analysis": str(flame_path),
        },
    }

    if args.baseline_dir:
        baseline_dir = Path(args.baseline_dir)
        b_perf = read_json(baseline_dir / "perf_release.json")
        b_heap = read_json(baseline_dir / "heap_analysis.json")
        b_flame = read_json(baseline_dir / "flame_analysis.json")

        comparison = {
            "baseline_dir": str(baseline_dir),
            "perf": compare_perf(b_perf, perf),
            "heap": compare_heap(b_heap, heap),
            "flame": compare_flame(b_flame, flame),
        }
        report["comparison"] = comparison
        write_json(run_dir / "comparison.json", comparison)

    write_json(run_dir / "report.json", report)
    print(f"Wrote report: {run_dir / 'report.json'}")
    if "comparison" in report:
        print(f"Wrote comparison: {run_dir / 'comparison.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
