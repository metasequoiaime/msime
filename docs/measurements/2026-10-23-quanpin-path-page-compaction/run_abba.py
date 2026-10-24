from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--resources', type=Path, required=True)
args = parser.parse_args()
folder = Path('target/page-compaction-evidence')
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
variants = {name: folder / f'{name}-rerank_latency' for name in ['baseline', 'candidate']}
hashes = {name: sha(path) for name, path in variants.items()}
watched = re.compile(r'cargo|rustc|(?:[\w.]+-)*(?:gcc|g\+\+|cc|c\+\+|ld|lld)|clang(?:\+\+)?(?:-\d+)?|swift(?:-frontend|-driver|-build|-test|c)|cmake|g?make|ninja|javac|ld64\.lld|docker|convert_eval|ninekey_latency|(?:baseline-|candidate-)?rerank_latency|paired-system')


def observe():
    processes = []
    for line in subprocess.check_output(['ps', '-axo', 'pid=,comm='], text=True).splitlines():
        pid, command = line.strip().split(None, 1)
        name = Path(command).name
        if watched.fullmatch(name):
            processes.append({'pid': int(pid), 'name': name})
    return {'at': time.monotonic(), 'load1': os.getloadavg()[0], 'processes': processes}


result = {'timed': False, 'binary_sha256': hashes, 'preflight': [], 'arms': []}
record = folder / 'abba.json'
# 独占创建并保留启动过程，防止中断或重复调用覆盖同一次尝试。
with record.open('x') as output:
    output.write(json.dumps(result, indent=2) + '\n')
started = time.monotonic()
quiet_since = None
while True:
    point = observe()
    result['preflight'].append(point)
    record.write_text(json.dumps(result, indent=2) + '\n')
    quiet = not point['processes'] and point['load1'] <= 10
    quiet_since = (quiet_since or point['at']) if quiet else None
    if quiet_since is not None and point['at'] - quiet_since >= 30:
        break
    if time.monotonic() - started >= 180:
        raise SystemExit('启动条件未满足，没有执行计时')
    time.sleep(1)

result['timed'] = True
for arm, variant in [('A1', 'baseline'), ('B1', 'candidate'), ('B2', 'candidate'), ('A2', 'baseline')]:
    binary = variants[variant]
    assert sha(binary) == hashes[variant]
    monitor = []
    with (folder / f'abba-{arm}.log').open('w') as output:
        child = subprocess.Popen([str(binary.resolve()), '--resources', str(args.resources.resolve()),
                                  '--set', 'resources/eval/sentences-nine-key-v1.tsv', '--grid', 'nine',
                                  '--warmup', '5', '--budget-ms', '16'], stdout=output, stderr=subprocess.STDOUT)
        while child.poll() is None:
            point = observe()
            point['processes'] = [process for process in point['processes'] if process['pid'] != child.pid]
            monitor.append(point)
            time.sleep(0.25)
        code = child.wait()
    assert sha(binary) == hashes[variant]
    result['arms'].append({'arm': arm, 'variant': variant, 'exit_code': code, 'monitor': monitor})
    (folder / 'abba.json').write_text(json.dumps(result, indent=2) + '\n')
    print(arm, 'exit', code, flush=True)
result['no_observed_external_tools'] = all(not point['processes'] for arm in result['arms'] for point in arm['monitor'])
(folder / 'abba.json').write_text(json.dumps(result, indent=2) + '\n')
