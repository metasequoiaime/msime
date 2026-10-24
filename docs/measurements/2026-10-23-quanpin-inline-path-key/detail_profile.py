from collections import Counter
from pathlib import Path
import json
import re
import runpy
import subprocess

folder = Path('target/next-hotspot-evidence')
analysis = runpy.run_path(str(Path(__file__).with_name('analyze_profile.py')))
nodes = analysis['nodes']
rules = analysis['rules']
symbols = sorted({match.group() for node in nodes if (match := re.match(r'_R\w+', node['frame']))})
result = subprocess.run(['c++filt'], input='\n'.join(symbols) + '\n', text=True, capture_output=True, check=True)
demangled = dict(zip(symbols, result.stdout.splitlines(), strict=True))
for node in nodes:
    frame = node['frame'].split('  (in ', 1)[0]
    node['name'] = demangled.get(frame, frame)
sections = [('runtime', r'msime_input_runtime'), ('nine_key', r'NineKeySession'),
            ('quanpin', r'QuanpinDictionary'), ('decode', r'decode_lattice|decode_graph|build_graph'),
            ('model', r'chinese_ime_lm'), ('bigram', r'BigramMemo'),
            ('ngram', r'NgramTable'), ('english', r'EnglishDictionary')]
allocation = re.compile(r'malloc|free|alloc.*(?:reserve|grow)|drop_glue')
inclusive, owners, self_frames, callers = Counter(), Counter(), Counter(), Counter()
for index, node in enumerate(nodes):
    exclusive = node['count'] - node['children']
    if not exclusive:
        continue
    chain = []
    cursor = index
    while cursor is not None:
        chain.append(nodes[cursor]['name'])
        cursor = nodes[cursor]['parent']
    if not any('type_case' in frame for frame in chain):
        continue
    self_frames[chain[0]] += exclusive
    for name, pattern in sections:
        if any(re.search(pattern, frame) for frame in chain):
            inclusive[name] += exclusive
    # 与主分析同样按最近已知帧分类；只有分配归属再向上寻找业务入口。
    raw_chain = []
    cursor = index
    while cursor is not None:
        raw_chain.append(nodes[cursor]['frame'])
        cursor = nodes[cursor]['parent']
    category = None
    for frame in raw_chain:
        category = next((name for name, pattern in rules if re.search(pattern, frame)), None)
        if category:
            break
    if category != 'allocation':
        continue
    allocation_index = -1
    owner = 'unclassified'
    for frame in chain[allocation_index + 1:]:
        found = next((name for name, pattern in sorted(sections, key=lambda pair: ['quanpin', 'nine_key', 'decode', 'model', 'runtime', 'english', 'bigram', 'ngram'].index(pair[0])) if re.search(pattern, frame)), None)
        if found:
            owner = found
            break
    owners[owner] += exclusive
    # 泛型名称内部含 alloc:: 时仍保留实际引擎帧。
    relevant = tuple(frame for frame in chain if frame.startswith(('<msime_engine::', 'msime_engine::')))
    callers[relevant[:4]] += exclusive
payload = {'type_case_samples': sum(self_frames.values()), 'inclusive_sections_overlap': dict(inclusive),
           'allocation_nearest_owner': dict(owners.most_common()), 'top_self_frames': self_frames.most_common(40)}
assert analysis['counts']['allocation'] == sum(owners.values())
assert sum(self_frames.values()) == sum(analysis['counts'].values())
(folder / 'profile-detail-reproduced.json').write_text(json.dumps(payload, ensure_ascii=False, indent=2) + '\n')
(folder / 'allocation-callers-reproduced.json').write_text(json.dumps([{'samples': count, 'callers': chain} for chain, count in callers.most_common(40)], ensure_ascii=False, indent=2) + '\n')
print(json.dumps(payload['allocation_nearest_owner']))
