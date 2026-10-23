import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

parser=argparse.ArgumentParser()
parser.add_argument('--resources',type=Path,default=Path(os.environ.get('MSIME_EVAL_RESOURCES','target/resources/7356c4a452d8ba15475c0e57f134fccf7b7cbe267440461541c74d09b11b6ffe')))
parser.add_argument('--paired',action='store_true')
parser.add_argument('--abba',action='store_true')
parser.add_argument('--label',default='paired-v2')
parser.add_argument('--quiet-seconds',type=int,default=5)
parser.add_argument('--idle-timeout',type=int,default=180)
parser.add_argument('--max-load',type=float,default=float('inf'))
args=parser.parse_args()
folder=Path('target/cache-projection-evidence').resolve()
resources=args.resources.resolve()
records=json.loads((folder/'timed-runs.json').read_text()) if (folder/'timed-runs.json').exists() else []
def observation():
    names=[Path(p.strip()).name for p in subprocess.check_output(['ps','-axo','comm='],text=True).splitlines()]
    return {'at':time.monotonic(),'load':list(os.getloadavg()),'compilers':{n:names.count(n) for n in ['cargo','rustc','clang','swift-frontend']}}

def run(label,command):
    pre=[]
    deadline=time.monotonic()+args.idle_timeout
    quiet=0
    while True:
        point=observation();pre.append(point)
        quiet=quiet+1 if not any(point['compilers'].values()) and point['load'][0] <= args.max_load else 0
        if quiet>=args.quiet_seconds:break
        if time.monotonic()>deadline:raise RuntimeError('等待现有编译进程退出超时，尚未开始计时')
        time.sleep(1)
    log=folder/(label+'.log')
    started=time.monotonic()
    monitor=[]
    with log.open('w') as output:
        child=subprocess.Popen([str(x) for x in command],stdout=output,stderr=subprocess.STDOUT)
        while True:
            monitor.append(observation())
            try:
                code=child.wait(timeout=1)
                break
            except subprocess.TimeoutExpired:pass
    item={'label':label,'preflight_policy':{'quiet_seconds':args.quiet_seconds,'max_load':args.max_load if args.max_load != float('inf') else None},'command':[str(x) for x in command],'exit_code':code,'elapsed_s':time.monotonic()-started,'preflight':pre,'monitor':monitor,'binary_sha256':hashlib.sha256(Path(command[0]).read_bytes()).hexdigest(),'log_file':log.name,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()}
    records.append(item)
    (folder/'timed-runs.json').write_text(json.dumps(records,indent=2)+'\n')
    print(label,'exit',code,'seconds',round(item['elapsed_s'],2),flush=True)
    return code

if args.paired:
    binary=folder/'paired-system'
    assert run(args.label,[binary,'lattice::merge::decoded_projection_tests::cached_projection_paired_latency','--exact','--ignored','--nocapture']) == 0
if args.abba:
    for label,variant in [('A1','baseline'),('B1','candidate-32'),('B2','candidate-32'),('A2','baseline')]:
        run(args.label+'-nine-latency-'+label,[folder/(variant+'-rerank_latency'),'--resources',resources,'--set','resources/eval/sentences-nine-key-v1.tsv','--grid','nine'])
