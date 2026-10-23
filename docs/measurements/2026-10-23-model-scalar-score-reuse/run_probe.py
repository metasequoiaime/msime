from pathlib import Path
import subprocess, json, os, time, hashlib
folder = Path('target/model-score-reuse-evidence').resolve()
resources = Path(os.environ['MSIME_PROBE_RESOURCES']).resolve()
command = [str(folder / 'probe-rerank_latency'), '--resources', str(resources), '--set', 'resources/eval/sentences-nine-key-v1.tsv', '--grid', 'nine']
env = os.environ.copy()
env['MSIME_PROBE_VOCAB'] = str(folder / 'vocab.json')
started = time.time()
with (folder / 'run.log').open('w') as output:
    child = subprocess.run(command, env=env, stdout=output, stderr=subprocess.STDOUT)
lines = (folder / 'run.log').read_text().splitlines()
records = [json.loads(line.split(',', 1)[1]) for line in lines if line.startswith('scalar-probe,')]
assert records, '探针没有观察到静态评分'
checkpoint = lines.index('scalar-probe-phase,warmup-finished')
warmup = [json.loads(line.split(',', 1)[1]) for line in lines[:checkpoint] if line.startswith('scalar-probe,')][-1]
differences = [json.loads(line.split(',', 1)[1]) for line in lines if line.startswith('scalar-probe-difference,')]
fallback_events = [line for line in lines if line.startswith('scalar-probe-fallback,')]
(folder / 'result.json').write_text(json.dumps({'command': command, 'exit_code': child.returncode, 'elapsed_seconds': time.time() - started, 'diagnostic_timing_only': True, 'final': records[-1], 'warmup': warmup, 'differences': differences, 'fallback_event_count': len(fallback_events), 'observations': len(records), 'resource_generation': resources.name, 'dataset_sha256': hashlib.sha256(Path('resources/eval/sentences-nine-key-v1.tsv').read_bytes()).hexdigest()}, indent=2) + '\n')
print(json.dumps({'exit_code': child.returncode, 'final': records[-1]}, indent=2))
