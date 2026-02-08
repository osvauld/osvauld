#!/usr/bin/env python3
"""Performance reporting for osvauld test scripts.

Provides the PerfReport class for collecting and reporting performance metrics
during test execution, with JSON export for historical comparison.

Usage:
    from osvauld.perf import PerfReport

    report = PerfReport()
    report.mark("setup_complete")

    # ... run tests ...

    report.mark("messages_start")
    # ... send messages ...
    report.mark("messages_done")
    report.message_count = 100

    report.print_report()
    report.save_json("results/run1.json")
"""

import json
import subprocess
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional


@dataclass
class PerfSample:
    """A single performance sample with memory and CPU usage."""
    timestamp: float
    memory_mb: Dict[str, float] = field(default_factory=dict)
    cpu_percent: Dict[str, float] = field(default_factory=dict)


@dataclass
class PerfReport:
    """Collect and report performance metrics.

    Tracks:
    - Timeline events (named timestamps)
    - Process resource samples (memory, CPU)
    - Message throughput
    - Latency metrics
    """

    start_time: float = field(default_factory=time.time)
    events: Dict[str, float] = field(default_factory=dict)
    samples: List[PerfSample] = field(default_factory=list)
    message_count: int = 0
    build_profile: str = "debug"
    latencies: Dict[str, float] = field(default_factory=dict)
    flamegraph_analysis: Optional[dict] = None

    def mark(self, event: str):
        """Record a named event with timestamp."""
        self.events[event] = time.time() - self.start_time

    def sample_processes(self, pids: Dict[str, int]):
        """Sample CPU/memory for processes using ps command."""
        if not pids:
            return

        pid_list = ','.join(str(p) for p in pids.values() if p)
        if not pid_list:
            return

        try:
            result = subprocess.run(
                ['ps', '-o', 'pid=,rss=,pcpu=', '-p', pid_list],
                capture_output=True, text=True, timeout=2
            )

            sample = PerfSample(timestamp=time.time() - self.start_time)

            for line in result.stdout.strip().split('\n'):
                if not line.strip():
                    continue
                parts = line.split()
                if len(parts) >= 3:
                    try:
                        pid = int(parts[0])
                        rss_kb = int(parts[1])
                        cpu = float(parts[2])

                        # Find name for this PID
                        for name, p in pids.items():
                            if p == pid:
                                sample.memory_mb[name] = rss_kb / 1024
                                sample.cpu_percent[name] = cpu
                                break
                    except (ValueError, IndexError):
                        pass

            if sample.memory_mb or sample.cpu_percent:
                self.samples.append(sample)
        except Exception:
            pass  # Silently skip failed samples

    def _calc_throughput(self) -> Optional[float]:
        """Calculate message throughput."""
        if 'messages_start' in self.events and 'messages_done' in self.events:
            duration = self.events['messages_done'] - self.events['messages_start']
            if duration > 0 and self.message_count > 0:
                return self.message_count / duration
        return None

    def _calc_peak_memory(self) -> Dict[str, float]:
        """Calculate peak memory usage per process."""
        peak = {}
        for sample in self.samples:
            for name, mem in sample.memory_mb.items():
                if name not in peak or mem > peak[name]:
                    peak[name] = mem
        return peak

    def _calc_avg_cpu(self) -> Dict[str, float]:
        """Calculate average CPU usage per process."""
        totals: Dict[str, List[float]] = {}
        for sample in self.samples:
            for name, cpu in sample.cpu_percent.items():
                if name not in totals:
                    totals[name] = []
                totals[name].append(cpu)

        return {name: sum(vals) / len(vals) for name, vals in totals.items() if vals}

    def _get_git_commit(self) -> str:
        """Get current git commit hash."""
        try:
            result = subprocess.run(
                ['git', 'rev-parse', '--short', 'HEAD'],
                capture_output=True, text=True, timeout=5
            )
            return result.stdout.strip()
        except Exception:
            return 'unknown'

    def print_report(self):
        """Print formatted performance report to stdout."""
        print("\n" + "=" * 64)
        print("  PERFORMANCE REPORT")
        print("=" * 64)

        # Build info
        print(f"\n  Build: {self.build_profile}")
        print(f"  Git: {self._get_git_commit()}")

        # Timeline
        print("\n  Timeline:")
        prev = 0.0
        for event, t in sorted(self.events.items(), key=lambda x: x[1]):
            print(f"    {t:7.1f}s (+{t - prev:5.1f}s)  {event}")
            prev = t

        # Throughput
        throughput = self._calc_throughput()
        if throughput:
            duration = self.events.get('messages_done', 0) - self.events.get('messages_start', 0)
            print(f"\n  Message Throughput: {throughput:.1f} msg/sec")
            print(f"  Total Messages: {self.message_count}")
            print(f"  Duration: {duration:.1f}s")

        # Memory stats
        peak_memory = self._calc_peak_memory()
        if peak_memory:
            print("\n  Memory Usage (peak):")
            for name in sorted(peak_memory.keys()):
                print(f"    {name:12s}: {peak_memory[name]:6.0f} MB")

        # CPU stats
        avg_cpu = self._calc_avg_cpu()
        if avg_cpu:
            print("\n  CPU Usage (avg):")
            for name in sorted(avg_cpu.keys()):
                print(f"    {name:12s}: {avg_cpu[name]:5.1f}%")

        # Memory growth
        growth = self._calc_memory_growth()
        if growth:
            print("\n  Memory Growth:")
            for name in sorted(growth.keys()):
                g = growth[name]
                per_msg = f" ({g['growth_per_msg_mb']:.2f} MB/msg)" if 'growth_per_msg_mb' in g else ""
                print(f"    {name:12s}: {g['start_mb']:.0f} -> {g['end_mb']:.0f} MB (+{g['growth_mb']:.0f} MB{per_msg})")

        # Latency / idle stats
        if self.latencies:
            print("\n  Latency / Idle Metrics:")
            for name, value in sorted(self.latencies.items()):
                if 'memory' in name or '_mb' in name:
                    print(f"    {name}: {value:.1f} MB")
                elif 'cpu' in name or '_pct' in name:
                    print(f"    {name}: {value:.1f}%")
                else:
                    print(f"    {name}: {value:.1f}ms")

        # Flamegraph summary
        if self.flamegraph_analysis:
            instances = self.flamegraph_analysis.get("instances", [])
            if instances:
                print("\n  Flamegraph CPU Hotspots (top 5, excl. idle):")
                # Pick the first shell instance (most representative)
                inst = instances[0]
                for h in inst.get("hotspots", [])[:5]:
                    fn = h["function"]
                    if len(fn) > 50:
                        fn = "..." + fn[-47:]
                    print(f"    {h['percent']:5.1f}%  {fn}")

        print("\n" + "=" * 64)

    def to_dict(self) -> dict:
        """Convert report to JSON-serializable dict."""
        result = {
            'timestamp': datetime.now().isoformat(),
            'git_commit': self._get_git_commit(),
            'build_profile': self.build_profile,
            'timeline': self.events,
            'message_count': self.message_count,
            'throughput_msg_per_sec': self._calc_throughput(),
            'memory_peak_mb': self._calc_peak_memory(),
            'memory_growth': self._calc_memory_growth(),
            'cpu_avg_percent': self._calc_avg_cpu(),
            'latency': self.latencies,
            'samples': [
                {
                    'time': s.timestamp,
                    'memory_mb': s.memory_mb,
                    'cpu_percent': s.cpu_percent,
                }
                for s in self.samples
            ],
        }
        if self.flamegraph_analysis:
            result['flamegraph'] = self.flamegraph_analysis
        return result

    def _calc_memory_growth(self) -> Dict[str, dict]:
        """Calculate memory growth rate per process.

        Returns per-process dict with:
          start_mb: memory at first sample
          end_mb: memory at last sample
          growth_mb: total growth
          growth_per_msg: MB growth per message (if message_count > 0)
        """
        if len(self.samples) < 2:
            return {}

        first = self.samples[0]
        last = self.samples[-1]
        result = {}

        for name in first.memory_mb:
            if name in last.memory_mb:
                start = first.memory_mb[name]
                end = last.memory_mb[name]
                growth = end - start
                entry = {
                    "start_mb": round(start, 1),
                    "end_mb": round(end, 1),
                    "growth_mb": round(growth, 1),
                }
                if self.message_count > 0:
                    entry["growth_per_msg_mb"] = round(growth / self.message_count, 2)
                result[name] = entry

        return result

    def save_json(self, path: str):
        """Save report to JSON file."""
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)
        print(f"  Results saved to: {path}")

    @staticmethod
    def load_json(path: str) -> dict:
        """Load report from JSON file."""
        with open(path) as f:
            return json.load(f)
