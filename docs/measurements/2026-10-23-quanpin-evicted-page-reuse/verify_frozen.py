from pathlib import Path
import hashlib
import json
import re
import subprocess

base = '6fa568ce12fa7d6c129fce7c4428eb4e8a1717d1'
source = subprocess.check_output(['git', 'show', f'{base}:crates/engine/src/quanpin/dictionary.rs'], text=True)
test = Path('crates/engine/src/quanpin/dictionary/evicted_page_tests.rs').read_text()

def extract(text, signature):
    start = text.index(signature)
    brace = text.index('{', start)
    depth = 1
    cursor = brace + 1
    while depth:
        depth += (text[cursor] == '{') - (text[cursor] == '}')
        cursor += 1
    return text[start:cursor]

def normalize(text):
    text = re.sub(r'(?m)^\s*//.*$', '', text)
    text = subprocess.run(['rustfmt', '--emit', 'stdout', '--edition', '2021'], input=text, text=True, capture_output=True, check=True).stdout
    return re.sub(r'\s+', '', text)

records = []
for name in ['query_filtered_rows', 'query_exact_projected']:
    actual = extract(test, f'fn before_recycle_{name}(').replace('before_recycle_', '')
    expected = extract(source, f'fn {name}(')
    assert normalize(actual) == normalize(expected), name
    records.append({'function': name, 'normalized_sha256': hashlib.sha256(normalize(actual).encode()).hexdigest()})
Path('target/post-sqlite-evidence/frozen-verification.json').write_text(json.dumps({'base': base, 'matches': records}, indent=2) + '\n')
print('两个完整基线函数机械核对一致')
