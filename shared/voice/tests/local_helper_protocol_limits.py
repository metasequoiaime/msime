"""Request-line size limit for the standalone local voice helper."""

import json
import subprocess
import sys


helper = sys.argv[1]
process = subprocess.Popen(
    [helper, "--idle-exit", "5"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True,
)

hello = json.loads(process.stdout.readline())
assert hello["type"] == "hello", hello

padding = "x" * (1024 * 1024)
process.stdin.write(json.dumps({"op": "ping", "id": 1, "padding": padding}) + "\n")
process.stdin.write(json.dumps({"op": "ping", "id": 2}) + "\n")
process.stdin.flush()

oversized = json.loads(process.stdout.readline())
assert oversized == {"type": "error", "message": "request too large"}, oversized
assert json.loads(process.stdout.readline()) == {
    "type": "pong",
    "id": 2,
    "available": hello["available"],
}

process.stdin.close()
assert process.wait(timeout=10) == 0
