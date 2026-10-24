"""固定公开九键集合，核对诊断包装前后的完整输出。"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resources', type=Path, required=True)
    parser.add_argument('--arm', choices=('baseline', 'diagnostic'), required=True)
    args = parser.parse_args()
    output = Path('target/cache-trace-evidence')
    binary = (output / f'{args.arm}-convert_eval').resolve()
    before = sha(binary)
    prefix = output / f'{args.arm}-nine'
    with prefix.with_suffix('.log').open('w') as log:
        result = subprocess.run([str(binary), '--resources', str(args.resources), '--set', 'resources/eval/sentences-nine-key-v1.tsv', '--grid', 'nine', '--report', str(prefix.with_suffix('.json')), '--dump', str(prefix.with_suffix('.jsonl'))], stdout=log, stderr=subprocess.STDOUT)
    assert result.returncode == 0
    assert sha(binary) == before
    for extension in ('json', 'jsonl'):
        if args.arm == 'diagnostic':
            assert prefix.with_suffix('.' + extension).read_bytes() == (output / ('baseline-nine.' + extension)).read_bytes()
    report = {'exit_code': result.returncode, 'binary_sha256': before, 'report_sha256': sha(prefix.with_suffix('.json')), 'candidates_sha256': sha(prefix.with_suffix('.jsonl')), 'equal_to_baseline': args.arm == 'diagnostic'}
    (output / f'{args.arm}-quality-status.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
