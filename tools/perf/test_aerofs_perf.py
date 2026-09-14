import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("aerofs_perf.py")
SPEC = importlib.util.spec_from_file_location("aerofs_perf", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
perf = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(perf)


class ProcParserTests(unittest.TestCase):
    def test_proc_status_extracts_rss_and_threads(self):
        parsed = perf.parse_proc_status(
            "Name:\taerofs\nVmRSS:\t  12345 kB\nThreads:\t17\nVmSize:\t99999 kB\n"
        )
        self.assertEqual(parsed, {"rss_kib": 12345, "threads": 17})

    def test_smaps_rollup_extracts_pss(self):
        parsed = perf.parse_smaps_rollup(
            "00400000-7fffffffffff ---p 00000000 00:00 0 [rollup]\n"
            "Rss: 20000 kB\nPss: 15000 kB\nPrivate_Clean: 1 kB\n"
        )
        self.assertEqual(parsed, {"pss_kib": 15000})

    def test_proc_stat_handles_spaces_in_comm(self):
        # fields after comm: state, ppid..; utime/stime are indexes 11/12.
        suffix = ["S"] + ["0"] * 10 + ["120", "30"] + ["0"] * 20
        text = "123 (aerofs worker) " + " ".join(suffix)
        self.assertEqual(perf.parse_proc_stat_cpu_ticks(text), 150)


class MetricsTests(unittest.TestCase):
    def test_flatten_numeric_ignores_strings_and_booleans(self):
        flattened = perf.flatten_numeric(
            {
                "active_supervised_tasks": 4,
                "providers": {"active_leases": 2, "healthy": True},
                "label": "runtime",
            }
        )
        self.assertEqual(
            flattened,
            {
                "metrics.active_supervised_tasks": 4,
                "metrics.providers.active_leases": 2,
            },
        )

    def test_summary_uses_baseline_median_workload_peak_and_recovery_tail(self):
        samples = [
            {"phase": "baseline", "elapsed_seconds": 0, "rss_kib": 100, "fd_count": 10},
            {"phase": "baseline", "elapsed_seconds": 1, "rss_kib": 110, "fd_count": 10},
            {"phase": "workload", "elapsed_seconds": 2, "rss_kib": 400, "fd_count": 30},
            {"phase": "workload", "elapsed_seconds": 3, "rss_kib": 350, "fd_count": 20},
            {"phase": "recovery", "elapsed_seconds": 4, "rss_kib": 180, "fd_count": 12},
            {"phase": "recovery", "elapsed_seconds": 5, "rss_kib": 120, "fd_count": 10},
            {"phase": "recovery", "elapsed_seconds": 6, "rss_kib": 110, "fd_count": 10},
            {"phase": "recovery", "elapsed_seconds": 7, "rss_kib": 105, "fd_count": 10},
            {"phase": "recovery", "elapsed_seconds": 8, "rss_kib": 105, "fd_count": 10},
        ]
        summary = perf.summarize_samples(samples, workload_seconds=2.0, workload_bytes=2 * 1024 * 1024)
        rss = summary["metrics"]["rss_kib"]
        self.assertEqual(rss["baseline_median"], 105.0)
        self.assertEqual(rss["workload_peak"], 400.0)
        # Recovery tail is the last 20%; for 5 recovery samples that is the last sample.
        self.assertEqual(rss["recovery_tail_median"], 105.0)
        self.assertEqual(summary["throughput"]["mib_per_second"], 1.0)

    def test_recovery_failures_catch_resource_leaks(self):
        summary = {
            "metrics": {
                "rss_kib": {
                    "recovery_to_baseline_ratio": 1.10,
                    "recovery_minus_baseline": 10,
                },
                "fd_count": {
                    "recovery_to_baseline_ratio": 1.0,
                    "recovery_minus_baseline": 0,
                },
                "metrics.resource_budget.transfer.in_use": {
                    "recovery_to_baseline_ratio": None,
                    "recovery_minus_baseline": 1,
                },
                "metrics.providers.active_leases": {
                    "recovery_to_baseline_ratio": None,
                    "recovery_minus_baseline": 2,
                },
            }
        }
        failures = perf.recovery_failures(summary, 1.25, 1.25, 5, 2)
        self.assertEqual(len(failures), 2)
        self.assertTrue(any("resource_budget.transfer.in_use" in value for value in failures))
        self.assertTrue(any("providers.active_leases" in value for value in failures))


if __name__ == "__main__":
    unittest.main()
