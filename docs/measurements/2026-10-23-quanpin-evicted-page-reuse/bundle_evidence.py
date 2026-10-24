from pathlib import Path
import gzip
import hashlib
import json
import re

source = Path('target/post-sqlite-evidence')
target = Path(__file__).resolve().parent
gates = json.loads((source / 'final-gates.json').read_text())
assert all(gates[name] == 0 for name in ['full_tests', 'clippy', 'format', 'notes', 'quick']), '最终门禁未全部通过，拒绝打包'
names = [
    'inputs.json', 'baseline-build.json', 'candidate-build.json', 'upstream-contract-review.json',
    'profile.sample.txt', 'sample.log', 'profile-run.log', 'profile-run.json',
    'profile-analysis.json', 'profile-detail-reproduced.json',
    'allocation-callers-reproduced.json', 'detail-final.log',
    'frozen-verification.json', 'red-test.log', 'green-test.log', 'full-tests.log',
    'candidate-build.log', 'clippy.log', 'notes-final.log', 'format-final.log',
    'quick.log', 'quick-initial-status.json', 'quick-recheck.log', 'quick-recheck-status.json',
    'voice-catalog-build.log', 'voice-catalog-recheck.log', 'voice-catalog-recheck.json',
    'baseline-quality.json', 'candidate-quality.json',
    'baseline-quality-driver.log', 'candidate-quality-driver.log',
    'abba.json', 'abba-summary.json', 'abba-status.json', 'abba-driver.log',
    'abba-A1.log', 'abba-B1.log', 'abba-B2.log', 'abba-A2.log',
    'final-gates.json',
]
for variant in ['baseline', 'candidate']:
    names.extend(f'{variant}-{grid}.json' for grid in ['quanpin', 'nine', 'fourteen'])
replacements = [
    (str(Path.cwd()), '<worktree>'),
    ('/Volumes/Workspace/ime/msime', '<main-worktree>'),
    (str(Path.home()), '<home>'),
]
secret = re.compile(r'github_pat_[A-Za-z0-9_]{20,}|ghp_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9_-]{20,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----')
manifest = []
sha = lambda data: hashlib.sha256(data).hexdigest()
for name in names:
    path = source / name
    if not path.exists():
        continue
    raw = path.read_bytes()
    text = raw.decode()
    assert not secret.search(text), f'{name} 包含疑似凭据，拒绝打包'
    for before, after in replacements:
        text = text.replace(before, after)
    text = re.sub(r'(?:/private)?/var/folders/[^\s"\x27):,]+', '<temporary>', text)
    clean = text.encode()
    compressed = gzip.compress(clean, mtime=0)
    (target / f'{name}.gz').write_bytes(compressed)
    manifest.append({'name': name, 'raw_bytes': len(raw), 'raw_sha256': sha(raw),
                     'sanitized_bytes': len(clean), 'sanitized_sha256': sha(clean),
                     'gzip_bytes': len(compressed), 'gzip_sha256': sha(compressed)})
(target / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
print(f'已打包 {len(manifest)} 份脱敏证据')
