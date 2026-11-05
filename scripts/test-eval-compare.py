#!/usr/bin/env python3
"""检查 scripts/eval_compare.py 的配对规则：名次口径、fixed/broken、精确 McNemar、同码分桶、硬门槛，以及 typing_sim 报告比较忽略资源身份。

不需要词库或评测二进制，全部用合成的 dump。
"""
import importlib.util
import json
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("eval_compare", ROOT / "scripts/eval_compare.py")
eval_compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(eval_compare)

# 精确 McNemar：3 条全同向是 0.25（README 里 4.25M 对 24.9M 的那个数），没有分歧是 1。
assert eval_compare.mcnemar_exact(0, 3) == 0.25
assert eval_compare.mcnemar_exact(3, 0) == 0.25
assert eval_compare.mcnemar_exact(0, 0) == 1.0
assert eval_compare.mcnemar_exact(5, 5) == 1.0
assert abs(eval_compare.mcnemar_exact(2, 10) - 0.0386) < 1e-4
# 分歧数很大时用整数算，不溢出也不丢精度。
assert eval_compare.mcnemar_exact(4000, 4000) == 1.0
assert 0.0 <= eval_compare.mcnemar_exact(100, 3000) < 1e-300

# 名次与 metrics.rs 同口径：文字完全相同的第一个，从 1 起。
assert eval_compare.gold_rank([{"text": "车件"}, {"text": "车检"}], "车检") == 2
assert eval_compare.gold_rank([{"text": "车件"}], "车检") is None


def write_dump(path, rows):
    path.write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows), encoding="utf-8")


def row(case_id, gold, texts, case_input="x"):
    return {"id": case_id, "input": case_input, "gold": gold, "context": "", "candidates": [{"text": t, "source": 0} for t in texts]}


with tempfile.TemporaryDirectory(prefix="msime-eval-compare-") as temporary:
    root = Path(temporary)
    before, after, tsv = root / "before.jsonl", root / "after.jsonl", root / "set.tsv"
    write_dump(before, [
        row("a", "车检", ["车件", "车检"]),          # 2 -> 1：top-1 修好
        row("b", "火灾", ["火灾"]),                  # 1 -> 6：top-1、top-5 都弄坏
        row("c", "闷的", ["门的", "闷的"]),          # 同码，2 -> 1，只进同码桶
        row("d", "万州", ["万州"]),                  # 不变
    ])
    write_dump(after, [
        row("a", "车检", ["车检", "车件"]),
        row("b", "火灾", ["或在", "1", "2", "3", "4", "火灾"]),
        row("c", "闷的", ["闷的", "门的"]),
        row("d", "万州", ["万州"]),
    ])
    tsv.write_text("# synthetic\nid\tinput\tgold\tsyllables\ttags\na\tx\t车检\t2\t\nb\tx\t火灾\t2\tsame-code-nine\nc\tx\t闷的\t2\tsame-code-full\nd\tx\t万州\t2\t\n", encoding="utf-8")

    result = eval_compare.compare_convert(eval_compare.load_dump(before), eval_compare.load_dump(after), eval_compare.load_tags(tsv), "full", [1, 5])
    main_top1 = result["main"]["top1"]
    assert result["main"]["cases"] == 3 and result["same_code"]["cases"] == 1
    assert [c["id"] for c in main_top1["fixed_cases"]] == ["a"]
    assert [c["id"] for c in main_top1["broken_cases"]] == ["b"]
    assert main_top1["before"] == round(2 / 3, 4) and main_top1["after"] == round(2 / 3, 4)
    assert result["main"]["top5"]["broken"] == 1 and result["main"]["top5"]["fixed"] == 0
    assert result["same_code"]["top1"]["fixed"] == 1
    # 换成九键的标签，b 进同码桶，c 回到主桶。
    nine = eval_compare.compare_convert(eval_compare.load_dump(before), eval_compare.load_dump(after), eval_compare.load_tags(tsv), "nine", [1])
    assert [c["id"] for c in nine["main"]["top1"]["fixed_cases"]] == ["a", "c"]
    assert nine["same_code"]["top1"]["broken"] == 1
    # 同 id 的其他集合不能提供分桶标签，否则退化用例可能被错误豁免。
    for column, replacement in ((1, "other"), (2, "另一个金标准")):
        wrong_set = root / "wrong.tsv"
        lines = tsv.read_text(encoding="utf-8").splitlines()
        fields = lines[2].split("\t")
        fields[column] = replacement
        lines[2] = "\t".join(fields)
        wrong_set.write_text("\n".join(lines) + "\n", encoding="utf-8")
        try:
            eval_compare.compare_convert(eval_compare.load_dump(before), eval_compare.load_dump(after), eval_compare.load_tags(wrong_set), "full", [1])
            raise AssertionError("set metadata must match the dump")
        except SystemExit as error:
            assert "--set" in str(error) and "differs" in str(error)

    # 没给 --set 时全部进主桶。
    untagged = eval_compare.compare_convert(eval_compare.load_dump(before), eval_compare.load_dump(after), None, "full", [1])
    assert untagged["main"]["cases"] == 4 and untagged["same_code"] is None

    # 硬门槛：broken > fixed 但 p 不显著时不拦；显著时拦。
    assert eval_compare.gate_failures(result, [1, 5], 0.05) == []
    many_broken = {"main": {"top1": {"fixed": 2, "broken": 10, "mcnemar_p": eval_compare.mcnemar_exact(2, 10)}}}
    assert len(eval_compare.gate_failures(many_broken, [1], 0.05)) == 1
    assert eval_compare.gate_failures(many_broken, [1], 0.01) == []

    # 两份 dump 的集合不同、或同一个 id 的金标准不同，都直接报错。
    other = root / "other.jsonl"
    write_dump(other, [row("a", "车检", ["车检"])])
    try:
        eval_compare.pair(eval_compare.load_dump(before), eval_compare.load_dump(other))
        raise AssertionError("mismatched ids must fail")
    except SystemExit as error:
        assert "not of the same set" in str(error)
    write_dump(other, [row("a", "车件", []), row("b", "火灾", []), row("c", "闷的", []), row("d", "万州", [])])
    try:
        eval_compare.pair(eval_compare.load_dump(before), eval_compare.load_dump(other))
        raise AssertionError("mismatched gold must fail")
    except SystemExit as error:
        assert "differs" in str(error)
    write_dump(other, [row("a", "车检", []), row("a", "车检", [])])
    try:
        eval_compare.load_dump(other)
        raise AssertionError("duplicate ids must fail")
    except SystemExit as error:
        assert "duplicate id" in str(error)

    # 命令行：--gate 在非同码桶显著变坏时退出 1；同码桶里再怎么变坏也不卡。
    regress_before, regress_after, regress_set = root / "rb.jsonl", root / "ra.jsonl", root / "regress.tsv"
    write_dump(regress_before, [row(f"w{i}", "对", ["对"]) for i in range(24)])
    regress_set.write_text("".join(f"w{i}\tx\t对\t\t{'same-code-nine' if i >= 12 else ''}\n" for i in range(24)), encoding="utf-8")
    gate = ["--set", str(regress_set), "--grid", "nine", "--list", "0"]
    write_dump(regress_after, [row(f"w{i}", "对", ["错", "对"] if i < 10 else ["对"]) for i in range(24)])
    assert eval_compare.main(["convert", str(regress_before), str(regress_after), "--gate", *gate]) == 1
    assert eval_compare.main(["convert", str(regress_before), str(regress_after), *gate]) == 0
    assert eval_compare.main(["convert", str(regress_after), str(regress_before), "--gate", *gate]) == 0
    write_dump(regress_after, [row(f"w{i}", "对", ["错", "对"] if 12 <= i < 22 else ["对"]) for i in range(24)])
    assert eval_compare.main(["convert", str(regress_before), str(regress_after), "--gate", *gate]) == 0
    # --gate 没给 --set 时拒绝：不然同码用例会混进被卡的桶。
    try:
        eval_compare.main(["convert", str(regress_before), str(regress_after), "--gate", "--list", "0"])
        raise AssertionError("--gate without --set must fail")
    except SystemExit as error:
        assert error.code == 2

    # 给了 --set 却没说是哪种键盘，同码桶分不出来，直接拒绝。
    try:
        eval_compare.main(["convert", str(before), str(after), "--set", str(tsv)])
        raise AssertionError("--set without --grid must fail")
    except SystemExit as error:
        assert error.code == 2

    # typing_sim：只差资源身份时相同；数字一变就不同，并列出字段。
    report = {"tool": "typing_sim", "config": {"resources": {"generation": "a" * 64, "source_commit": "b" * 40}, "keyboard": "nine"}, "overall": {"dev": {"cfa": 0.5, "cca": 0.4}, "test": {"cfa": 0.6, "cca": 0.5}}}
    moved = json.loads(json.dumps(report))
    moved["config"]["resources"] = {"generation": "c" * 64, "source_commit": "d" * 40}
    compared = eval_compare.compare_typing_reports(report, moved)
    assert compared["identical"] and compared["changed_fields"] == []
    moved["overall"]["test"]["cfa"] = 0.61
    compared = eval_compare.compare_typing_reports(report, moved)
    assert not compared["identical"] and compared["changed_fields"] == ["overall.test.cfa"]
    assert compared["headline"]["test"]["cfa"] == [0.6, 0.61]
    # 别的配置字段变了照样算不同：只忽略那两项。
    moved = json.loads(json.dumps(report))
    moved["config"]["keyboard"] = "full"
    assert eval_compare.compare_typing_reports(report, moved)["changed_fields"] == ["config.keyboard"]
    assert report["config"]["resources"]["generation"] == "a" * 64, "stripping must not mutate the input"

    # typing_sim dump 按（行、第几块）配对，预热不计。
    def block(row_id, split, chars, exact, first):
        return {"row": row_id, "split": split, "chars": chars, "top1_exact": exact, "first_choice_chars": first}

    dump_before, dump_after = root / "tb.jsonl", root / "ta.jsonl"
    write_dump(dump_before, [block("r1", "warmup", 2, False, 0), block("r2", "dev", 2, False, 0), block("r2", "dev", 3, True, 3)])
    write_dump(dump_after, [block("r1", "warmup", 2, True, 2), block("r2", "dev", 2, True, 2), block("r2", "dev", 3, True, 3)])
    blocks = eval_compare.compare_typing_dumps(eval_compare.load_typing_dump(dump_before), eval_compare.load_typing_dump(dump_after))
    assert blocks == {"dev": {"blocks": 2, "chars": 5, "top1_exact_fixed": 1, "top1_exact_broken": 0, "top1_exact_mcnemar_p": 1.0, "first_choice_chars": [3, 5]}}, blocks

print("eval compare: ok")
