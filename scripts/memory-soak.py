#!/usr/bin/env python3
"""Read Linux process RSS, descriptors, and thread counts; does not inject input."""
import json
import pathlib
import sys
import time

pid = int(sys.argv[1])
duration = int(sys.argv[2]) if len(sys.argv) > 2 else 60
samples = []
for elapsed in range(0, duration + 1, 2):
    root = pathlib.Path(f"/proc/{pid}")
    try:
        status = dict(line.split(":", 1) for line in (root / "status").read_text().splitlines() if ":" in line)
        sample = {
            "seconds": elapsed,
            "rss_kib": int(status["VmRSS"].split()[0]),
            "threads": int(status["Threads"].strip()),
            "descriptors": len(list((root / "fd").iterdir())),
        }
    except (FileNotFoundError, KeyError):
        sys.exit("Process exited during sampling")
    samples.append(sample)
    print(json.dumps(sample), flush=True)
    if elapsed < duration:
        time.sleep(2)
print(json.dumps({"summary": {
    "initial_rss_kib": samples[0]["rss_kib"],
    "final_rss_kib": samples[-1]["rss_kib"],
    "last_half_rss_range_kib": [min(s["rss_kib"] for s in samples[len(samples)//2:]), max(s["rss_kib"] for s in samples[len(samples)//2:])],
    "descriptor_range": [min(s["descriptors"] for s in samples), max(s["descriptors"] for s in samples)],
    "thread_range": [min(s["threads"] for s in samples), max(s["threads"] for s in samples)],
}}))
