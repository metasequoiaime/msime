"""Malformed JSON fields must not terminate the local voice helper."""

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

# A valid JSON object with a value of the wrong type used to escape the
# reader thread's nlohmann::json exception and abort the helper process.
process.stdin.write('{"op":null}\n')
process.stdin.write('{"op":"ping","id":1}\n')
process.stdin.flush()

malformed = json.loads(process.stdout.readline())
assert malformed == {"type": "error", "message": "malformed request"}, malformed
assert json.loads(process.stdout.readline()) == {
    "type": "pong",
    "id": 1,
    "available": hello["available"],
}

process.stdin.close()
assert process.wait(timeout=10) == 0
