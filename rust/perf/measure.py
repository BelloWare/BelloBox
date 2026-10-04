#!/usr/bin/env python3
"""Repeatable process/JSONL telemetry measurements. Never labels CPU time as FPS.

Usage: measure.py --trials 10 --output results.json -- command args...
Child process must exit (e.g. a benchmark or --smoke-test). GUI startup/frames
require explicit app instrumentation and an observed display.
"""
import argparse, json, math, os, platform, statistics, subprocess, tempfile, time, threading
from pathlib import Path


def percentile(values, p):
    if not values:
        return None
    values = sorted(values)
    k = (len(values) - 1) * p
    lo, hi = math.floor(k), math.ceil(k)
    return values[lo] + (values[hi] - values[lo]) * (k - lo)


def summary(values):
    return {"n": len(values), "min": min(values) if values else None,
            "p50": percentile(values, .5), "p95": percentile(values, .95),
            "p99": percentile(values, .99), "max": max(values) if values else None}


def proc_rss_kib(pid):
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1])
    except (OSError, ValueError):
        pass
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--trials", type=int, default=10)
    ap.add_argument("--timeout", type=float, default=30)
    ap.add_argument("--output", required=True)
    ap.add_argument("command", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not 1 <= args.trials <= 10000 or args.timeout <= 0:
        ap.error("provide command, 1..10000 trials and positive timeout")
    runs = []
    for trial in range(args.trials):
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            start = time.perf_counter_ns()
            proc = subprocess.Popen(command, stdout=stdout, stderr=stderr)
            peak_rss = [0]
            stop = threading.Event()
            def sample():
                while not stop.is_set():
                    peak_rss[0] = max(peak_rss[0], proc_rss_kib(proc.pid))
                    stop.wait(.005)
            sampler = threading.Thread(target=sample, daemon=True)
            sampler.start()
            timed_out = False
            # wait() without a timeout uses the OS blocking wait, avoiding the
            # ~5ms sampling cadence rounding short command runtimes upward.
            def expire():
                nonlocal timed_out
                if proc.poll() is None:
                    timed_out = True
                    proc.kill()
            timer = threading.Timer(args.timeout, expire)
            timer.start()
            rc = proc.wait()
            elapsed_ms = (time.perf_counter_ns() - start) / 1e6
            timer.cancel()
            stop.set()
            sampler.join()
            stderr.seek(0)
            runs.append({"trial": trial, "elapsed_ms": elapsed_ms,
                         "sampled_peak_rss_kib": peak_rss[0], "exit_code": rc,
                         "timed_out": timed_out,
                         "stderr_tail": stderr.read()[-2000:].decode(errors="replace")})
    valid = [r for r in runs if r["exit_code"] == 0 and not r["timed_out"]]
    report = {"schema": 1, "command": command, "platform": platform.platform(),
              "cpu_count": os.cpu_count(), "timestamp_utc": time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              "method": "process wall time; single-process RSS sampled every ~5ms; trials include launch overhead; caches not cleared; not UI FPS or input latency",
              "all_passed": len(valid) == len(runs),
              "elapsed_ms": summary([r["elapsed_ms"] for r in valid]),
              "sampled_peak_rss_kib": summary([r["sampled_peak_rss_kib"] for r in valid]),
              "runs": runs}
    dest = Path(args.output)
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k != "runs"}, indent=2))
    raise SystemExit(0 if report["all_passed"] else 1)


if __name__ == "__main__":
    main()
