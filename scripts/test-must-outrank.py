#!/usr/bin/env python3
"""检查 scripts/must_outrank.py 的判定规则和提交的 resources/eval/must-outrank.tsv。

判定只看两个词在候选表里的名次：outrank 要应靠前的词出现并且排在前面，not-on-first-page 要不该出现的词不在首屏、而锚定词在首屏。夹具的每一行都要能解析、能在声明的键盘上打、带来源。不需要词库或评测二进制。
"""
import importlib.util
import json
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("must_outrank", ROOT / "scripts/must_outrank.py")
must_outrank = importlib.util.module_from_spec(spec)
spec.loader.exec_module(must_outrank)


def row(kind, keyboard, case_input, higher, lower, case_id="t"):
    return {"id": case_id, "kind": kind, "keyboard": keyboard, "input": case_input, "should_rank_higher": higher, "should_rank_lower": lower, "source": "test"}


outrank = row("outrank", "full", "chejian", "车检", "车件")
assert must_outrank.judge(outrank, ["车检", "车件"], 6)["passed"]
# 应靠后的词不在候选表里也算过。
assert must_outrank.judge(outrank, ["车检"], 6)["passed"]
failed = must_outrank.judge(outrank, ["车件", "车检"], 6)
assert not failed["passed"] and (failed["higher_rank"], failed["lower_rank"]) == (2, 1)
# 应靠前的词不出现是失败，不是空过。
missing = must_outrank.judge(outrank, ["车件"], 6)
assert not missing["passed"] and "not in the candidates" in missing["reason"]

hidden = row("not-on-first-page", "nine", "63476", "内容", "Negro")
assert must_outrank.judge(hidden, ["内容", "内人", "1", "2", "3", "4", "Negro"], 6)["passed"]
# 英文词不分大小写；在首屏第 2 位是失败。
assert not must_outrank.judge(hidden, ["内容", "negro"], 6)["passed"]
# 首屏大小跟着参数走：每页 9 个时第 7 位也在首屏。
assert not must_outrank.judge(hidden, ["内容", "内人", "1", "2", "3", "4", "Negro"], 9)["passed"]
# 锚定词不在首屏，说明输入写错了或候选整个变了，不能算过。
anchor_gone = must_outrank.judge(hidden, ["内人"], 6)
assert not anchor_gone["passed"] and "may be wrong" in anchor_gone["reason"]
# 不写锚定词时只看不该出现的词。
assert must_outrank.judge(row("not-on-first-page", "nine", "63476", "-", "Negro"), ["内人"], 6)["passed"]
# 汉字逐字比较，不做大小写或部分匹配。
assert must_outrank.rank_of(["车检站", "车检"], "车检") == 2

# 交给 convert_eval 的字母：九键数字换成编码相同的字母，14 键只收组码。
assert must_outrank.typed_letters(row("outrank", "nine", "2435426", "车检", "车件")) == "agdjgam"
assert must_outrank.typed_letters(row("outrank", "full", "huo'zai", "火灾", "或在")) == "huo'zai"
for bad in (row("outrank", "nine", "che", "a", "b"), row("outrank", "nine", "0123", "a", "b"), row("outrank", "fourteen", "chejian", "a", "b"), row("outrank", "full", "Che", "a", "b")):
    try:
        must_outrank.typed_letters(bad)
        raise AssertionError(f"{bad['input']} on {bad['keyboard']} must be rejected")
    except ValueError:
        pass

with tempfile.TemporaryDirectory(prefix="msime-must-outrank-") as temporary:
    root = Path(temporary)
    # 夹具校验：缺来源、kind 写错、重复 id 都在跑之前报错。
    header = "id\tkind\tkeyboard\tinput\tshould_rank_higher\tshould_rank_lower\tsource\n"
    for body, message in (
        ("a\toutrank\tfull\tchejian\t车检\t车件\t\n", "source"),
        ("a\toutranks\tfull\tchejian\t车检\t车件\tx\n", "kind"),
        ("a\toutrank\tfull\tchejian\t车检\t车件\tx\na\toutrank\tfull\tchejian\t车检\t车件\tx\n", "duplicate"),
        ("a\toutrank\tfull\tchejian\t-\t车件\tx\n", "should_rank_higher"),
        ("a\toutrank\tnine\tchejian\t车检\t车件\tx\n", "digits"),
    ):
        fixture = root / "fixture.tsv"
        fixture.write_text(header + body, encoding="utf-8")
        try:
            must_outrank.load_fixture(fixture)
            raise AssertionError(f"fixture must be rejected: {body!r}")
        except SystemExit as error:
            assert message in str(error), (message, str(error))

    # check 按键盘取 dump、按 id 取候选表；dump 缺这一行时报错。
    rows = [row("outrank", "full", "chejian", "车检", "车件", "f1"), row("not-on-first-page", "nine", "63476", "内容", "Negro", "n1")]
    full_dump, nine_dump = root / "full.jsonl", root / "nine.jsonl"
    full_dump.write_text(json.dumps({"id": "f1", "input": "chejian", "gold": "车检", "candidates": [{"text": "车件", "source": 0}, {"text": "车检", "source": 0}]}, ensure_ascii=False) + "\n", encoding="utf-8")
    nine_dump.write_text(json.dumps({"id": "n1", "input": "mdgpm", "gold": "内容", "candidates": [{"text": "内容", "source": 0}, {"text": "Negro", "source": 4}]}, ensure_ascii=False) + "\n", encoding="utf-8")
    results = must_outrank.check(rows, {"full": must_outrank.eval_keys.load_dump(full_dump), "nine": must_outrank.eval_keys.load_dump(nine_dump)}, 6)
    assert [r["passed"] for r in results] == [False, False]
    assert must_outrank.print_results(results) == 1
    try:
        must_outrank.check(rows, {"full": {}, "nine": {}}, 6)
        raise AssertionError("a dump without the row must fail")
    except SystemExit as error:
        assert "not in the full dump" in str(error)

    # 用例集：每种键盘一份，输入是字母，金标准是应靠前的词。
    case_set = root / "nine.tsv"
    must_outrank.write_case_set([rows[1]], case_set)
    assert case_set.read_text(encoding="utf-8").splitlines()[-1] == "n1\tmdgpm\t内容\t2\tnot-on-first-page"

# 提交的夹具能解析，覆盖方案列出的第一批。
committed = must_outrank.load_fixture(ROOT / "resources/eval/must-outrank.tsv")
pairs = {(r["keyboard"], r["input"], r["should_rank_higher"], r["should_rank_lower"]) for r in committed}
for expected in (("full", "chejian", "车检", "车件"), ("nine", "2435426", "车检", "车件"), ("nine", "63633", "闷的", "门的"), ("full", "huo'zai", "火灾", "或在"), ("nine", "9269468", "万州", "挽昼"), ("nine", "63476", "内容", "Negro")):
    assert expected in pairs, expected
# 九键的数字应当就是应靠前的词的读音在九键上的码（63476 是 Negro 的码，内容 在那里是补全）。
assert must_outrank.eval_keys.encode("nine", "che'jian") == "2435426"
assert must_outrank.eval_keys.encode("nine", "men'de") == "63633"
assert must_outrank.eval_keys.encode("nine", "wan'zhou") == "9269468"
assert must_outrank.eval_keys.encode("nine", "negro") == "63476"

print("must-outrank: ok")
