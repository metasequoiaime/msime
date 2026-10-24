"""链接真实 WordItem 和生产 PathPage；不启用测试计数分配器。"""
from pathlib import Path
import csv
import json
import statistics
import subprocess

root = Path.cwd()
output = root / 'target/page-compaction-evidence'
source = Path(__file__).with_name('cpu.rs').read_text()
source += f'\n#[path = {json.dumps(str(root / "crates/engine/src/cache.rs"))}]\nmod cache;\n'
source += 'mod path_page {\ninclude!(' + json.dumps(str(root / 'crates/engine/src/quanpin/dictionary/path_page.rs')) + ');\n'
source += '''
impl PathPage {
    pub(crate) fn retained_bytes(&self) -> usize {
        use std::mem::size_of;
        match self {
            Self::Empty => 0,
            Self::Full(rows) => rows.capacity() * size_of::<crate::WordItem>() + rows.iter().map(|r| r.pinyin.capacity() + r.canonical_pinyin.capacity() + r.word.capacity() + r.corrected_from.capacity() + r.sentence_words.capacity() * size_of::<String>() + r.sentence_words.iter().map(String::capacity).sum::<usize>()).sum::<usize>(),
            Self::Compact(page) => size_of::<CompactPage>() + page.pinyin.as_ref().map_or(0, String::capacity) + page.rows.capacity() * size_of::<crate::dictionary::DictRow>() + page.rows.iter().map(|r| r.key.capacity() + r.value.capacity()).sum::<usize>(),
        }
    }
}
}
'''
generated = output / 'cpu-generated.rs'
generated.write_text(source)
command = ['rustc', '--edition=2021', '-O', '-A', 'dead_code', str(generated), '-L', f'dependency={root / "target/release/deps"}', '-o', str(output / 'cpu')]
for dependency in ['msime_engine', 'lru', 'libc']:
    libraries = list((root / 'target/release/deps').glob(f'lib{dependency}-*.rlib'))
    assert len(libraries) == 1, libraries
    command += ['--extern', f'{dependency}={libraries[0]}']
subprocess.run(command, check=True)
with (output / 'cpu.csv').open('w') as stdout, (output / 'cpu.log').open('w') as stderr:
    subprocess.run([str(output / 'cpu')], stdout=stdout, stderr=stderr, check=True)
cells = {}
for row in csv.DictReader((output / 'cpu.csv').open()):
    key = tuple(row[field] for field in ['rows', 'key_bytes', 'matching', 'mode'])
    cells.setdefault(key, []).append(row)
assert len(cells) == 84
summary = []
for key, rows in cells.items():
    assert len(rows) == 21
    ratio = statistics.median(int(r['new_ns']) / int(r['old_ns']) for r in rows)
    limit = 0.98 if key[3] == 'cold' and int(key[0]) >= 32 else 1.05
    summary.append(dict(zip(['rows', 'key_bytes', 'matching', 'mode'], key)) | {'median_ratio': ratio, 'limit': limit, 'passed': ratio <= limit, 'old_page_bytes': int(rows[0]['old_page_bytes']), 'new_page_bytes': int(rows[0]['new_page_bytes'])})
(output / 'cpu-summary.json').write_text(json.dumps(summary, indent=2) + '\n')
failed = [cell for cell in summary if not cell['passed']]
print(json.dumps({'cells': len(cells), 'failed': failed}, indent=2), flush=True)
