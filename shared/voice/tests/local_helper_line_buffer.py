"""An oversized line without a newline must not grow the helper's buffer."""

import json
import resource
import subprocess
import sys


helper = sys.argv[1]
process = subprocess.Popen(
    [helper, "--idle-exit", "5"], stdin=subprocess.PIPE, stdout=subprocess.PIPE
)
hello = json.loads(process.stdout.readline())
assert hello["type"] == "hello", hello

# Cross the line limit, then keep the newline away while the reader consumes
# 32 MiB. The discarded tail must not remain in memory.
process.stdin.write(b"x" * (1024 * 1024 + 1))
process.stdin.flush()
for _ in range(31):
    process.stdin.write(b"x" * 1024 * 1024)
    process.stdin.flush()
process.stdin.write(b"\n{\"op\":\"ping\",\"id\":1}\n")
process.stdin.close()
output = process.stdout.read().splitlines()
assert process.wait(timeout=10) == 0

assert json.loads(output[0]) == {"type": "error", "message": "request too large"}, output
assert json.loads(output[1]) == {"type": "pong", "id": 1, "available": hello["available"]}, output

usage = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
if sys.platform == "darwin":
    usage_bytes = usage
else:
    usage_bytes = usage * 1024
assert usage_bytes < 20 * 1024 * 1024, usage_bytes
