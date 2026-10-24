"""产品测试和静态检查；quick 门禁单独记录最终树。"""
from pathlib import Path
import argparse
import json
import os
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--resources', type=Path, required=True)
args = parser.parse_args()
folder = Path('target/page-compaction-evidence')
environment = dict(os.environ, CARGO_BUILD_JOBS='2', MSIME_NEURAL_MODEL_DIR=str(args.resources.resolve()))
results = []
for name, command in [
    ('candidate-tests', ['cargo', 'test', '--locked', '-p', 'msime-engine', '-p', 'msime-input-runtime', '--no-fail-fast']),
    ('clippy', ['cargo', 'clippy', '--locked', '-p', 'msime-engine', '-p', 'msime-input-runtime', '--all-targets', '--', '-D', 'warnings']),
    ('voice-prebuild', ['cargo', 'build', '--locked', '-p', 'msime-client-providers', '--example', 'export_voice_language_catalog']),
    ('fmt', ['cargo', 'fmt', '--all', '--', '--check']),
]:
    started = time.monotonic()
    with (folder / f'{name}.log').open('w') as output:
        child = subprocess.run(command, env=environment, stdout=output, stderr=subprocess.STDOUT)
    results.append({'name': name, 'command': command, 'exit_code': child.returncode, 'seconds': time.monotonic() - started})
    (folder / 'verification-status.json').write_text(json.dumps(results, indent=2) + '\n')
    print(name, 'exit', child.returncode, flush=True)
    if child.returncode:
        raise SystemExit(child.returncode)
