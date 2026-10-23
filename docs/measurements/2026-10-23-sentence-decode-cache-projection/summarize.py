from pathlib import Path
import json
import re

folder = Path('target/cache-projection-evidence')
records = json.loads((folder / 'timed-runs.json').read_text())
summary = []
for record in records:
    log = (folder / record['log_file']).read_text()
    item = {key: record.get(key) for key in ['label', 'exit_code', 'elapsed_s', 'binary_sha256', 'log_file', 'log_sha256', 'preflight_policy']}
    item['monitor_points'] = len(record['monitor'])
    item['compiler_overlap_points'] = sum(any(point['compilers'].values()) for point in record['monitor'])
    cells = [{'shape': match[0], 'median_ratio': float(match[1]), 'limit': float(match[2])}
             for match in re.findall(r'summary ([^:]+): ([\d.]+), limit=([\d.]+)', log)]
    if cells:
        assert len(cells) == 96
        item['cells'] = len(cells)
        item['failed_cells'] = [cell for cell in cells if cell['median_ratio'] >= cell['limit']]
        item['target_min_ratio'] = min(cell['median_ratio'] for cell in cells if cell['limit'] == 0.98)
        item['target_max_ratio'] = max(cell['median_ratio'] for cell in cells if cell['limit'] == 0.98)
        item['control_max_ratio'] = max(cell['median_ratio'] for cell in cells if cell['limit'] == 1.05)
        (folder / (record['label'] + '-summary.json')).write_text(json.dumps(cells, indent=2) + '\n')
    for label in ['engine only', 'with model', 'reranker adds']:
        match = re.search(label + r'\s+mean\s+([\d.-]+)\s+p50\s+([\d.-]+)\s+p95\s+([\d.-]+)\s+p99\s+([\d.-]+)\s+max\s+([\d.-]+)', log)
        if match:
            item[label] = dict(zip(['mean', 'p50', 'p95', 'p99', 'max'], map(float, match.groups())))
    summary.append(item)
(folder / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
