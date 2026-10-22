from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import os

root = Path.cwd()
folder = root / 'target/reranker-profile-evidence'
allocator = root / 'crates/engine/src/ime/personal_rerank.rs'
original = allocator.read_bytes()
needle = b'    #[global_allocator]\n'
assert original.count(needle) == 1
try:
    allocator.write_bytes(original.replace(needle, b'    #[allow(dead_code)]\n'))
    with (folder / 'system-build.jsonl').open('w') as output, (folder / 'system-build.log').open('w') as errors:
        result = subprocess.run(['cargo', 'test', '--locked', '--release', '-p', 'msime-engine', '--lib', '--no-run', '--message-format=json'], stdout=output, stderr=errors, env={**os.environ, 'CARGO_BUILD_JOBS':'4'})
    assert result.returncode == 0, result.returncode
    records = [json.loads(line) for line in (folder / 'system-build.jsonl').read_text().splitlines()]
    binaries = [r['executable'] for r in records if r.get('reason') == 'compiler-artifact' and r.get('target', {}).get('name') == 'msime_engine' and r.get('executable')]
    assert len(binaries) == 1, binaries
    binary = folder / 'paired-system'
    shutil.copy2(binaries[0], binary)
    (folder / 'system-binary.json').write_text(json.dumps({'allocator': 'std::alloc::System', 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}, indent=2) + '\n')
finally:
    allocator.write_bytes(original)
assert allocator.read_bytes() == original
