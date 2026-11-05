#!/usr/bin/env python3
"""从固定的香港官方名称及审校读音生成两种方案的补充表；不联网、不猜多音字。"""

from __future__ import annotations

import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import re
import subprocess

DIRECTORY = Path('sources/hong-kong')
OUTPUT = DIRECTORY / 'landsd-readings.tsv'
RULES = Path('curation/hong-kong-readings.tsv')
FORMS = Path('curation/hong-kong-forms.tsv')


def han(text: str) -> bool:
    return bool(text) and all(
        '\u3400' <= c <= '\u4dbf' or '\u4e00' <= c <= '\u9fff'
        or '\U00020000' <= c <= '\U000323af' for c in text
    )


def lines(path: Path):
    for number, line in enumerate(path.read_text(encoding='utf-8-sig').splitlines(), 1):
        if line and not line.startswith('#'):
            yield number, line.split('\t')


def names_from_snapshot(dictionary: Path) -> dict[str, list[dict]]:
    root = dictionary / DIRECTORY
    manifest = json.loads((root / 'sources.json').read_text())
    expected = {'landsd-place-names.geojson', 'landsd-street-names.json', 'CSDI-terms.txt'}
    if {f['path'] for f in manifest['files']} != expected:
        raise ValueError('来源清单必须完整列出街道和地方名称两份快照')
    for item in manifest['files']:
        data = (root / item['path']).read_bytes()
        if len(data) != item['size'] or hashlib.sha256(data).hexdigest() != item['sha256']:
            raise ValueError(f"来源快照摘要不符：{item['path']}")
    result = defaultdict(list)
    for row in json.loads((root / 'landsd-street-names.json').read_text()):
        row = row['attributes']
        if row['CHINESESTREETNAME']:
            result[row['CHINESESTREETNAME']].append({'kind': 'street', 'english': row['ENGLISHSTREETNAME']})
    for row in json.loads((root / 'landsd-place-names.geojson').read_text())['features']:
        row = row['properties']
        if row['NAME_TC']:
            result[row['NAME_TC']].append({'kind': 'place', 'english': row['NAME_EN'], 'status': row['NAME_STATUS']})
    return dict(sorted(result.items()))


def conversion(dictionary: Path) -> dict[str, str]:
    result = {}
    for _, fields in lines(dictionary / 'sources/opencc/TSCharacters.txt'):
        result[fields[0]] = fields[1].split()[0]
    # 香港和台湾异体先回到 OpenCC 基准，再简化；原始繁体名始终另存。
    for filename in ['HKVariants.txt', 'TWVariants.txt']:
        for _, fields in lines(dictionary / 'sources/opencc' / filename):
            for variant in fields[1].split():
                result.setdefault(variant, result.get(fields[0], fields[0]))
    return result


def lexicons(dictionary: Path):
    yue = defaultdict(set)
    for filename in ['jyut6ping3.chars.dict.yaml', 'jyut6ping3.words.dict.yaml']:
        for _, fields in lines(dictionary / 'sources/cantonese' / filename):
            if len(fields) >= 2 and han(fields[0]) and re.fullmatch(r'[a-z1-6 ]+', fields[1]):
                reading = re.sub('[1-6]', '', fields[1])
                if len(reading.split()) == len(fields[0]):
                    yue[fields[0]].add(reading)
    cmn = defaultdict(set)
    for filename in ['single-chars.txt', 'rime-ice.txt', 'rime-ice-supplement.txt', 'places.txt']:
        for _, fields in lines(dictionary / 'sources/pinyin' / filename):
            if len(fields) >= 2 and han(fields[0]) and re.fullmatch(r"[a-z]+(?:'[a-z]+)*", fields[1]):
                if len(fields[1].split("'")) == len(fields[0]):
                    cmn[fields[0]].add(fields[1].replace("'", ' '))
    return {'yue': yue, 'cmn': cmn}


def load_rules(dictionary: Path, lex: dict) -> dict:
    result = {'yue': {}, 'cmn': {}}
    path = dictionary / RULES
    if not path.exists():
        return result
    for number, fields in lines(path):
        if len(fields) not in (3, 4):
            raise ValueError(f'{RULES}:{number}: 需要方案、字词、去声调读音三列')
        scheme, word, reading = fields[:3]
        if scheme not in result or not han(word) or len(word) != len(reading.split()):
            raise ValueError(f'{RULES}:{number}: 方案或字词与音节数错误')
        if word in result[scheme] or not re.fullmatch('[a-z]+(?: [a-z]+)*', reading):
            raise ValueError(f'{RULES}:{number}: 重复或非法读音')
        for char, syllable in zip(word, reading.split(), strict=True):
            if char not in lex[scheme] and len(word) == 1 and len(fields) == 4 and fields[3].startswith('https://'):
                continue
            if syllable not in lex[scheme].get(char, set()):
                raise ValueError(f'{RULES}:{number}: {char} 的 {syllable} 不在固定字表内')
        result[scheme][word] = reading
    return result


def resolve(name: str, lex: dict, rules: dict) -> tuple[str | None, list]:
    result, issues = [], []
    pos = 0
    while pos < len(name):
        # 地名审校词优先于通用词库，即使通用词库的匹配更长。
        word = next((name[pos:end] for end in range(len(name), pos, -1)
                     if end - pos > 1 and name[pos:end] in rules), None)
        if word is not None:
            values = {rules[word]}
        else:
            word = next((name[pos:end] for end in range(len(name), pos, -1)
                         if name[pos:end] in lex or name[pos:end] in rules), name[pos])
            values = {rules[word]} if len(word) == 1 and word in rules else lex.get(word, set())
        if len(values) != 1:
            issues.append({'span': word, 'readings': sorted(values), 'offset': pos})
        else:
            result.append(next(iter(values)))
        pos += len(word)
    return (None if issues else ' '.join(result)), issues


def normalize_name(name: str) -> str:
    # 官方表里的排版空白、分段横线和方向括号不参与读音；字母与私用字不删。
    name = name.strip().replace('　－　', '').replace(' (北)', '北').replace(' (南)', '南')
    return name.translate(str.maketrans('１２３４５', '一二三四五'))


def generate(dictionary: Path) -> tuple[list[str], dict]:
    names = names_from_snapshot(dictionary)
    lex = lexicons(dictionary)
    rules = load_rules(dictionary, lex)
    variants = conversion(dictionary)
    forms = dict(fields for _, fields in lines(dictionary / FORMS)) if (dictionary / FORMS).exists() else {}
    rows, audit = [], {'input_names': len(names), 'ready': [], 'pending': [], 'unsupported': [], 'normalized': []}
    for original, provenance in names.items():
        name = normalize_name(original)
        if name != original:
            audit['normalized'].append({'original': original, 'name': name})
        if not han(name) or len(name) < 2:
            audit['unsupported'].append({'name': name, 'source': provenance, 'reason': '不是两个及以上纯汉字的名称；保留原文，不删除标记或猜测私用字'})
            continue
        simplified = forms.get(name, ''.join(variants.get(c, c) for c in name))
        yue, yue_issues = resolve(name, lex['yue'], rules['yue'])
        cmn, cmn_issues = resolve(simplified, lex['cmn'], rules['cmn'])
        if yue_issues or cmn_issues:
            audit['pending'].append({'name': name, 'simplified': simplified, 'source': provenance, 'yue': yue_issues, 'cmn': cmn_issues})
            continue
        row = '\t'.join([name, simplified, yue, cmn.replace(' ', "'")])
        if row not in rows:
            rows.append(row)
        audit['ready'].append(name)
    audit['counts'] = {k: len(audit[k]) for k in ['ready', 'pending', 'unsupported']}
    assert sum(audit['counts'].values()) == len(names)
    return rows, audit


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dictionary', type=Path, required=True)
    parser.add_argument('--audit', type=Path, required=True)
    parser.add_argument('--check', action='store_true', help='核对补充表与当前固定输入一致')
    parser.add_argument('--write', action='store_true', help='仅在没有待审读音时写出补充表')
    args = parser.parse_args()
    rows, audit = generate(args.dictionary)
    args.audit.write_text(json.dumps(audit, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(audit['counts'], ensure_ascii=False))
    if args.check:
        actual = [line for line in (args.dictionary / OUTPUT).read_text().splitlines() if line and not line.startswith('#')]
        if actual != rows or audit['pending']:
            raise SystemExit('补充表与输入不一致或仍有待审读音')
    if args.write:
        if audit['pending']:
            raise SystemExit('仍有未确认读音，请先审校；没有写出补充表')
        root = Path(__file__).resolve().parents[2]
        commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
        if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=root):
            raise SystemExit('生成器工作区有未提交改动；请先提交再生成')
        header = f'# msime 仓库提交 {commit}\n# 来源、摘要与使用条款见 sources.json；读音规则见 ../../curation/hong-kong-readings.tsv。\n# 生成：python3 crates/dict-builder/hong_kong_places.py --dictionary <checkout> --audit <audit.json> --write\n# 原始中文名去重；不删除与基础词表重合的行，构建时仅插入不存在的同键同词。\n# 香港繁体名称\t简体名称\t去声调粤拼（空格）\t普通话全拼（单引号）\n'
        (args.dictionary / OUTPUT).write_text(header + '\n'.join(rows) + '\n')
        data = (args.dictionary / OUTPUT).read_bytes()
        path = args.dictionary / DIRECTORY / 'sources.json'
        manifest = json.loads(path.read_text())
        audit_data = (json.dumps(audit, ensure_ascii=False, indent=2) + '\n').encode()
        (args.dictionary / DIRECTORY / 'audit.json').write_bytes(audit_data)
        manifest['generated'] = {'audit_sha256': hashlib.sha256(audit_data).hexdigest(), 'path': OUTPUT.name, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'msime_commit': commit, 'rows': len(rows), 'counts': audit['counts']}
        path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
