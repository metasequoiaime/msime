from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import time

folder = Path("target/collocation-tail-evidence")
binary = folder / "paired-system"
expected = json.loads((folder / "system-binary.json").read_text())["sha256"]
assert hashlib.sha256(binary.read_bytes()).hexdigest() == expected

def observe():
    result = subprocess.run(["ps", "-axo", "pid=,comm="], capture_output=True, text=True, check=True)
    matches = []
    for line in result.stdout.splitlines():
        pid, command = line.strip().split(None, 1)
        # 只保留工具名与 pid，不记录其它进程参数。
        name = Path(command).name
        if re.fullmatch(r"cargo|rustc|clang(?:\+\+)?|cc1|swift-frontend|cmake|make|ninja|rerank_latency|paired-system", name):
            matches.append({"pid": int(pid), "name": name})
    return {"time": time.time(), "load1": os.getloadavg()[0], "processes": matches}

records = []
started = time.monotonic()
quiet_since = None
while True:
    record = observe()
    records.append(record)
    quiet = not record["processes"] and record["load1"] <= 10
    quiet_since = (quiet_since or time.monotonic()) if quiet else None
    if quiet_since is not None and time.monotonic() - quiet_since >= 30:
        break
    if time.monotonic() - started >= 180:
        (folder / "timed-run.json").write_text(json.dumps({"timed": False, "preflight": records}, indent=2) + "\n")
        raise SystemExit("空闲启动条件未满足，未运行计时")
    time.sleep(1)

command = [str(binary.resolve()), "collocation_tail_complete_decode_pairs", "--ignored", "--nocapture", "--test-threads=1"]
monitor = []
with (folder / "paired.log").open("w") as output:
    child = subprocess.Popen(command, env={**os.environ, "MSIME_TAIL_SYSTEM_ALLOCATOR": "1"}, stdout=output, stderr=subprocess.STDOUT)
    while child.poll() is None:
        monitor.append(observe())
        time.sleep(0.25)
    code = child.wait()
assert hashlib.sha256(binary.read_bytes()).hexdigest() == expected
(folder / "timed-run.json").write_text(json.dumps({"timed": True, "exit_code": code, "child_pid": child.pid, "preflight": records, "monitor": monitor, "binary_sha256": expected}, indent=2) + "\n")
raise SystemExit(code)
