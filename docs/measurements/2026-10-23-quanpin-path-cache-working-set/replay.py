"""重放固定路径缓存访问序列；不把反事实重放当作真实按键运行。"""

import argparse
from collections import Counter, OrderedDict
import gzip
import json
from pathlib import Path


def load_events(path):
    opener = gzip.open if path.suffix == '.gz' else open
    with opener(path, 'rt') as stream:
        return [line.split()[1:] for line in stream if line.startswith('FIFO_TRACE ')]


def annotate(events):
    actual = {}
    annotated = []
    summaries = Counter()
    content = {}
    for index, event in enumerate(events):
        kind, cache = event[:2]
        cache = int(cache)
        if kind == 'N':
            assert cache not in actual
            actual[cache] = OrderedDict()
            assert int(event[2]) == 128
            summaries['cache_instances'] += 1
        elif kind in ('G', 'P'):
            key, hit = map(int, event[2:])
            rows = actual[cache].get(key)
            assert bool(rows) == bool(hit), (index, event)
            if kind == 'G' and not hit:
                following = events[index + 1]
                assert following[:3] == ['S', str(cache), str(key)], (event, following)
                rows = tuple(following[3:])
            annotated.append((kind, cache, key, rows))
            summaries[f'{kind}_hits' if hit else f'{kind}_misses'] += 1
            continue
        elif kind == 'S':
            key = int(event[2])
            rows = tuple(event[3:])
            assert len(rows) == 5
            old = content.get((cache, key))
            if old is not None and old[-1] != rows[-1]:
                summaries['content_changes_without_clear'] += 1
            content[cache, key] = rows
            endogenous = index > 0 and events[index - 1] == ['G', str(cache), str(key), '0']
            summaries['demand_insertions' if endogenous else 'external_insertions'] += 1
            if key not in actual[cache] and len(actual[cache]) == 128:
                actual[cache].popitem(last=False)
            actual[cache][key] = rows
            if endogenous:
                continue
            annotated.append(('S', cache, key, rows))
            continue
        elif kind in ('C', 'D'):
            actual[cache].clear()
            content = {k: v for k, v in content.items() if k[0] != cache}
            summaries[kind] += 1
            if kind == 'D':
                del actual[cache]
        else:
            raise AssertionError(event)
        annotated.append((kind, cache, None, None))
    assert not actual, '必须记录每个实例析构'
    return annotated, summaries


def simulate(events, positive, negative=None, skip_empty=False):
    caches = {}
    counts = Counter()
    held = Counter()
    peak = Counter()

    def account(rows, sign):
        held['entries'] += sign
        held['key_requested_bytes'] += sign * int(rows[1])
        held['page_requested_bytes'] += sign * int(rows[2])

    def store(cache, key, rows):
        empty = int(rows[0]) == 0
        bucket = int(empty) if negative is not None else 0
        slots = caches[cache][bucket]
        capacity = negative if bucket else positive
        if skip_empty and empty:
            for existing in caches[cache]:
                if key in existing:
                    account(existing.pop(key), -1)
            return
        for existing in caches[cache]:
            if key in existing and existing is not slots:
                account(existing.pop(key), -1)
        if key not in slots and len(slots) == capacity:
            _, evicted = slots.popitem(last=False)
            account(evicted, -1)
            counts['evictions'] += 1
            counts['empty_evictions' if int(evicted[0]) == 0 else 'nonempty_evictions'] += 1
        if key in slots:
            account(slots[key], -1)
        slots[key] = rows
        account(rows, 1)

    def lookup(cache, key):
        for slots in caches[cache]:
            if key in slots:
                return slots[key]
        return None

    for kind, cache, key, rows in events:
        if kind == 'N':
            caches[cache] = [OrderedDict() for _ in range(2 if negative is not None else 1)]
        elif kind in ('C', 'D'):
            for slots in caches[cache]:
                for row in slots.values():
                    account(row, -1)
                slots.clear()
            if kind == 'D':
                del caches[cache]
        elif kind == 'S':
            store(cache, key, rows)
        elif kind in ('G', 'P'):
            cached = lookup(cache, key)
            hit = cached is not None
            counts[f'{kind}_hits' if hit else f'{kind}_misses'] += 1
            if kind == 'P':
                counts['probe_outcome_changes'] += hit != (rows is not None)
            if cached is not None and rows is not None:
                counts['changed_content_hits'] += cached[-1] != rows[-1]
            if kind == 'G':
                empty = int(rows[0]) == 0
                counts[('empty_' if empty else 'nonempty_') + ('hits' if hit else 'misses')] += 1
                if not hit:
                    counts['rebuilt_rows'] += int(rows[0])
                    if not (skip_empty and empty):
                        counts['cloned_page_requested_bytes'] += int(rows[2])
                        counts['cloned_page_allocations'] += int(rows[3])
                        counts['owned_key_requested_bytes'] += int(rows[1])
                        counts['owned_key_allocations'] += int(rows[1]) != 0
                    store(cache, key, rows)
        held['key_plus_page_requested_bytes'] = held['key_requested_bytes'] + held['page_requested_bytes']
        for metric, value in held.items():
            peak[metric] = max(peak[metric], value)
    assert not caches and all(value == 0 for value in held.values())
    return {'counts': dict(counts), 'peak_across_live_instances': dict(peak)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trace', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    raw = load_events(args.trace)
    events, actual = annotate(raw)
    baseline = simulate(events, 128)
    for metric in ('G_hits', 'G_misses', 'P_hits', 'P_misses'):
        assert baseline['counts'].get(metric, 0) == actual[metric], metric
    assert baseline['counts'].get('changed_content_hits', 0) == 0
    configs = [(f'fifo-{cap}', cap, None, False) for cap in (128, 256, 512, 1024, 2048, 4096)]
    configs += [(f'split-{p}-{n}', p, n, False) for p, n in ((64, 64), (96, 32), (64, 128), (128, 256))]
    configs += [('skip-empty-128', 128, None, True)]
    results = {name: simulate(events, p, n, skip) for name, p, n, skip in configs}
    report = {'fixed_sequence_only': True, 'raw_events': len(raw), 'original_verified': dict(actual), 'results': results}
    args.out.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
