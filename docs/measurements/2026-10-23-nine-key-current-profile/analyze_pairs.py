from pathlib import Path
import itertools
import json
import math
import re
import statistics

folder = Path('target/current-nine-key-evidence')
run = json.loads((folder / 'paired-run.json').read_text())
if not run['timed']:
    records = run['preflight']
    result = {'timed': False, 'performance_budget_result': None,
              'preflight_observations': len(records),
              'preflight_seconds': records[-1]['time'] - records[0]['time'],
              'preflight_load_range': [min(r['load1'] for r in records), max(r['load1'] for r in records)],
              'preflight_with_processes': sum(bool(r['processes']) for r in records)}
else:
    cells = {}
    for line in (folder / 'paired.log').read_text().splitlines():
        match = re.fullmatch(r'ROW_PAIR (\d+) (\d+) (off|cold|hot) (\d+) (\d+) ([\d.]+) ([\d.]+)', line)
        if match:
            columns, width, mode, repeats, pair, old, new = match.groups()
            key = (int(columns), int(width), mode)
            pair = int(pair)
            cell = cells.setdefault(key, {})
            assert pair not in cell
            assert float(old) > 0 and float(new) > 0
            cell[pair] = {'repeats': int(repeats), 'old_seconds': float(old), 'new_seconds': float(new)}
    expected = set(itertools.product([4, 12], [1, 4, 16], ['off', 'cold', 'hot']))
    assert set(cells) == expected
    results = []
    for (columns, width, mode), pairs in sorted(cells.items()):
        assert set(pairs) == set(range(21))
        ratios = [pair['new_seconds'] / pair['old_seconds'] for pair in pairs.values()]
        results.append({'columns': columns, 'width': width, 'mode': mode, 'median': statistics.median(ratios), 'pairs': pairs})
    targets = [r['median'] for r in results if r['width'] >= 4 and r['mode'] != 'off']
    geometric_mean = math.exp(sum(math.log(ratio) for ratio in targets) / len(targets))
    external = [{**record, 'processes': [p for p in record['processes'] if p['pid'] != run['child_pid']]} for record in run['monitor']]
    budget = all(r['median'] < 1.05 for r in results) and geometric_mean < 0.98
    assert budget == (run['exit_code'] == 0)
    result = {'timed': True, 'performance_budget_result': budget, 'wide_target_geometric_mean': geometric_mean,
              'observations_with_external_processes': sum(bool(r['processes']) for r in external), 'results': results}
(folder / 'paired-analysis.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
