#!/usr/bin/env python3
"""检查 scripts/vulgar_exposure.py：粗俗词判定与引擎 `lattice::offensive` 同一规则，部分输入的生成，曝光计数和基线比较，以及提交的两份输入集。

不需要词库或评测二进制：判定和计数用合成的候选表，输入集只核对格式、去重和目标码。
"""
import importlib.util
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("vulgar_exposure", ROOT / "scripts/vulgar_exposure.py")
exposure = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exposure)
eval_keys = exposure.eval_keys

# 两份输入集与 quanpin-words-v2 同一份参照词库（eval_keys 写死），文件头要记着它。
reference = f"# reference-dictionary: {eval_keys.REFERENCE_RELEASE} msime-pinyin.db sha256={eval_keys.REFERENCE_PINYIN_SHA256}"
for frozen in exposure.SETS.values():
    assert reference in frozen.read_text(encoding="utf-8").splitlines(), frozen

words = exposure.load_word_list([])
# 引擎词表：子串命中，豁免写法整个盖住的不算，单字不查（decode_offensive_tests.rs 用的同一批例子）。
assert words.is_offensive("鸡巴") and words.is_offensive("别鸡巴") and words.is_offensive("是傻逼")
assert not words.is_offensive("水乳交融") and not words.is_offensive("口交换机") and not words.is_offensive("狗吃屎")
assert words.is_offensive("吃屎了")
assert not words.is_offensive("屎") and not words.is_offensive("内容")
# 英文蔑称保留构建器清单，并读取引擎新增的粗俗词表，不分大小写。
english = exposure.english_offensive(exposure.ENGLISH_OFFENSIVE_SOURCE)
assert len(english) == 55 and "chink" in english and "yids" in english
assert words.is_offensive_english("Chink") and words.is_offensive_english("Negro")
assert words.is_offensive_english("Shitty") and words.is_offensive_english("gangbang")
assert not words.is_offensive_english("cocktail")
# 轻度粗口只在歧义码上计入，26 键仍按原政策处理。
assert not words.is_offensive("牛逼")
assert exposure.load_word_list([], "nine").is_offensive("牛逼")
assert not exposure.load_word_list([], "nine").is_offensive("我操心")

with tempfile.TemporaryDirectory(prefix="msime-vulgar-exposure-") as temporary:
    root = Path(temporary)
    extra = root / "extra.txt"
    extra.write_text("# 测试用\n牛逼\nNegro\n!牛逼哄哄\n", encoding="utf-8")
    more = exposure.load_word_list([extra])
    assert more.is_offensive("牛逼") and not more.is_offensive("牛逼哄哄") and more.is_offensive_english("negro")

    # 同一份候选在全键盘保留轻度粗口，九键将其计入；新增英文表两者都计入。
    probe_cases = {"probe": {"input": "niu", "word": "牛", "kinds": ["prefix"]}}
    probe_dump = {"probe": {"input": "niu", "gold": "牛", "candidates": [
        {"text": "牛逼", "source": 0}, {"text": "Shitty", "source": eval_keys.ENGLISH_SOURCE}]}}
    for keyboard, expected in [("full", 0), ("nine", 1)]:
        measured = exposure.analyze(keyboard, probe_cases, probe_dump, exposure.load_word_list([], keyboard), 6)
        assert len(measured["hits"]) == expected
        assert len(measured["english_hits"]) == 1

    # 部分输入：打全、每个更短的前缀、前 k 个音节的首字母。
    inputs = exposure.partial_inputs("che'jian")
    assert ("chejian", "full") in inputs and ("chejia", "prefix") in inputs and ("c", "prefix") in inputs and ("cj", "initials") in inputs
    assert ("chejian", "prefix") not in inputs and len(inputs) == 1 + 6 + 1
    # 九键按数字去重：权重高的词先占住这串码，后来的只补类别。
    sets = exposure.build_inputs([("车检", 2, ["che'jian"]), ("别叫", 1, ["bie'jiao"])])
    assert sets["full"]["chejian"][1] == "车检" and "full" in sets["full"]["chejian"][2]
    assert sets["nine"]["243542"][0] == "chejia" and sets["nine"]["243542"][2] >= {"prefix"}
    assert sets["nine"]["2435422"][2] == {"target"}, "the plan's target codes come first"
    assert "2435422" not in sets["full"], "nine-key targets are not typed on the full keyboard"

    # 计数：首屏里的粗俗词（以及它是不是这串输入本身的读音）、英文冒犯词和第 2 位英文。
    cases = {
        "a": {"input": "biejia", "word": "别叫", "kinds": ["prefix"]},
        "b": {"input": "shabi", "word": "傻逼", "kinds": ["full", "target"]},
        "c": {"input": "neiro", "word": "内容", "kinds": ["prefix"]},
        "d": {"input": "chejian", "word": "车检", "kinds": ["full"]},
    }

    def candidates(*texts):
        return {"candidates": [{"text": text, "source": 4 if text.isascii() else 0} for text in texts]}

    dump = {
        "a": candidates("别叫", "别鸡巴", "别家"),
        "b": candidates("傻逼", "煞笔"),
        "c": candidates("内容", "Chink"),
        "d": candidates("车间", "车件", "1", "2", "3", "4", "鸡巴"),
    }
    for case_id, row in dump.items():
        row.update(input=cases[case_id]["input"], gold=cases[case_id]["word"])
    analysis = exposure.analyze("full", cases, dump, words, 6)
    # 同 id 换输入或金标准不是同一次测量，不能空过。
    for field in ("input", "gold"):
        altered = {**dump, "a": {**dump["a"], field: "synthetic-other"}}
        try:
            exposure.analyze("full", cases, altered, words, 6)
            raise AssertionError("mismatched dump identity must fail")
        except SystemExit as error:
            assert "input or gold" in str(error)
    duplicate = root / "duplicate.jsonl"
    duplicate.write_text('{"id":"a"}\n{"id":"a"}\n', encoding="utf-8")
    try:
        eval_keys.load_dump(duplicate)
        raise AssertionError("duplicate ids must fail")
    except SystemExit as error:
        assert "duplicate id" in str(error)
    exposure.mark_exact("full", analysis["hits"], {"傻逼": {"shabi"}, "别鸡巴": {"biejiba"}, "煞笔": {"shabi"}})
    summary = exposure.summarize("full", analysis, cases)
    assert summary["vulgar_first_page_inputs"] == 2, summary   # a、b；d 的鸡巴在第 7 位，不在首屏
    assert summary["vulgar_first_page_rows"] == 3 and summary["vulgar_top1_inputs"] == 1
    assert summary["vulgar_first_page_inputs_violating"] == 1, "b only shows the words typed at their own reading"
    assert summary["vulgar_first_page_inputs_by_kind"] == {"full": 1, "prefix": 1, "target": 1}
    assert summary["english_offensive_first_page_inputs"] == 1
    assert summary["english_second_slot_inputs"] == 1 and summary["english_second_slot_full_codes"] == 0 and summary["full_code_inputs"] == 2
    assert [target["code"] for target in analysis["targets"]] == ["shabi"]
    # 九键的 exact 按数字串比：9 键上 shabi 和别的读音同码也算这串输入本身。
    nine = exposure.analyze("nine", {"b": cases["b"]}, {"b": dump["b"]}, words, 6)
    exposure.mark_exact("nine", nine["hits"], {"傻逼": {"shabi"}, "煞笔": {"shabi"}})
    assert all(hit["exact"] for hit in nine["hits"])
    # 但九键上 exact 不豁免：歧义码的首屏不许有粗俗词，b 在九键上算违反政策。
    assert exposure.summarize("nine", nine, {"b": cases["b"]})["vulgar_first_page_inputs_violating"] == 1
    # dump 缺输入说明跑错了集合。
    try:
        exposure.analyze("full", cases, {"a": dump["a"]}, words, 6)
        raise AssertionError("missing inputs must fail")
    except SystemExit as error:
        assert "not in the dump" in str(error)

    # 基线比较：三项门槛任何一项增加就报；输入集换了不可比。
    def result(first_page, top1, english_count, digest="x"):
        return {"first_page_rows": 6, "keyboards": {"nine": {"summary": {"vulgar_first_page_inputs": first_page, "vulgar_top1_inputs": top1, "english_offensive_first_page_inputs": english_count, "set_sha256": digest}}}}

    assert exposure.compare_baseline(result(3, 1, 0), result(3, 1, 0)) == []
    narrower = {**result(0, 0, 0), "first_page_rows": 1}
    assert "first_page_rows" in exposure.compare_baseline(narrower, result(3, 1, 0))[0]
    assert exposure.compare_baseline(result(2, 0, 0), result(3, 1, 0)) == []
    assert exposure.compare_baseline(result(4, 1, 0), result(3, 1, 0)) == ["nine: vulgar_first_page_inputs 3 -> 4"]
    assert "input set changed" in exposure.compare_baseline(result(3, 1, 0, "y"), result(3, 1, 0))[0]
    assert "not measured" in exposure.compare_baseline({"first_page_rows": 6, "keyboards": {}}, result(3, 1, 0))[0]

# 提交的输入集：表头记着参照词库，id 和码都不重复，方案点名的目标码都在九键集里。
for keyboard, path in exposure.SETS.items():
    text = path.read_text(encoding="utf-8")
    assert "# reference-dictionary: dict-v" in text and "# derived-by: python3 scripts/vulgar_exposure.py inputs" in text
    cases = exposure.load_set(path)
    codes = [eval_keys.encode(keyboard, case["input"]) for case in cases.values()]
    assert len(codes) == len(set(codes)), f"{path.name} repeats a code"
    assert all(case["kinds"] for case in cases.values())
nine_codes = {eval_keys.encode("nine", case["input"]) for case in exposure.load_set(exposure.SETS["nine"]).values()}
for code in ("2435422", "435422", "74224", "63476", "6347664", "42642", "74488"):
    assert code in nine_codes, code

print("vulgar exposure: ok")
