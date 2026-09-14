# AeroFS Runtime Performance Benchmark

This benchmark is designed for the resource-lifecycle question that matters most for a long-lived file manager:

```text
idle baseline -> sustained workload -> workload complete -> idle recovery
```

It records both OS-level process state and AeroFS aggregate runtime state so a run can distinguish allocator/RSS behavior from a real logical resource leak.

## What is sampled

`tools/perf/aerofs_perf.py` is standard-library-only Python and targets Linux hosts. At each sample it records:

- process CPU usage from `/proc/<pid>/stat` (one saturated core = 100%)
- RSS from `/proc/<pid>/status`
- PSS from `/proc/<pid>/smaps_rollup` when readable
- file-descriptor count from `/proc/<pid>/fd`
- thread count
- every numeric leaf exposed by `GET /metrics`
- SQLite main database, WAL, and SHM file sizes when `--db-path` is supplied

The harness writes:

- `metadata.json` — run configuration and workload command
- `samples.jsonl` — append-only raw samples, useful even if a run is interrupted
- `samples.csv` — the same samples for plotting/analysis
- `summary.json` — baseline median, workload peak, recovery-tail median, deltas, ratios, and throughput

The recovery tail is the last 20% of recovery samples. This avoids declaring recovery complete merely because one early post-load sample happened to be low.

## Basic usage

Build and start AeroFS normally, then identify the backend PID and database path. The default server port in the project quickstart is `8080`, so pass that URL explicitly:

```bash
python3 tools/perf/aerofs_perf.py \
  --pid "$(pgrep -n backend)" \
  --base-url http://127.0.0.1:8080 \
  --db-path /path/to/filemanager.db \
  --label idle-smoke \
  --baseline-seconds 60 \
  --recovery-seconds 120 \
  --workload 'sleep 30'
```

The workload is deliberately an arbitrary shell command. That keeps the observer independent from authentication, storage-provider setup, and the particular load generator being tested. A workload can therefore be a shell script, `curl`, `hey`, `wrk`, a sync API driver, or a remote-provider test tool.

For a known payload, add `--workload-bytes` and/or `--workload-files` to calculate throughput:

```bash
python3 tools/perf/aerofs_perf.py \
  --pid "$PID" \
  --base-url http://127.0.0.1:8080 \
  --db-path "$DB" \
  --label transfer-10g \
  --baseline-seconds 120 \
  --recovery-seconds 300 \
  --workload-bytes 10737418240 \
  --workload './tools/my-transfer-load.sh'
```

## Recovery assertions

For repeatable regression runs, `--assert-recovery` turns selected recovery properties into a process exit status. Defaults are intentionally conservative:

- recovery RSS <= 1.25x baseline median
- recovery PSS <= 1.25x baseline median
- recovery FD count <= baseline + 5
- recovery thread count <= baseline + 2
- every `resource_budget.*.in_use` counter returns to its baseline
- `providers.active_leases` returns to its baseline
- every `execution.*` counter returns to its own baseline, including active/queued/running transfers and active/executing/paused sync jobs

Execution assertions compare against the measured baseline rather than hard-coding zero. This allows a benchmark to coexist with unrelated pre-existing paused or queued work while still detecting workload-created state that failed to drain.

Example:

```bash
python3 tools/perf/aerofs_perf.py \
  --pid "$PID" \
  --base-url http://127.0.0.1:8080 \
  --db-path "$DB" \
  --label regression \
  --baseline-seconds 120 \
  --recovery-seconds 300 \
  --assert-recovery \
  --max-rss-ratio 1.20 \
  --max-pss-ratio 1.15 \
  --workload './tools/load.sh'
```

RSS/PSS may remain above baseline because allocator arenas and the kernel page cache are not equivalent to live AeroFS state. A failure in permit, lease, or execution counters is therefore a stronger leak signal than RSS alone.

## Recommended scenario matrix

Run release builds on the same machine, same filesystem, same configuration, and preferably the same data set between revisions.

| ID | Scenario | Suggested baseline / recovery | Primary signals |
| --- | --- | --- | --- |
| A | Idle only / no-op workload | 10 min / 10 min | CPU%, supervised tasks, DB in-use, FD/thread stability |
| B | Single 10 GiB transfer | 2 min / 5 min | throughput, RSS/PSS peak, transfer/global permits, WAL |
| C | Four concurrent 10 GiB transfers | 2 min / 5 min | concurrency bounds, CPU, network/local permits, FD peak |
| D | Sync tree with ~100k files | 2 min / 5 min | RSS/PSS peak, DB/WAL growth, sync/global permits |
| E | 10k small transfers | 2 min / 10 min | terminal-state reclamation, FD/thread recovery, DB growth |
| F | 100k-op sync interrupted and restarted | 2 min / 10 min | recovery memory bound, restart latency, permits/leases |

For scenario A, a workload such as `sleep 600` is useful: it keeps the phase boundary explicit while generating no application work.

## Comparing runs

For each scenario compare at least:

```text
baseline median
workload peak
recovery-tail median
recovery - baseline
recovery / baseline
workload duration + throughput
```

A healthy lifecycle generally has the following shape:

```text
resource_budget.*.in_use    baseline -> elevated -> baseline
providers.active_leases     baseline -> elevated -> baseline
execution.transfer_active   baseline -> elevated -> baseline
execution.transfer_queue_depth baseline -> elevated -> baseline
execution.sync_active       baseline -> elevated -> baseline
metadata_cache.in_flight    baseline -> elevated -> baseline
database_pool.in_use        baseline -> elevated -> baseline
FD/thread count             baseline -> bounded peak -> near baseline
RSS/PSS                     baseline -> bounded peak -> stable post-load plateau
```

`metadata_cache.entries` is not expected to return to zero because retained cache entries are intentional. SQLite/WAL files also do not necessarily shrink immediately after work; they should be interpreted separately from live heap/process state.

## Permissions and portability

The harness is Linux-specific because it reads `/proc`. PSS and FD visibility can be restricted by container namespaces, Android SELinux, `hidepid`, or different users. Missing optional values are omitted and reported in `probe_errors`; the rest of the run continues.

If `/metrics` is unavailable or intentionally blocked, use `--no-metrics`. The OS-level measurements still work.
