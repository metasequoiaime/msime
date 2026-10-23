"""按实际清空边界、预热及逐键刷新统计路径缓存工作集。"""

import argparse
from collections import Counter, defaultdict
import gzip
import json
from pathlib import Path


def describe(values):
    values = sorted(values)
    def percentile(p):
        return values[min(len(values) - 1, (len(values) * p + 99) // 100 - 1)] if values else 0
    return {'count': len(values), 'sum': sum(values), 'p50': percentile(50), 'p95': percentile(95), 'max': max(values, default=0)}


def summarize(path):
    opener = gzip.open if path.suffix == '.gz' else open
    frames = []
    frame = None
    phase = -1
    epochs = defaultdict(set)
    cumulative_keys = defaultdict(set)
    counters = Counter()
    epoch_sizes = []
    last_query = {}
    gaps = []
    query_index = Counter()
    with opener(path, 'rt') as source:
        for line in source:
            fields = line.split()
            if not fields:
                continue
            if fields[0] in ('FIFO_TRACE_CASE', 'FIFO_TRACE_PHASE', 'FIFO_TRACE_KEY'):
                if frame is not None:
                    frames.append(frame)
                    frame = None
                if fields[0] == 'FIFO_TRACE_PHASE':
                    phase = int(fields[1])
                elif fields[0] == 'FIFO_TRACE_KEY':
                    frame = {'phase': phase, 'queries': 0, 'misses': 0, 'probes': 0, 'distinct': set(), 'miss_keys': set()}
                continue
            if fields[0] != 'FIFO_TRACE':
                continue
            kind, cache = fields[1:3]
            cache = int(cache)
            if kind in ('C', 'D'):
                if epochs[cache]:
                    epoch_sizes.append(len(epochs[cache]))
                epochs[cache].clear()
                last_query = {k: v for k, v in last_query.items() if k[0] != cache}
            elif kind == 'G':
                key, hit = map(int, fields[3:5])
                identity = cache, key
                query_index[cache] += 1
                if identity in last_query:
                    gaps.append(query_index[cache] - last_query[identity])
                last_query[identity] = query_index[cache]
                if not hit:
                    counters['repeat_misses_in_epoch' if key in epochs[cache] else 'first_misses_in_epoch'] += 1
                epochs[cache].add(key)
                cumulative_keys[cache].add(key)
                if frame is not None:
                    frame['queries'] += 1
                    frame['misses'] += not hit
                    frame['distinct'].add(identity)
                    if not hit:
                        frame['miss_keys'].add(identity)
            elif kind == 'P' and frame is not None:
                frame['probes'] += 1
            elif kind == 'S':
                key, rows = map(int, fields[3:5])
                counters['empty_insertions' if rows == 0 else 'nonempty_insertions'] += 1
    if frame is not None:
        frames.append(frame)
    by_phase = {}
    for tag in (0, 1):
        group = [f for f in frames if f['phase'] == tag]
        by_phase['warmup' if tag == 0 else 'formal'] = {
            metric: describe([len(f[metric]) if isinstance(f[metric], set) else f[metric] for f in group])
            for metric in ('queries', 'misses', 'probes', 'distinct', 'miss_keys')
        }
        by_phase['warmup' if tag == 0 else 'formal']['refreshes_above_128_distinct_queries'] = sum(len(f['distinct']) > 128 for f in group)
    return {'counters': dict(counters), 'distinct_query_keys_per_instance': {str(k): len(v) for k, v in cumulative_keys.items()}, 'distinct_query_keys_per_nonempty_epoch': describe(epoch_sizes), 'query_gap_in_same_cache': describe(gaps), 'refreshes': by_phase}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trace', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    report = summarize(args.trace)
    args.out.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
