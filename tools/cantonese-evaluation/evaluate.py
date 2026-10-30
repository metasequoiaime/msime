#!/usr/bin/env python3
"""用固定公开输入评估粤语补词，实验库只写入 target，不修改发布资源。"""
import argparse
import collections
import hashlib
import json
import re
import shutil
import sqlite3
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def verified_inputs(directory, fetch):
    manifest = json.loads(Path(__file__).with_name('inputs.json').read_text(encoding='utf-8'))
    directory.mkdir(parents=True, exist_ok=True)
    for item in manifest:
        path = directory / item['name']
        if not path.exists() and fetch:
            with urllib.request.urlopen(item['url'], timeout=60) as response:
                data = response.read(item['size'] + 1)
            if len(data) != item['size'] or hashlib.sha256(data).hexdigest() != item['sha256']:
                raise ValueError('公开输入校验失败：' + item['name'])
            path.write_bytes(data)
        data = path.read_bytes()
        if len(data) != item['size'] or hashlib.sha256(data).hexdigest() != item['sha256']:
            raise ValueError('公开输入校验失败：' + item['name'])
    upstream = json.loads((directory / 'upstream.lock.json').read_text(encoding='utf-8'))
    current = json.loads((ROOT / 'resources/dictionary-sources.lock.json').read_text(encoding='utf-8'))
    for name in ['rime-cantonese', 'hkcancor']:
        if upstream['upstreams'][name] != current['references'][name]:
            raise ValueError('本仓来源锁已变化，需重审评估输入：' + name)
    actual = {item['name']: item for item in manifest}
    for item in upstream['files']:
        basename = Path(item['path']).name
        name = {'jyut6ping3.chars.dict.yaml': 'chars.yaml', 'jyut6ping3.words.dict.yaml': 'words.yaml',
                'essay-cantonese.txt': 'essay.txt', 'hkcancor-word-counts.txt': 'corpus.txt'}.get(basename)
        if name and any(item[key] != actual[name][key] for key in ['size', 'sha256']):
            raise ValueError('词库仓来源锁与输入不符：' + name)
    locked = json.loads((ROOT / 'resources/language-dictionaries.lock.json').read_text(encoding='utf-8'))
    baseline = next(item for item in locked['artifacts'] if item['name'] == 'msime-cantonese.db')
    if any(baseline[key] != actual['baseline.db'][key] for key in ['size', 'sha256']):
        raise ValueError('发布词库锁已变化，需更新基线')
    return manifest


def rows(path):
    body = False
    for line in path.read_text(encoding='utf-8').splitlines():
        if line == '...':
            body = True
        elif body and line and not line.startswith('#'):
            yield line.split('\t')


def toneless(reading):
    result = tuple(re.sub(r'[1-6]$', '', part) for part in reading.split(' '))
    if not result or any(not re.fullmatch('[a-z]+', part) for part in result):
        raise ValueError('非法粤拼标注')
    return result


def counts(path):
    result = {}
    for line in path.read_text(encoding='utf-8').splitlines():
        if line and not line.startswith('#'):
            text, count = line.split('\t')
            result[text] = max(result.get(text, 0), int(count))
    return result


def han_word(text):
    return len(text) >= 2 and all(0x3400 <= ord(c) <= 0x4dbf or 0x4e00 <= ord(c) <= 0x9fff
                                 or 0xf900 <= ord(c) <= 0xfaff
                                 or 0x20000 <= ord(c) <= 0x323af for c in text)


def evaluate(directory, output):
    readings = collections.defaultdict(set)
    shares = collections.defaultdict(dict)
    for text, reading, *share in rows(directory / 'chars.yaml'):
        key = toneless(reading)
        readings[text].add(key)
        # 无百分比的行按现有构建器的缺省完整字频处理；声调合并后保留最大份额。
        shares[text][key] = max(shares[text].get(key, 0), int(share[0][:-1]) if share else 100)
    words = collections.defaultdict(set)
    for text, reading in rows(directory / 'words.yaml'):
        words[text].add(toneless(reading))
    essay, corpus = counts(directory / 'essay.txt'), counts(directory / 'corpus.txt')
    database = sqlite3.connect((directory / 'baseline.db').resolve().as_uri() + '?mode=ro', uri=True)
    entries = list(database.execute('SELECT key, text, weight FROM entries'))
    baseline_texts = {text for _, text, _ in entries}
    preferred = {char: min(options, key=lambda key: (-options[key], key)) for char, options in shares.items()}

    def annotate(text, strict):
        keys = []
        for char in text:
            options = readings.get(char, set())
            if not options or strict and (len(options) != 1 or len(next(iter(options))) != 1):
                return None
            key = next(iter(options)) if strict else preferred[char]
            if len(key) != 1:
                return None
            keys.extend(key)
        return tuple(keys)

    consistency = {}
    for label, strict in [('single_reading', True), ('most_common_reading', False)]:
        eligible = {text: annotate(text, strict) for text in words if han_word(text)}
        eligible = {text: key for text, key in eligible.items() if key is not None}
        mismatch = {text: key for text, key in eligible.items() if key not in words[text]}
        consistency[label] = {'eligible_word_types': len(eligible), 'mismatched_word_types': len(mismatch),
                              'mismatch_percent': round(len(mismatch) * 100 / len(eligible), 4)}
    scenarios = []
    for threshold in [1000, 100, 1]:
        missing = {text: weight for text, weight in essay.items()
                   if weight >= threshold and han_word(text) and text not in baseline_texts}
        additions = [(' '.join(key), text, weight) for text, weight in sorted(missing.items())
                     if (key := annotate(text, True)) is not None]
        # 同一 SQLite 版本、相同结构、相同压实步骤，只改变实验补词条目。
        path = output / f'single-reading-{threshold}.db'
        shutil.copyfile(directory / 'baseline.db', path)
        with sqlite3.connect(path) as target:
            target.executemany('INSERT INTO entries(key,text,weight) VALUES(?,?,?)', additions)
            target.execute('ANALYZE')
            target.commit()
            target.execute('VACUUM')
        scenarios.append({'minimum_essay_score': threshold, 'missing_word_types': len(missing),
                          'single_reading_additions': len(additions), 'sqlite_bytes': path.stat().st_size})
    control_size = scenarios[0]['sqlite_bytes']
    if scenarios[0]['single_reading_additions'] != 0:
        raise ValueError('高门槛对照与发布库不一致，不能继续报告增量')
    for scenario in scenarios:
        scenario['growth_against_control_bytes'] = scenario['sqlite_bytes'] - control_size
    probes = []
    for text in ['上傳', '我打你', '我打', '打你', '你好', '我哋', '食咗', '銀行', '行路', '長大', '長度']:
        probes.append({'text': text, 'baseline': database.execute('SELECT key,weight FROM entries WHERE text=? ORDER BY key', (text,)).fetchall(),
                       'essay_score': essay.get(text, 0), 'corpus_count': corpus.get(text, 0),
                       'single_reading_key': annotate(text, True), 'most_common_key': annotate(text, False)})
    report = {'measurement': {'sqlite_version': sqlite3.sqlite_version,
                              'baseline_page_size': database.execute('PRAGMA page_size').fetchone()[0]},
              'baseline': {'bytes': (directory / 'baseline.db').stat().st_size,
                           'entry_rows': len(entries), 'character_rows': sum(len(text) == 1 for _, text, _ in entries),
                           'word_rows': sum(len(text) > 1 for _, text, _ in entries), 'unique_texts': len(baseline_texts)},
              'inputs': {'characters': len(readings), 'words_file_word_types': len(words),
                         'essay_han_word_types': sum(han_word(text) for text in essay),
                         'corpus_word_types': len(corpus), 'corpus_tokens_in_word_counts': sum(corpus.values())},
              'same_source_consistency_not_error_rate': consistency, 'single_reading_scenarios': scenarios, 'probes': probes}
    database.close()
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, default=ROOT / 'target/cantonese-evaluation')
    parser.add_argument('--output', type=Path, default=ROOT / 'target/cantonese-evaluation/results')
    parser.add_argument('--fetch', action='store_true', help='缺失时下载并校验固定公开输入')
    args = parser.parse_args()
    verified_inputs(args.input, args.fetch)
    args.output.mkdir(parents=True, exist_ok=True)
    report = evaluate(args.input, args.output)
    result = json.dumps(report, ensure_ascii=False, indent=2) + '\n'
    (args.output / 'report.json').write_text(result, encoding='utf-8')
    print(result)


if __name__ == '__main__':
    main()
