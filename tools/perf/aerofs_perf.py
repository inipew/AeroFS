#!/usr/bin/env python3
"""AeroFS idle -> workload -> idle runtime benchmark harness.

Standard-library only so it can run on production-like Linux hosts without adding
benchmark dependencies to the server. It samples process state from /proc, the
AeroFS /metrics endpoint, and SQLite/WAL file sizes while an arbitrary workload
command runs.
"""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time
from typing import Any, Iterable
from urllib import error as urlerror
from urllib import request as urlrequest


DEFAULT_INTERVAL = 1.0
DEFAULT_BASELINE_SECONDS = 30.0
DEFAULT_RECOVERY_SECONDS = 60.0


def _numeric(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def parse_proc_status(text: str) -> dict[str, int]:
    """Parse the /proc/<pid>/status fields used by the benchmark."""
    result: dict[str, int] = {}
    for line in text.splitlines():
        if ":" not in line:
            continue
        key, raw = line.split(":", 1)
        value = raw.strip().split()
        if not value:
            continue
        if key == "VmRSS":
            result["rss_kib"] = int(value[0])
        elif key == "Threads":
            result["threads"] = int(value[0])
    return result


def parse_smaps_rollup(text: str) -> dict[str, int]:
    """Parse proportional set size from /proc/<pid>/smaps_rollup."""
    for line in text.splitlines():
        if line.startswith("Pss:"):
            return {"pss_kib": int(line.split()[1])}
    return {}


def parse_proc_stat_cpu_ticks(text: str) -> int:
    """Return utime + stime ticks while handling spaces/parentheses in comm."""
    right = text.rfind(")")
    if right < 0:
        raise ValueError("invalid /proc stat: missing process name terminator")
    fields = text[right + 2 :].split()
    # After removing pid + comm, field 3 (state) is index 0. Linux fields 14/15
    # (utime/stime) therefore become indexes 11/12.
    if len(fields) <= 12:
        raise ValueError("invalid /proc stat: too few fields")
    return int(fields[11]) + int(fields[12])


def flatten_numeric(value: Any, prefix: str = "metrics") -> dict[str, float | int]:
    """Flatten only numeric leaves from the aggregate /metrics JSON."""
    flattened: dict[str, float | int] = {}
    if isinstance(value, dict):
        for key, child in value.items():
            child_prefix = f"{prefix}.{key}" if prefix else str(key)
            flattened.update(flatten_numeric(child, child_prefix))
    elif _numeric(value):
        flattened[prefix] = value
    return flattened


def _read_text(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return None


def _file_size(path: Path | None) -> int | None:
    if path is None:
        return None
    try:
        return path.stat().st_size
    except OSError:
        return None


def _median(values: Iterable[float]) -> float | None:
    data = list(values)
    return float(statistics.median(data)) if data else None


def _tail(samples: list[dict[str, Any]]) -> list[dict[str, Any]]:
    if not samples:
        return []
    count = max(1, math.ceil(len(samples) * 0.2))
    return samples[-count:]


def summarize_samples(
    samples: list[dict[str, Any]],
    workload_seconds: float,
    workload_bytes: int | None = None,
    workload_files: int | None = None,
) -> dict[str, Any]:
    """Summarize numeric fields into baseline / peak / recovery-tail values."""
    phases = {
        phase: [sample for sample in samples if sample.get("phase") == phase]
        for phase in ("baseline", "workload", "recovery")
    }
    recovery_tail = _tail(phases["recovery"])

    numeric_keys: set[str] = set()
    for sample in samples:
        numeric_keys.update(key for key, value in sample.items() if _numeric(value))
    numeric_keys.discard("elapsed_seconds")

    metrics: dict[str, Any] = {}
    for key in sorted(numeric_keys):
        baseline = [float(s[key]) for s in phases["baseline"] if _numeric(s.get(key))]
        workload = [float(s[key]) for s in phases["workload"] if _numeric(s.get(key))]
        recovery = [float(s[key]) for s in recovery_tail if _numeric(s.get(key))]
        if not baseline and not workload and not recovery:
            continue

        baseline_median = _median(baseline)
        recovery_median = _median(recovery)
        entry: dict[str, Any] = {
            "baseline_median": baseline_median,
            "workload_peak": max(workload) if workload else None,
            "recovery_tail_median": recovery_median,
        }
        if baseline_median is not None and recovery_median is not None:
            if baseline_median == 0:
                entry["recovery_minus_baseline"] = recovery_median
                entry["recovery_to_baseline_ratio"] = None
            else:
                entry["recovery_minus_baseline"] = recovery_median - baseline_median
                entry["recovery_to_baseline_ratio"] = recovery_median / baseline_median
        metrics[key] = entry

    throughput: dict[str, Any] = {"workload_seconds": workload_seconds}
    if workload_seconds > 0 and workload_bytes is not None:
        throughput["bytes"] = workload_bytes
        throughput["bytes_per_second"] = workload_bytes / workload_seconds
        throughput["mib_per_second"] = workload_bytes / workload_seconds / (1024 * 1024)
    if workload_seconds > 0 and workload_files is not None:
        throughput["files"] = workload_files
        throughput["files_per_second"] = workload_files / workload_seconds

    return {
        "sample_counts": {phase: len(rows) for phase, rows in phases.items()},
        "throughput": throughput,
        "metrics": metrics,
    }


def recovery_failures(
    summary: dict[str, Any],
    max_rss_ratio: float,
    max_pss_ratio: float,
    max_fd_delta: float,
    max_thread_delta: float,
) -> list[str]:
    """Return human-readable recovery invariant failures."""
    metrics = summary.get("metrics", {})
    failures: list[str] = []

    def check_ratio(key: str, maximum: float) -> None:
        entry = metrics.get(key)
        if not entry:
            return
        ratio = entry.get("recovery_to_baseline_ratio")
        if _numeric(ratio) and ratio > maximum:
            failures.append(f"{key} recovery ratio {ratio:.3f} > {maximum:.3f}")

    def check_delta(key: str, maximum: float) -> None:
        entry = metrics.get(key)
        if not entry:
            return
        delta = entry.get("recovery_minus_baseline")
        if _numeric(delta) and delta > maximum:
            failures.append(f"{key} recovery delta {delta:.3f} > {maximum:.3f}")

    check_ratio("rss_kib", max_rss_ratio)
    check_ratio("pss_kib", max_pss_ratio)
    check_delta("fd_count", max_fd_delta)
    check_delta("threads", max_thread_delta)

    # Logical resources are stronger leak signals than allocator/RSS behavior. Every
    # permit, provider lease, and durable execution counter must return to its own
    # pre-workload baseline; a non-zero baseline is valid when unrelated work already exists.
    for key, entry in metrics.items():
        is_resource_permit = key.startswith("metrics.resource_budget.") and key.endswith(".in_use")
        is_provider_lease = key == "metrics.providers.active_leases"
        is_execution_state = key.startswith("metrics.execution.")
        if not (is_resource_permit or is_provider_lease or is_execution_state):
            continue
        delta = entry.get("recovery_minus_baseline")
        if _numeric(delta) and delta > 0:
            failures.append(f"{key} did not return to baseline (+{delta:.3f})")

    return failures


class Probe:
    def __init__(self, pid: int, metrics_url: str | None, db_path: Path | None) -> None:
        self.pid = pid
        self.proc = Path("/proc") / str(pid)
        self.metrics_url = metrics_url
        self.db_path = db_path
        self.clock_ticks = float(os.sysconf("SC_CLK_TCK"))
        self._previous_ticks: int | None = None
        self._previous_time: float | None = None
        self.start = time.monotonic()

    def _process_metrics(self, now: float, errors: list[str]) -> dict[str, Any]:
        if not self.proc.exists():
            raise RuntimeError(f"process {self.pid} no longer exists")

        result: dict[str, Any] = {}
        status = _read_text(self.proc / "status")
        if status is not None:
            result.update(parse_proc_status(status))
        else:
            errors.append("unable to read /proc status")

        smaps = _read_text(self.proc / "smaps_rollup")
        if smaps is not None:
            result.update(parse_smaps_rollup(smaps))

        try:
            result["fd_count"] = len(list((self.proc / "fd").iterdir()))
        except OSError:
            errors.append("unable to count process file descriptors")

        stat = _read_text(self.proc / "stat")
        if stat is not None:
            try:
                ticks = parse_proc_stat_cpu_ticks(stat)
                if self._previous_ticks is not None and self._previous_time is not None:
                    delta_ticks = ticks - self._previous_ticks
                    delta_time = now - self._previous_time
                    if delta_time > 0:
                        # One saturated CPU core is 100%, matching common process tools.
                        result["cpu_percent"] = (delta_ticks / self.clock_ticks) / delta_time * 100.0
                self._previous_ticks = ticks
                self._previous_time = now
            except ValueError as exc:
                errors.append(str(exc))
        return result

    def _runtime_metrics(self, errors: list[str]) -> dict[str, Any]:
        if not self.metrics_url:
            return {}
        try:
            with urlrequest.urlopen(self.metrics_url, timeout=1.5) as response:
                payload = json.loads(response.read().decode("utf-8"))
            return flatten_numeric(payload)
        except (OSError, ValueError, urlerror.URLError) as exc:
            errors.append(f"metrics endpoint: {exc}")
            return {}

    def _database_metrics(self) -> dict[str, Any]:
        if self.db_path is None:
            return {}
        db = self.db_path
        return {
            "db_size_bytes": _file_size(db),
            "wal_size_bytes": _file_size(Path(f"{db}-wal")),
            "shm_size_bytes": _file_size(Path(f"{db}-shm")),
        }

    def sample(self, phase: str) -> dict[str, Any]:
        now = time.monotonic()
        errors: list[str] = []
        sample: dict[str, Any] = {
            "timestamp_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
            "elapsed_seconds": now - self.start,
            "phase": phase,
        }
        sample.update(self._process_metrics(now, errors))
        sample.update(self._runtime_metrics(errors))
        sample.update({key: value for key, value in self._database_metrics().items() if value is not None})
        if errors:
            sample["probe_errors"] = errors
        return sample


def sample_for(
    duration: float,
    phase: str,
    interval: float,
    probe: Probe,
    samples: list[dict[str, Any]],
    jsonl,
) -> None:
    deadline = time.monotonic() + max(0.0, duration)
    first = True
    while first or time.monotonic() < deadline:
        first = False
        sample = probe.sample(phase)
        samples.append(sample)
        jsonl.write(json.dumps(sample, sort_keys=True) + "\n")
        jsonl.flush()
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        time.sleep(min(interval, remaining))


def write_csv(path: Path, samples: list[dict[str, Any]]) -> None:
    keys = sorted({key for sample in samples for key in sample})
    preferred = ["timestamp_utc", "elapsed_seconds", "phase"]
    fields = preferred + [key for key in keys if key not in preferred]
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for sample in samples:
            row = {
                key: json.dumps(value, sort_keys=True) if isinstance(value, (list, dict)) else value
                for key, value in sample.items()
            }
            writer.writerow(row)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True, help="PID of the running AeroFS backend")
    parser.add_argument(
        "--base-url",
        default="http://127.0.0.1:3000",
        help="AeroFS base URL; /metrics is appended (use --no-metrics to disable)",
    )
    parser.add_argument("--no-metrics", action="store_true", help="Do not query the AeroFS /metrics endpoint")
    parser.add_argument("--db-path", type=Path, help="SQLite database path; also samples -wal and -shm")
    parser.add_argument("--label", default="runtime", help="Short run label used in output metadata")
    parser.add_argument("--output-dir", type=Path, help="Result directory (created automatically)")
    parser.add_argument("--sample-interval", type=float, default=DEFAULT_INTERVAL)
    parser.add_argument("--baseline-seconds", type=float, default=DEFAULT_BASELINE_SECONDS)
    parser.add_argument("--recovery-seconds", type=float, default=DEFAULT_RECOVERY_SECONDS)
    parser.add_argument("--workload", required=True, help="Shell command that drives the benchmark workload")
    parser.add_argument("--workload-cwd", type=Path, help="Working directory for the workload command")
    parser.add_argument("--workload-bytes", type=int, help="Known payload bytes for throughput calculation")
    parser.add_argument("--workload-files", type=int, help="Known file count for throughput calculation")
    parser.add_argument("--allow-workload-failure", action="store_true")
    parser.add_argument("--assert-recovery", action="store_true", help="Fail if idle resources do not return near baseline")
    parser.add_argument("--max-rss-ratio", type=float, default=1.25)
    parser.add_argument("--max-pss-ratio", type=float, default=1.25)
    parser.add_argument("--max-fd-delta", type=float, default=5.0)
    parser.add_argument("--max-thread-delta", type=float, default=2.0)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.sample_interval <= 0:
        raise SystemExit("--sample-interval must be > 0")
    if args.baseline_seconds < 0 or args.recovery_seconds < 0:
        raise SystemExit("baseline/recovery durations must be >= 0")

    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    output = args.output_dir or Path("perf-results") / f"{timestamp}-{args.label}"
    output.mkdir(parents=True, exist_ok=True)

    metrics_url = None if args.no_metrics else f"{args.base_url.rstrip('/')}/metrics"
    metadata = {
        "label": args.label,
        "pid": args.pid,
        "metrics_url": metrics_url,
        "db_path": str(args.db_path) if args.db_path else None,
        "sample_interval_seconds": args.sample_interval,
        "baseline_seconds": args.baseline_seconds,
        "recovery_seconds": args.recovery_seconds,
        "workload": args.workload,
        "workload_cwd": str(args.workload_cwd) if args.workload_cwd else None,
        "started_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
    }
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    probe = Probe(args.pid, metrics_url, args.db_path)
    samples: list[dict[str, Any]] = []
    workload: subprocess.Popen[str] | None = None
    workload_started = 0.0
    workload_seconds = 0.0
    workload_code: int | None = None

    print(f"[aerofs-perf] results: {output}")
    with (output / "samples.jsonl").open("w", encoding="utf-8") as jsonl:
        print(f"[aerofs-perf] baseline: {args.baseline_seconds:.1f}s")
        sample_for(args.baseline_seconds, "baseline", args.sample_interval, probe, samples, jsonl)

        print(f"[aerofs-perf] workload: {args.workload}")
        workload_started = time.monotonic()
        workload = subprocess.Popen(
            args.workload,
            cwd=str(args.workload_cwd) if args.workload_cwd else None,
            shell=True,
            text=True,
        )
        try:
            while workload.poll() is None:
                sample = probe.sample("workload")
                samples.append(sample)
                jsonl.write(json.dumps(sample, sort_keys=True) + "\n")
                jsonl.flush()
                time.sleep(args.sample_interval)
            workload_code = workload.returncode
        except KeyboardInterrupt:
            workload.terminate()
            try:
                workload.wait(timeout=5)
            except subprocess.TimeoutExpired:
                workload.kill()
                workload.wait()
            workload_code = workload.returncode
            raise
        finally:
            workload_seconds = max(0.0, time.monotonic() - workload_started)

        # Always capture a workload-boundary sample, including very short commands.
        boundary = probe.sample("workload")
        samples.append(boundary)
        jsonl.write(json.dumps(boundary, sort_keys=True) + "\n")
        jsonl.flush()

        print(f"[aerofs-perf] recovery: {args.recovery_seconds:.1f}s")
        sample_for(args.recovery_seconds, "recovery", args.sample_interval, probe, samples, jsonl)

    write_csv(output / "samples.csv", samples)
    summary = summarize_samples(samples, workload_seconds, args.workload_bytes, args.workload_files)
    summary["workload_exit_code"] = workload_code
    failures = recovery_failures(
        summary,
        args.max_rss_ratio,
        args.max_pss_ratio,
        args.max_fd_delta,
        args.max_thread_delta,
    )
    summary["recovery_failures"] = failures
    (output / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print(f"[aerofs-perf] workload exit={workload_code} duration={workload_seconds:.3f}s")
    for key in ("rss_kib", "pss_kib", "fd_count", "threads", "cpu_percent"):
        entry = summary["metrics"].get(key)
        if entry:
            print(
                f"[aerofs-perf] {key}: baseline={entry['baseline_median']} "
                f"peak={entry['workload_peak']} recovery={entry['recovery_tail_median']}"
            )

    if workload_code not in (0, None) and not args.allow_workload_failure:
        return int(workload_code) if 0 < int(workload_code) < 256 else 1
    if args.assert_recovery and failures:
        for failure in failures:
            print(f"[aerofs-perf] recovery assertion failed: {failure}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())