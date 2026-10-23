"""整理本片的合成评测证据，清除本机路径并固定 gzip 时间戳。"""
from pathlib import Path
import argparse
import gzip
import hashlib
import json

parser = argparse.ArgumentParser()
parser.add_argument('--resources', type=Path, required=True)
parser.add_argument('--main-checkout', type=Path)
args = parser.parse_args()
root = Path.cwd()
source = root / 'target/page-compaction-evidence'
destination = Path(__file__).resolve().parent
replacements = [
    (str(root), '<WORKTREE>'),
    (str(args.resources.resolve()), '<RESOURCES>'),
    (str(Path.home()), '<HOME>'),
]
if args.main_checkout:
    replacements.append((str(args.main_checkout.resolve()), '<MAIN_CHECKOUT>'))
names = [
    'baseline-tests.log', 'red.log', 'green.log', 'nested-capacity.log', 'candidate-tests.log',
    'baseline-build.log', 'baseline-build-status.json', 'candidate-build.log', 'candidate-build-status.json',
    'baseline-quality.json', 'candidate-quality.json', 'baseline-quality-driver.log', 'candidate-quality-driver.log', 'verification-driver.log', 'verification-status.json',
    'first-candidate-tests.log', 'first-verification-status.json', 'first-verification-driver.log', 'session-budget.log', 'clippy.log', 'voice-prebuild.log', 'fmt.log', 'notes.log', 'quick.log', 'quick-status.json',
    'abba.json', 'abba-summary.json', 'abba-driver.log', 'abba-status.json', 'cpu.csv', 'cpu-summary.json', 'cpu.log', 'cpu-driver.log',
    'v1-cpu.csv', 'v1-cpu-summary.json', 'v1-cpu.log', 'v1-cpu-driver.log',
]
for name, path in [
    ('linux-native-gate.log', root / 'target/linux-native-gate.log'),
    ('linux-desktop-check.log', root / 'target/linux-desktop-check/check.log'),
]:
    if path.is_file():
        (source / name).write_bytes(path.read_bytes())
        names.append(name)
for variant in ['baseline', 'candidate']:
    for layout in ['quanpin', 'nine', 'fourteen']:
        for extension in ['json', 'jsonl', 'log']:
            names.append(f'{variant}-{layout}.{extension}')
for arm in ['A1', 'B1', 'B2', 'A2']:
    if (source / f'abba-{arm}.log').exists():
        names.append(f'abba-{arm}.log')
for name in names:
    content = (source / name).read_text()
    for before, after in replacements:
        content = content.replace(before, after)
    (destination / f'{name}.gz').write_bytes(gzip.compress(content.encode(), mtime=0))
content = (source / 'v1-path_page.rs').read_text()
(destination / 'v1-path_page.rs').write_text(content)
manifest = {'base_commit': '53df4a89c1db06d264eadf4e57cb0b54758d61fa', 'files': {}}
for path in sorted(destination.iterdir()):
    if path.is_file() and path.name != 'manifest.json':
        payload = path.read_bytes()
        manifest['files'][path.name] = {'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}
(destination / 'manifest.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + '\n')
print('packaged', len(manifest['files']), 'files')
