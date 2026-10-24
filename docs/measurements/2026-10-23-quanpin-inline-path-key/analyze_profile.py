from pathlib import Path
from collections import Counter
import json
import re

folder = Path('target/next-hotspot-evidence')
source = (folder / 'profile.sample.txt').read_text()
graph = source.split('Call graph:\n', 1)[1].split('Total number in stack', 1)[0]
nodes = []
stack = []
for line in graph.splitlines():
    match = re.match(r'^([ +!:|]*)(\d+) (.+)$', line)
    if not match:
        continue
    depth, count, frame = len(match[1]), int(match[2]), match[3]
    while stack and nodes[stack[-1]]['depth'] >= depth:
        stack.pop()
    parent = stack[-1] if stack else None
    nodes.append({'depth': depth, 'count': count, 'frame': frame,
                  'parent': parent, 'children': 0})
    index = len(nodes) - 1
    if parent is not None:
        nodes[parent]['children'] += count
    stack.append(index)

# 独占样本只归到最深的一个已知入口，初始化与逐键路径分开累计。
rules = [('model_linear', r'chinese_ime_lm.*(?:linear_into|dot_i8|dot_row)'),
         ('bigram_memo', r'BigramMemo.*score'),
         ('ngram_lookup', r'NgramTable.*lookup'),
         ('allocation', r'malloc|free|alloc.*(?:reserve|grow)|drop_glue'),
         ('copy_or_compare', r'memcpy|memmove|memcmp|memset'),
         ('lattice_decode', r'decode_graph|lattice.*cxx_sort'),
         ('dictionary', r'QuanpinDictionary|EnglishDictionary|sqlite3'),
         ('nine_key', r'NineKeySession|nine_key.*(?:push_ranked|rank_key)'),
         ('model_other', r'chinese_ime_lm'),
         ('runtime', r'msime_input_runtime')]
counts = Counter()
outside = 0
for index, node in enumerate(nodes):
    exclusive = node['count'] - node['children']
    assert exclusive >= 0, node
    chain = []
    cursor = index
    while cursor is not None:
        chain.append(nodes[cursor]['frame'])
        cursor = nodes[cursor]['parent']
    if not any('type_case' in frame for frame in chain):
        outside += exclusive
        continue
    category = 'unclassified'
    for frame in chain:
        found = next((name for name, pattern in rules if re.search(pattern, frame)), None)
        if found is not None:
            category = found
            break
    counts[category] += exclusive
root_samples = sum(node['count'] for node in nodes if node['parent'] is None)
assert root_samples == outside + sum(counts.values())
result = {'all_thread_samples': root_samples, 'outside_type_case': outside,
          'type_case_samples': sum(counts.values()),
          'exclusive_samples_by_nearest_known_frame': dict(counts.most_common()),
          'scope': '两臂混合、进程启动后最多60秒，包含预热；采样和外部构建下只作诊断。'}
(folder / 'profile-analysis.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
print(json.dumps(result, ensure_ascii=False, indent=2))
