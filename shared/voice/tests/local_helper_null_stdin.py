"""stdin 是 /dev/null 时，助手读到 EOF 就要退出，不能等到空闲超时。"""

import json
import subprocess
import sys
import time


helper = sys.argv[1]
started = time.monotonic()
process = subprocess.Popen(
    [helper, "--idle-exit", "60"], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE
)
hello = json.loads(process.stdout.readline())
assert hello["type"] == "hello", hello
# macOS 的 poll() 对 /dev/null 只回 POLLNVAL，没处理时助手会空转到 --idle-exit 的 60 秒。
assert process.wait(timeout=10) == 0
assert time.monotonic() - started < 10
