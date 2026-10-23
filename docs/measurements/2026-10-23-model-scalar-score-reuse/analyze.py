from pathlib import Path
import collections, gzip, json, re
folder = Path('target/model-score-reuse-evidence')
rows = [json.loads(line.split(',', 1)[1]) for line in (folder / 'run.log').read_text().splitlines() if line.startswith('scalar-probe,')]
last = [0] * 4
histograms = [collections.Counter() for _ in last]
patterns = []
for row in rows:
    delta = [value - before for value, before in zip(row['hits'], last)]
    for histogram, count in zip(histograms, delta):
        histogram[count] += 1
    patterns.append(delta)
    last = row['hits']
previous = Path('docs/measurements/2026-10-23-lattice-column-score-cache/probe-run.log.gz')
old_log = gzip.decompress(previous.read_bytes()).decode()
old_rows = [json.loads(re.search(r'hits=(\[[^]]+\])', line).group(1)) for line in old_log.splitlines() if line.startswith('rerank-probe,')]
old_last = 0
matched = []
for old_row, new_delta in zip(old_rows, patterns):
    old_delta = old_row[-1] - old_last
    old_last = old_row[-1]
    matched.append(new_delta[-1] == old_delta * 9)
result = json.loads((folder / 'result.json').read_text())
measured = {key: result['final'][key] - result['warmup'][key] for key in ['calls', 'items', 'previous', 'current']}
for key in ['hits', 'additional', 'different', 'additional_different']:
    measured[key] = [a - b for a, b in zip(result['final'][key], result['warmup'][key])]
summary = {'per_call_hit_histograms': [dict(histogram) for histogram in histograms], 'old_batch_128_observations': len(old_rows), 'new_scalar_1024_observations': len(rows), 'same_per_call_hit_pattern': len(rows) == len(old_rows) and all(matched), 'after_warmup_counters': measured}
(folder / 'coverage-analysis.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
