import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--variant', required=True, choices=['baseline', 'candidate', 'candidate-32'])
parser.add_argument('--resources', required=True, type=Path)
parser.add_argument('--output-dir', type=Path, default=Path('target/unified-lattice-evidence'))
parser.add_argument('--quality', action='store_true')
parser.add_argument('--latency', action='store_true')
parser.add_argument('--long-latency', action='store_true')
parser.add_argument('--words', action='store_true')
args = parser.parse_args()
folder = args.output_dir.resolve()
folder.mkdir(parents=True, exist_ok=True)
resources = args.resources.resolve()
record_path = folder / (args.variant + '-runs.json')
records = json.loads(record_path.read_text()) if record_path.exists() else []

def run(label, command):
    path = folder / (args.variant + '-' + label + '.log')
    monitor = []
    started = time.monotonic()
    with path.open('w') as log:
        process = subprocess.Popen([str(value) for value in command], stdout=log, stderr=subprocess.STDOUT)
        while True:
            names = subprocess.check_output(['ps', '-axo', 'comm'], text=True).splitlines()
            counts = {name:sum(Path(value.strip()).name == name for value in names) for name in ['rustc','cargo','clang','swift-frontend']}
            monitor.append({'elapsed_s':round(time.monotonic()-started,3),'load':list(os.getloadavg()),'processes':counts})
            try:
                code = process.wait(timeout=2)
                break
            except subprocess.TimeoutExpired:
                pass
    record = {'label':label,'command':[str(value) for value in command],'exit_code':code,'elapsed_s':round(time.monotonic()-started,3),'log_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'monitor':monitor}
    records.append(record)
    (folder / (args.variant + '-runs.json')).write_text(json.dumps(records,indent=2)+'\n')
    print(args.variant,label,'exit',code,'seconds',record['elapsed_s'],flush=True)
    if code not in [0,1]:
        raise RuntimeError(path.read_text()[-3000:])

if args.quality:
    for name, grid in [('nine-sentences','nine'),('quanpin-sentences',None)]:
        command = [folder / (args.variant+'-convert_eval'),'--resources',resources,'--set','resources/eval/sentences-nine-key-v1.tsv','--report',folder/(args.variant+'-'+name+'.json'),'--dump',folder/(args.variant+'-'+name+'.jsonl')]
        if grid:
            command += ['--grid',grid]
        run(name,command)
if args.latency:
    run('nine-latency',[folder/(args.variant+'-rerank_latency'),'--resources',resources,'--set','resources/eval/sentences-nine-key-v1.tsv','--grid','nine'])

if args.long_latency:
    source=Path('resources/eval/sentences-nine-key-v1.tsv').read_text().splitlines()
    long_set=folder/'sentences-nine-key-12.tsv'
    long_set.write_text('\n'.join(line for line in source if line.startswith('#') or line.startswith('id\t') or (len(line.split('\t'))>3 and line.split('\t')[3]=='12'))+'\n')
    run('nine-12-latency',[folder/(args.variant+'-rerank_latency'),'--resources',resources,'--set',long_set,'--grid','nine'])

if args.words:
    run('nine-words',[folder/(args.variant+'-convert_eval'),'--resources',resources,'--set','resources/eval/quanpin-words-v1.tsv','--limit','3000','--grid','nine','--report',folder/(args.variant+'-nine-words.json'),'--dump',folder/(args.variant+'-nine-words.jsonl')])
