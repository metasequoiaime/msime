#!/usr/bin/env python3
"""检查 scripts/build-quanpin-words-v2.py 的判据和提交的 resources/eval/quanpin-words-v2.tsv。

码表必须与引擎的 `KEYPAD`、`FOURTEEN_KEYPAD` 逐字相同，否则同码标签和引擎实际打出的码对不上。提交的 v2 必须是 v1 的有序子集（id、输入、金标准、音节数原样），同码标签按留下的行重新算一遍要与文件里的相同，文件头的计数要与行数相符。不需要词库：杂质三类的判定只用合成读音核对，重新生成要参照词库，见脚本的文件头。
"""
import importlib.util
import re
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("build_v2", ROOT / "scripts/build-quanpin-words-v2.py")
build_v2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build_v2)
import eval_keys  # noqa: E402  与 build_v2 同目录，exec_module 时已经能找到

nine_key = (ROOT / "crates/engine/src/nine_key.rs").read_text(encoding="utf-8")
assert f'const KEYPAD: &[u8; 26] = b"{eval_keys.NINE_KEYPAD}";' in nine_key, "NINE_KEYPAD drifted from nine_key.rs"
assert f'const FOURTEEN_KEYPAD: &[u8; 26] = b"{eval_keys.FOURTEEN_KEYPAD}";' in nine_key, "FOURTEEN_KEYPAD drifted from nine_key.rs"
assert eval_keys.encode("nine", "che'jian") == "2435426"
assert eval_keys.encode("full", "che'jian") == "che'jian"
assert eval_keys.encode("fourteen", "bugao") == eval_keys.encode("fourteen", "nihao")
# 九键数字换回字母，再编码回来是同一串；0、1 打不出字母。
assert eval_keys.encode("nine", eval_keys.nine_digits_to_letters("2435426")) == "2435426"
try:
    eval_keys.nine_digits_to_letters("1234")
    raise AssertionError("1 has no letters")
except ValueError:
    pass

# lue/nue 与 lve/nve 同读；三类杂质各一条，加一条干净的。
assert build_v2.canonical("zhan'lve") == build_v2.canonical("zhanlue") == "zhanlve"
readings = {"策略": {"celve"}, "爱憎分明": {"aizengfenming"}, "呆板": {"daiban"}}
assert build_v2.classify("celue", "策略", readings) is None
assert build_v2.classify("aizengfenmin", "爱憎分明", readings) == "truncated"
assert build_v2.classify("aiban", "呆板", readings) == "reading-mismatch"
assert build_v2.classify("bihua", "比划", readings) == "missing"
# 截断落在 lue 中间（…lu 是 …lue 的前缀，不是 …lve 的）仍算截断；lu 打 驴（lv）是读音不符，不能被这条规则放过。
assert build_v2.classify("fazhanzhanlu", "发展战略", {"发展战略": {"fazhanzhanlve"}}) == "truncated"
assert build_v2.classify("xiongcaidalu", "雄才大略", {"雄才大略": {"xiongcaidalve"}}) == "truncated"
assert build_v2.classify("lu", "驴", {"驴": {"lv"}}) == "reading-mismatch"

# 同码：输入相同、金标准不同才算；同一个词重复出现不算。九键上 西安/一按 这种全拼不同的也算。
rows = [["1", "shiyan", "实验", "2", ""], ["2", "shiyan", "试验", "2", ""], ["3", "xian", "西安", "2", ""], ["4", "yian", "一按", "2", ""], ["5", "aba", "阿爸", "2", ""], ["6", "aba", "阿爸", "2", ""]]
tags = build_v2.same_code_tags(rows)
assert tags["1"] == ["same-code-full", "same-code-nine", "same-code-fourteen"]
assert tags["3"] == ["same-code-nine"] and tags["4"] == ["same-code-nine"]
assert "5" not in tags and "6" not in tags
assert build_v2.upper_bound(rows, "full") == 5 / 6

# 提交的 v2 与 v1 一致。
_, v1_rows = build_v2.load_v1(ROOT / "resources/eval/quanpin-words-v1.tsv")
v2_header, v2_rows = build_v2.load_v1(ROOT / "resources/eval/quanpin-words-v2.tsv")
v1_index = {row[0]: index for index, row in enumerate(v1_rows)}
positions = [v1_index[row[0]] for row in v2_rows]
assert positions == sorted(positions), "v2 must keep v1's order"
for row in v2_rows:
    assert row[:4] == v1_rows[v1_index[row[0]]][:4], row
recomputed = build_v2.same_code_tags(v2_rows)
for row in v2_rows:
    assert row[4] == ",".join(recomputed.get(row[0], [])), row
header = "\n".join(v2_header)
counts = re.search(r"# v1=(\d+) kept=(\d+) truncated=(\d+) reading-mismatch=(\d+) missing=(\d+)", header)
assert counts, "v2 header lost its counts"
v1_total, kept, *removed = map(int, counts.groups())
assert v1_total == len(v1_rows) and kept == len(v2_rows) and kept + sum(removed) == v1_total
# 冻结集的参照词库写死在 eval_keys，不随资源锁前进；v2 的文件头要与它一致。
reference = f"# reference-dictionary: {eval_keys.REFERENCE_RELEASE} msime-pinyin.db sha256={eval_keys.REFERENCE_PINYIN_SHA256}"
assert reference in v2_header, "v2 header must record the pinned reference dictionary"
# 别的词库（比如锁升级后的新版）不能拿来重新生成冻结集。
with tempfile.TemporaryDirectory(prefix="msime-v2-reference-") as temporary:
    other = Path(temporary) / "msime-pinyin.db"
    other.write_bytes(b"not the reference")
    try:
        eval_keys.check_reference_pinyin(other)
        raise AssertionError("a different msime-pinyin.db must be rejected")
    except SystemExit as error:
        assert eval_keys.REFERENCE_RELEASE in str(error)

print("quanpin-words-v2: ok")
