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
folder.mkdir(parents=True, exist_ok=True)
binary = Path('target/release/examples/rerank_latency').resolve()
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
lock = json.loads(Path('resources/desktop-dictionary.lock.json').read_text())
for entry in lock['artifacts']:
    resource = args.resources / entry['name']
    assert resource.stat().st_size == entry['size']
    assert sha(resource) == entry['sha256'], entry['name']


def observe():
    listing = subprocess.check_output(['ps', '-axo', 'pid=,comm='], text=True)
    compiling = []
    for line in listing.splitlines():
        pid, command = line.strip().split(None, 1)
        name = Path(command).name
        if re.fullmatch(r'cargo|rustc|clang(?:\+\+)?|swift-frontend|cmake|make|ninja|javac', name):
            compiling.append({'pid': int(pid), 'name': name})
    return {'time': time.time(), 'load1': os.getloadavg()[0], 'compiling': compiling}


binary_hash = sha(binary)
records = [observe()]
command = [str(binary), '--resources', str(args.resources.resolve()), '--set',
           'resources/eval/sentences-nine-key-v1.tsv', '--grid', 'nine',
           '--warmup', '5', '--budget-ms', '16']
with (folder / 'profile-run.log').open('w') as output, (folder / 'sample.log').open('w') as sample_output:
    child = subprocess.Popen(command, stdout=output, stderr=subprocess.STDOUT)
    sampler = subprocess.Popen(['sample', str(child.pid), '20', '1', '-file',
                               str((folder / 'profile.sample.txt').resolve())],
                              stdout=sample_output, stderr=subprocess.STDOUT)
    while child.poll() is None or sampler.poll() is None:
        records.append(observe())
        time.sleep(0.25)
    code = child.wait()
    sample_code = sampler.wait()
assert sha(binary) == binary_hash
report = {'diagnostic_only': True, 'host_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
          'binary_sha256': binary_hash, 'child_pid': child.pid, 'exit_code': code,
          'sample_exit_code': sample_code, 'observations': records}
(folder / 'profile-run.json').write_text(json.dumps(report, indent=2) + '\n')
assert (folder / 'profile.sample.txt').exists()
print(json.dumps({key: value for key, value in report.items() if key != 'observations'}, indent=2))
