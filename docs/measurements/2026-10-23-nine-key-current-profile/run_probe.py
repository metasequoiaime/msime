from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--resources', required=True, type=Path)
args = parser.parse_args()
folder = Path('target/current-nine-key-evidence')
binary = folder / 'paired-system'
expected = json.loads((folder / 'probe-binary.json').read_text())['binary_sha256']
assert hashlib.sha256(binary.read_bytes()).hexdigest() == expected


def observe():
    listing = subprocess.check_output(['ps', '-axo', 'pid=,comm='], text=True)
    compiling = []
    for line in listing.splitlines():
        pid, command = line.strip().split(None, 1)
        name = Path(command).name
        if re.fullmatch(r'cargo|rustc|clang(?:\+\+)?|swift-frontend|cmake|make|ninja|javac|rerank_latency|paired-system', name):
            compiling.append({'pid': int(pid), 'name': name})
    return {'time': time.time(), 'load1': os.getloadavg()[0], 'processes': compiling}


records = []
started = time.monotonic()
quiet_since = None
while True:
    record = observe()
    records.append(record)
    quiet = not record['processes'] and record['load1'] <= 10
    quiet_since = (quiet_since or time.monotonic()) if quiet else None
    if quiet_since is not None and time.monotonic() - quiet_since >= 30:
        break
    if time.monotonic() - started >= 180:
        (folder / 'paired-run.json').write_text(json.dumps({'timed': False, 'preflight': records}, indent=2) + '\n')
        raise SystemExit('空闲启动条件未满足，未执行配对计时')
    time.sleep(1)

monitor = []
with (folder / 'paired.log').open('w') as output:
    child = subprocess.Popen([str(binary.resolve()), 'row_binding_complete_decode_pairs', '--ignored', '--nocapture', '--test-threads=1'],
                             stdout=output, stderr=subprocess.STDOUT,
                             env={**os.environ, 'MSIME_ROW_SYSTEM_ALLOCATOR': '1', 'MSIME_NEURAL_MODEL_DIR': str(args.resources.resolve())})
    while child.poll() is None:
        monitor.append(observe())
        time.sleep(0.25)
    code = child.wait()
assert hashlib.sha256(binary.read_bytes()).hexdigest() == expected
(folder / 'paired-run.json').write_text(json.dumps({'timed': True, 'exit_code': code, 'child_pid': child.pid, 'binary_sha256': expected, 'preflight': records, 'monitor': monitor}, indent=2) + '\n')
raise SystemExit(code)
