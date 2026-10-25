#!/usr/bin/env python3
"""保留原 CTest 预算与退出码，另存慢原生测试的系统调用栈。"""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time


def main():
    if sys.platform != "darwin" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise SystemExit("此入口只用于一次性的 CI Mac")
    build, reports = (Path(value).resolve() for value in sys.argv[1:])
    reports.mkdir(parents=True, exist_ok=True)
    stopped = threading.Event()

    def monitor():
        observed = {}
        sampled = set()
        targets = (("candidate-row-layout-test", "", 10, 1),
                   ("skin-preview-test", "", 10, 1),
                   ("shortcut-test", " --caps-lock", 0.2, 3))
        while not stopped.wait(0.2):
            for executable, arguments, delay, duration in targets:
                pattern = "^" + re.escape(str(build / executable) + arguments) + "( |$)"
                found = subprocess.run(["pgrep", "-f", pattern], text=True, capture_output=True, timeout=3)
                for value in found.stdout.split():
                    pid = int(value)
                    key = (executable, pid)
                    first = observed.setdefault(key, time.monotonic())
                    if key in sampled or time.monotonic() - first < delay:
                        continue
                    sampled.add(key)
                    destination = reports / f"{executable}-{pid}.sample.txt"
                    try:
                        result = subprocess.run(["sample", str(pid), str(duration), "10", "-file", str(destination)],
                                                text=True, capture_output=True, timeout=duration + 5)
                        status = {"pid": pid, "exit": result.returncode, "stderr": result.stderr}
                    except subprocess.TimeoutExpired:
                        status = {"pid": pid, "error": "sample timeout"}
                    destination.with_suffix(".json").write_text(json.dumps(status, indent=2), encoding="utf-8")

    thread = threading.Thread(target=monitor, daemon=True)
    thread.start()
    try:
        return subprocess.run(["ctest", "--no-tests=error", "--test-dir", str(build),
                               "--output-on-failure", "--timeout", "20"], check=False).returncode
    finally:
        stopped.set()
        thread.join(timeout=9)


if __name__ == "__main__":
    raise SystemExit(main())
