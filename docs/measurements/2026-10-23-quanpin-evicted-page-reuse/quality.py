from pathlib import Path
import argparse
import hashlib
import json
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--resources', type=Path, required=True)
parser.add_argument('--variant', choices=['baseline', 'candidate'], required=True)
args = parser.parse_args()
folder = Path('target/post-sqlite-evidence')
binary = folder / f'{args.variant}-convert_eval'
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
expected = sha(binary)
records = []
for grid, options in [('quanpin', []), ('nine', ['--grid', 'nine']), ('fourteen', ['--grid', 'fourteen'])]:
    stem = f'{args.variant}-{grid}'
    with (folder / f'{stem}.log').open('w') as output:
        result = subprocess.run([str(binary.resolve()), '--resources', str(args.resources.resolve()),
                                 '--set', 'resources/eval/sentences-nine-key-v1.tsv',
                                 '--report', str(folder / f'{stem}.json'), '--dump', str(folder / f'{stem}.jsonl'),
                                 *options], stdout=output, stderr=subprocess.STDOUT)
    assert result.returncode == 0, stem
    assert sha(binary) == expected
    records.append({'grid': grid, 'report_sha256': sha(folder / f'{stem}.json'),
                    'candidates_sha256': sha(folder / f'{stem}.jsonl')})
    print(stem, 'exit 0', flush=True)
(folder / f'{args.variant}-quality.json').write_text(json.dumps({'binary_sha256': expected, 'results': records}, indent=2) + '\n')
if args.variant == 'candidate':
    for grid, _ in [('quanpin', []), ('nine', []), ('fourteen', [])]:
        for extension in ['json', 'jsonl']:
            assert (folder / f'candidate-{grid}.{extension}').read_bytes() == (folder / f'baseline-{grid}.{extension}').read_bytes()
    print('三种布局的完整质量报告与全部候选逐字节相同；该评测关闭英文混输，仅作控制', flush=True)
