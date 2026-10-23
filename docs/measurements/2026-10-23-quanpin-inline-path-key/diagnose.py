from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess

root = Path.cwd()
folder = root / 'target/next-hotspot-evidence'
source = root / 'crates/engine/src/quanpin/dictionary.rs'
assert (root / '.git').is_file(), '必须使用独立 worktree'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip() == 'e7fa91f121e0b071bddfaa96917f87869e37018e'
assert not subprocess.check_output(['git', 'diff', '--name-only', 'HEAD', '--', 'crates', 'Cargo.toml', 'Cargo.lock'], text=True).strip()
original = source.read_bytes()
needle = b'        self.cache.insert(cache_key.to_owned(), result.clone());'
assert original.count(needle) == 1
replacement = b'''        let (_, previous) = self.cache.insert_with_previous(cache_key.to_owned(), result.clone());
        let mut reused = 0usize;
        let mut total = usize::from(!result.is_empty());
        if let Some(previous) = &previous {
            reused += usize::from(!result.is_empty() && previous.capacity() == result.len());
        }
        for (index, row) in result.iter().enumerate() {
            for (field, value) in [&row.pinyin, &row.canonical_pinyin, &row.word, &row.corrected_from].iter().enumerate() {
                total += usize::from(!value.is_empty());
                if let Some(old) = previous.as_ref().and_then(|rows| rows.get(index)) {
                    let old = [&old.pinyin, &old.canonical_pinyin, &old.word, &old.corrected_from][field];
                    reused += usize::from(!value.is_empty() && old.capacity() == value.len());
                }
            }
            assert!(row.sentence_words.is_empty());
        }
        eprintln!("PATH_COPY_DIAGNOSTIC {} {} {} {} {}", result.len(), usize::from(previous.is_some()), total, reused, cache_key.len());'''
try:
    source.write_bytes(original.replace(needle, replacement))
    with (folder / 'diagnostic-build.log').open('w') as out:
        subprocess.run(['cargo', 'build', '--release', '--locked', '-p', 'msime-input-runtime', '--example', 'rerank_latency'], stdout=out, stderr=subprocess.STDOUT, check=True, env={**os.environ, 'CARGO_BUILD_JOBS': '2'})
    diagnostic = folder / 'diagnostic-rerank_latency'
    shutil.copy2(root / 'target/release/examples/rerank_latency', diagnostic)
    (folder / 'diagnostic-patch.txt').write_bytes(replacement)
finally:
    source.write_bytes(original)
assert source.read_bytes() == original
resource = Path(os.environ['RESOURCE_DIR'])
lock = json.loads((root / 'resources/desktop-dictionary.lock.json').read_text())
for entry in lock['artifacts']:
    path = resource / entry['name']
    assert path.stat().st_size == entry['size']
    assert hashlib.sha256(path.read_bytes()).hexdigest() == entry['sha256']
(folder / 'inputs.json').write_text(json.dumps(lock, indent=2) + '\n')
command = [str(diagnostic), '--resources', str(resource), '--set', 'resources/eval/sentences-nine-key-v1.tsv', '--grid', 'nine', '--warmup', '5', '--budget-ms', '16']
with (folder / 'diagnostic-run.log').open('w') as out:
    result = subprocess.run(command, stdout=out, stderr=subprocess.STDOUT)
rows = []
for line in (folder / 'diagnostic-run.log').read_text().splitlines():
    if line.startswith('PATH_COPY_DIAGNOSTIC '):
        rows.append(list(map(int, line.split()[1:])))
report = {'diagnostic_only': True, 'host_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(), 'binary_sha256': hashlib.sha256(diagnostic.read_bytes()).hexdigest(), 'exit_code': result.returncode, 'queries': len(rows), 'evicting_queries': sum(row[1] for row in rows), 'total_rows': sum(row[0] for row in rows), 'clone_allocations': sum(row[2] for row in rows), 'reusable_allocations': sum(row[3] for row in rows), 'queries_with_reusable_allocations': sum(row[3] > 0 for row in rows), 'key_lengths': {str(n): sum(row[4] == n for row in rows) for n in sorted({row[4] for row in rows})}, 'counts': rows}
(folder / 'diagnostic-counts.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({key: value for key, value in report.items() if key != 'counts'}, indent=2))
