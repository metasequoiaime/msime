"""构建数字诊断程序，结束后逐字恢复产品源码；只运行公开合成资源。"""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


ROOT = Path.cwd()
EVIDENCE = ROOT / 'target/cache-trace-evidence'
DOCUMENTS = ROOT / 'docs/measurements/2026-10-23-quanpin-path-cache-working-set'
BASE = '11cdc33ba64dc95adb3c4621a185ddd26088a524'
DICTIONARY = ROOT / 'crates/engine/src/quanpin/dictionary.rs'
MODULE = DICTIONARY.parent / 'dictionary/traced_path_cache.rs'
LATENCY = ROOT / 'crates/input-runtime/examples/rerank_latency.rs'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    assert (ROOT / '.git').is_file()
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == BASE
    assert not subprocess.check_output(['git', 'diff', '--name-only', 'HEAD', '--', 'crates', 'Cargo.toml', 'Cargo.lock'], text=True).strip()
    assert not MODULE.exists()
    for name in ('rerank_latency', 'convert_eval'):
        assert (EVIDENCE / f'baseline-{name}').is_file()
    resources = Path(os.environ['RESOURCE_DIR'])
    lock = json.loads((ROOT / 'resources/desktop-dictionary.lock.json').read_text())
    for entry in lock['artifacts']:
        path = resources / entry['name']
        assert path.stat().st_size == entry['size']
        assert sha(path) == entry['sha256']
    (EVIDENCE / 'inputs.json').write_text(json.dumps(lock, indent=2) + '\n')
    originals = {path: path.read_bytes() for path in (DICTIONARY, LATENCY)}
    source = originals[DICTIONARY].decode()
    edits = [
        ('mod page_copy;', 'mod page_copy;\nmod traced_path_cache;\nuse traced_path_cache::TracedPathCache;'),
        ('    cache: FifoCache<String, Vec<WordItem>>,', '    cache: TracedPathCache,'),
        ('            cache: FifoCache::new(CACHE_CAPACITY),', '            cache: TracedPathCache::new(CACHE_CAPACITY),'),
        ('if let Some(rows) = self.cache.get_ref_by(&key) {', 'if let Some(rows) = self.cache.peek_ref_by(&key) {'),
    ]
    for old, new in edits:
        assert source.count(old) == 1, old
        source = source.replace(old, new)
    latency = originals[LATENCY].decode()
    marker = '    let mut discard = Arm::default();'
    assert latency.count(marker) == 1
    latency = latency.replace(marker, '    eprintln!("FIFO_TRACE_PHASE 0");\n' + marker)
    marker = '    let mut off = Arm::default();'
    assert latency.count(marker) == 1
    latency = latency.replace(marker, '    eprintln!("FIFO_TRACE_PHASE 1");\n' + marker)
    marker = '    runtime.dispatch(Action::Command(Command::Cancel))?;'
    assert latency.count(marker) == 1
    latency = latency.replace(marker, '''    static CASE_INDEX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let case_index = CASE_INDEX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    eprintln!("FIFO_TRACE_CASE {case_index}");
''' + marker)
    marker = '    for byte in keys {'
    assert latency.count(marker) == 1
    latency = latency.replace(marker, '''    for (key_index, byte) in keys.into_iter().enumerate() {
        eprintln!("FIFO_TRACE_KEY {key_index}");''')
    try:
        DICTIONARY.write_text(source)
        LATENCY.write_text(latency)
        shutil.copy2(DOCUMENTS / 'traced_path_cache.rs', MODULE)
        (EVIDENCE / 'diagnostic.patch').write_bytes(subprocess.check_output(['git', 'diff', '--', str(DICTIONARY), str(LATENCY)]))
        with (EVIDENCE / 'diagnostic-build.log').open('w') as log:
            subprocess.run(['cargo', 'build', '--release', '--locked', '-p', 'msime-input-runtime', '--example', 'rerank_latency', '--example', 'convert_eval'], stdout=log, stderr=subprocess.STDOUT, env={**os.environ, 'CARGO_BUILD_JOBS': '2'}, check=True)
        for name in ('rerank_latency', 'convert_eval'):
            shutil.copy2(ROOT / f'target/release/examples/{name}', EVIDENCE / f'diagnostic-{name}')
    finally:
        for path, content in originals.items():
            path.write_bytes(content)
        MODULE.unlink(missing_ok=True)
    for path, content in originals.items():
        assert path.read_bytes() == content
    summary = {'base': BASE, 'restored': {str(p.relative_to(ROOT)): sha(p) for p in originals}, 'binaries': {f'{arm}-{name}': sha(EVIDENCE / f'{arm}-{name}') for arm in ('baseline', 'diagnostic') for name in ('rerank_latency', 'convert_eval')}}
    command = [str(EVIDENCE / 'diagnostic-rerank_latency'), '--resources', str(resources), '--set', 'resources/eval/sentences-nine-key-v1.tsv', '--grid', 'nine', '--warmup', '5', '--budget-ms', '16']
    start = time.monotonic()
    with (EVIDENCE / 'trace.log').open('w') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    summary.update(exit_code=result.returncode, elapsed_seconds=time.monotonic() - start)
    (EVIDENCE / 'diagnostic-status.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
