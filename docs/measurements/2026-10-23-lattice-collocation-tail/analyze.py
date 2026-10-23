from collections import defaultdict
from pathlib import Path
import json
import statistics

folder = Path("target/collocation-tail-evidence")
run = json.loads((folder / "timed-run.json").read_text())
if not run["timed"]:
    print("未满足空闲条件，没有性能样本")
    raise SystemExit(0)
groups = defaultdict(list)
for line in (folder / "paired.log").read_text().splitlines():
    if line.startswith("tail-pair,"):
        sample = json.loads(line.split(",", 1)[1])
        groups[sample["columns"], sample["width"], sample["mode"]].append(sample)
assert len(groups) == 18
assert all(len(samples) == 21 for samples in groups.values())
assert all({sample["pair"] for sample in samples} == set(range(21)) for samples in groups.values())
unexpected = [
    {"time": observation["time"], "process": process}
    for observation in run["monitor"]
    for process in observation["processes"]
    if process["pid"] != run["child_pid"]
]
summary = []
for (columns, width, mode), samples in sorted(groups.items()):
    summary.append({
        "columns": columns, "width": width, "mode": mode,
        "median_ratio": statistics.median(sample["new_ns"] / sample["old_ns"] for sample in samples),
        "minimum_batch_ns": min(min(sample["old_ns"], sample["new_ns"]) for sample in samples),
        "pairs": len(samples),
    })
result = {
    "timed": True,
    "exit_code": run["exit_code"],
    "unexpected_processes": unexpected,
    "monitor_count": len(run["monitor"]),
    "maximum_monitored_load": max(observation["load1"] for observation in run["monitor"]),
    "all_batches_at_least_5ms": all(item["minimum_batch_ns"] >= 5_000_000 for item in summary),
    "summary": summary,
}
(folder / "timing-summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
