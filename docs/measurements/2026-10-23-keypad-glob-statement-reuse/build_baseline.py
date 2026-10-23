from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

root = Path.cwd()
folder = root / 'target/keypad-glob-evidence'
base = 'e60da08cb07196e3008a0c83fdb8e4f6961dcc0f'
paths = [Path('crates/engine/src/dictionary/english.rs')]
allowed = {str(path) for path in paths} | {
    'crates/engine/src/dictionary/english/keypad_append_tests.rs',
    'crates/engine/src/dictionary/english/keypad_reprepare_tests.rs',
}
changed = set(subprocess.check_output(['git', 'diff', '--name-only', base, '--', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'resources'], text=True).splitlines())
assert changed <= allowed, '固定基线外存在其它产品改动，拒绝构建混合基线'
originals = {path: path.read_bytes() for path in paths}
sha = lambda data: hashlib.sha256(data).hexdigest()
try:
    for path in paths:
        path.write_bytes(subprocess.check_output(['git', 'show', f'{base}:{path}']))
    with (folder / 'baseline-build.log').open('w') as output:
        subprocess.run(['cargo', 'build', '--release', '--locked', '-p', 'msime-input-runtime', '--example', 'convert_eval', '--example', 'rerank_latency'],
                       check=True, stdout=output, stderr=subprocess.STDOUT, env={**os.environ, 'CARGO_BUILD_JOBS': '4'})
    binaries = {}
    for name in ['convert_eval', 'rerank_latency']:
        source = root / 'target/release/examples' / name
        destination = folder / f'baseline-{name}'
        shutil.copy2(source, destination)
        binaries[name] = sha(destination.read_bytes())
finally:
    for path, data in originals.items():
        path.write_bytes(data)
assert all(path.read_bytes() == data for path, data in originals.items())
(folder / 'baseline-build.json').write_text(json.dumps({'base': base, 'binaries': binaries, 'candidate_sources_restored': {str(path): sha(data) for path, data in originals.items()}}, indent=2) + '\n')
print('固定基线 release 程序已构建，候选源码逐字恢复')
