#!/usr/bin/env python3
"""逐例配对比较两次评测：`convert_eval --dump` 的候选表，或 `typing_sim` 的报告和 dump。

为什么要有它：`verify-local.sh` 把评测报告和提交的基线整份逐字节比较，只能说明「变没变」，说明不了「好还是坏」；两份汇总值相减也不行，单臂比例的置信半宽在词级集上也有一两个点，而真正要问的是同一批用例里有多少条从错变对、多少条从对变错。这里按用例 id 配对，列出修好的（fixed）和弄坏的（broken），对 top-1、top-5 各做一次精确 McNemar 检验（只看两边判定不同的那些用例，二项分布 p=0.5 的双侧 p 值）。

同码用例单独分桶：同一串码（全拼的字母串、九键的数字串、14 键的组码串）在集合里对应不止一个金标准时，至多一个能排第一，top-1 的上限天生不到 1，换一份词库只是在它们之间换人。`quanpin-words-v2.tsv` 给这种用例打了 `same-code-full`、`same-code-nine`、`same-code-fourteen` 标签；给了 `--set` 时按 `--grid` 选的那个标签分桶，同码桶只报告，不进 `--gate`。

用法：

    python3 scripts/eval_compare.py convert BEFORE.jsonl AFTER.jsonl \\
        [--set resources/eval/quanpin-words-v2.tsv] [--grid full|nine|fourteen] \\
        [--top 1,5] [--list 40] [--json OUT.json] [--gate] [--alpha 0.05]

    python3 scripts/eval_compare.py typing BEFORE.json AFTER.json [--dumps BEFORE.jsonl AFTER.jsonl]

`convert` 的两份 dump 必须来自同一份集合（id 集合相同，每个 id 的输入和金标准相同），否则报错退出。`--gate` 按 dict-cleanse 方案 4.3 的硬门槛：非同码桶里任何一个 k，broken 多于 fixed 而且 McNemar p < `--alpha`，以退出码 1 结束。

`typing` 比较两份 `typing_sim` 报告，忽略 `config.resources` 里的 `generation` 和 `source_commit`：这两项记录的是资源身份，换一份词库必然不同，逐字节比较因此每次换资源都红，掩盖了真正的数字有没有变。其余字段完全相同时退出码 0，否则列出不同的字段和 dev、test 的主要指标差，退出码 1。给了 `--dumps` 时再按块配对（行号加这一行里的第几块，同一组参数下切块只由种子决定，与词库无关），对整块首选（`top1_exact`）做 McNemar，并比较严口径首选字数（`first_choice_chars`）。
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

GRIDS = ("full", "nine", "fourteen")
# typing_sim 报告里记录资源身份、换资源必然不同的字段，比较时忽略。
TYPING_IGNORED = (("config", "resources", "generation"), ("config", "resources", "source_commit"))
TYPING_HEADLINE = ("cfa", "cfa_lenient", "cca", "cpr", "zcr", "kspc", "unreachable")


def mcnemar_exact(fixed: int, broken: int) -> float:
    """精确 McNemar 双侧 p 值：判定不同的 n=fixed+broken 条里，较少的一边不多于 min(fixed, broken) 的二项概率的两倍，上限 1。"""
    n = fixed + broken
    if n == 0:
        return 1.0
    k = min(fixed, broken)
    term = 1
    tail = 1
    for i in range(1, k + 1):
        term = term * (n - i + 1) // i
        tail += term
    return min(1.0, 2 * tail / (1 << n))


def gold_rank(candidates: list, gold: str) -> int | None:
    """金标准在候选表里的名次（从 1 起），与 `convert_eval` 的 `metrics.rs` 同口径：文字完全相同的第一个。"""
    for index, candidate in enumerate(candidates):
        if candidate.get("text") == gold:
            return index + 1
    return None


def load_dump(path: Path) -> dict:
    """读一份 `convert_eval --dump`，按 id 返回 {input, gold, rank}。同一个 id 出现两次（双拼的多组配置写在一份 dump 里）时报错。"""
    cases = {}
    with path.open(encoding="utf-8") as file:
        for number, line in enumerate(file, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            case_id = row["id"]
            if case_id in cases:
                raise SystemExit(f"{path}:{number}: duplicate id {case_id}; compare one configuration per dump")
            cases[case_id] = {
                "input": row["input"],
                "gold": row["gold"],
                "rank": gold_rank(row.get("candidates", []), row["gold"]),
            }
    return cases


def load_tags(path: Path) -> dict:
    """评测集每条用例的标签（第 5 列，逗号分隔），格式同 `convert_eval` 的 `load`。"""
    tags = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or line.startswith("id\t"):
            continue
        fields = line.split("\t")
        tags[fields[0]] = [tag for tag in (fields[4] if len(fields) > 4 else "").split(",") if tag]
    return tags


def pair(before: dict, after: dict) -> list:
    """按 id 配对两份 dump。id 集合、输入或金标准不一致说明不是同一份集合，直接报错。"""
    if before.keys() != after.keys():
        only_before = sorted(before.keys() - after.keys())
        only_after = sorted(after.keys() - before.keys())
        raise SystemExit(
            f"the dumps are not of the same set: {len(only_before)} id(s) only before (e.g. {only_before[:3]}), "
            f"{len(only_after)} only after (e.g. {only_after[:3]})"
        )
    pairs = []
    for case_id in sorted(before):
        a, b = before[case_id], after[case_id]
        if (a["input"], a["gold"]) != (b["input"], b["gold"]):
            raise SystemExit(f"{case_id}: input or gold differs between the dumps ({a['input']} {a['gold']} vs {b['input']} {b['gold']})")
        pairs.append({"id": case_id, "input": a["input"], "gold": a["gold"], "before": a["rank"], "after": b["rank"]})
    return pairs


def hit(rank: int | None, k: int) -> bool:
    return rank is not None and rank <= k


def compare_bucket(pairs: list, tops: list) -> dict:
    """一个桶里每个 k 的命中率、fixed、broken 和 McNemar p。fixed/broken 按名次列出，便于对照。"""
    result = {"cases": len(pairs)}
    for k in tops:
        fixed = [p for p in pairs if not hit(p["before"], k) and hit(p["after"], k)]
        broken = [p for p in pairs if hit(p["before"], k) and not hit(p["after"], k)]
        before_hits = sum(hit(p["before"], k) for p in pairs)
        after_hits = sum(hit(p["after"], k) for p in pairs)
        result[f"top{k}"] = {
            "before": round(before_hits / len(pairs), 4) if pairs else 0.0,
            "after": round(after_hits / len(pairs), 4) if pairs else 0.0,
            "fixed": len(fixed),
            "broken": len(broken),
            "mcnemar_p": mcnemar_exact(len(fixed), len(broken)),
            "fixed_cases": fixed,
            "broken_cases": broken,
        }
    return result


def compare_convert(before: dict, after: dict, tags: dict | None, grid: str, tops: list) -> dict:
    pairs = pair(before, after)
    tag = f"same-code-{grid}"
    if tags is not None:
        missing = [p["id"] for p in pairs if p["id"] not in tags]
        if missing:
            raise SystemExit(f"{len(missing)} dumped id(s) are not in --set (e.g. {missing[:3]}); pass the set the dumps were run on")
        same = [p for p in pairs if tag in tags[p["id"]]]
        main = [p for p in pairs if tag not in tags[p["id"]]]
    else:
        same, main = [], pairs
    return {
        "grid": grid,
        "same_code_tag": tag if tags is not None else None,
        "main": compare_bucket(main, tops),
        "same_code": compare_bucket(same, tops) if tags is not None else None,
    }


def gate_failures(result: dict, tops: list, alpha: float) -> list:
    """方案 4.3 的硬门槛：非同码桶里 broken > fixed 且 p < alpha 的 k。"""
    failures = []
    for k in tops:
        top = result["main"][f"top{k}"]
        if top["broken"] > top["fixed"] and top["mcnemar_p"] < alpha:
            failures.append(f"top-{k}: broken {top['broken']} > fixed {top['fixed']}, McNemar p={top['mcnemar_p']:.3g} < {alpha}")
    return failures


def format_rank(rank: int | None) -> str:
    return "-" if rank is None else str(rank)


def print_convert(result: dict, tops: list, listed: int) -> None:
    buckets = [("main", result["main"])]
    if result["same_code"] is not None:
        buckets.append((f"same-code ({result['same_code_tag']}, report only)", result["same_code"]))
    for name, bucket in buckets:
        print(f"[{name}] {bucket['cases']} cases")
        for k in tops:
            top = bucket[f"top{k}"]
            print(f"  top-{k}: {top['before']:.4f} -> {top['after']:.4f}  fixed {top['fixed']}  broken {top['broken']}  McNemar p={top['mcnemar_p']:.3g}")
    for k in tops:
        top = result["main"][f"top{k}"]
        for label in ("broken", "fixed"):
            cases = top[f"{label}_cases"]
            if not cases or listed <= 0:
                continue
            print(f"{label} top-{k} (main, {len(cases)}{', first ' + str(listed) if len(cases) > listed else ''}): id input gold rank before->after")
            for case in cases[:listed]:
                print(f"  {case['id']}\t{case['input']}\t{case['gold']}\t{format_rank(case['before'])}->{format_rank(case['after'])}")


def strip_ignored(report: dict) -> dict:
    """去掉 typing_sim 报告里记录资源身份的字段，返回副本。"""
    copy = json.loads(json.dumps(report))
    for path in TYPING_IGNORED:
        node = copy
        for key in path[:-1]:
            node = node.get(key, {}) if isinstance(node, dict) else {}
        if isinstance(node, dict):
            node.pop(path[-1], None)
    return copy


def differences(a, b, path: str = "") -> list:
    """两份 JSON 值不同的位置，按路径列出。"""
    if isinstance(a, dict) and isinstance(b, dict):
        found = []
        for key in sorted(set(a) | set(b)):
            sub = f"{path}.{key}" if path else key
            if key not in a or key not in b:
                found.append(sub)
            else:
                found.extend(differences(a[key], b[key], sub))
        return found
    if isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        found = []
        for index, (x, y) in enumerate(zip(a, b)):
            found.extend(differences(x, y, f"{path}[{index}]"))
        return found
    return [] if a == b else [path]


def compare_typing_reports(before: dict, after: dict) -> dict:
    a, b = strip_ignored(before), strip_ignored(after)
    changed = differences(a, b)
    headline = {}
    for split in ("dev", "test"):
        x = a.get("overall", {}).get(split, {})
        y = b.get("overall", {}).get(split, {})
        headline[split] = {name: [x.get(name), y.get(name)] for name in TYPING_HEADLINE if name in x or name in y}
    return {"identical": not changed, "changed_fields": changed, "headline": headline}


def load_typing_dump(path: Path) -> dict:
    """typing_sim dump 的每一块，按（行号、这一行里的第几块）为键；预热的块不计分，跳过。"""
    blocks = {}
    ordinal = {}
    with path.open(encoding="utf-8") as file:
        for line in file:
            if not line.strip():
                continue
            row = json.loads(line)
            index = ordinal.get(row["row"], 0)
            ordinal[row["row"]] = index + 1
            if row.get("split") == "warmup":
                continue
            blocks[(row["row"], index)] = row
    return blocks


def compare_typing_dumps(before: dict, after: dict) -> dict:
    if before.keys() != after.keys():
        raise SystemExit("the typing dumps do not hold the same blocks: run both arms with the same set, seed and chunking")
    result = {}
    for split in sorted({row["split"] for row in before.values()}):
        keys = [key for key in before if before[key]["split"] == split]
        for key in keys:
            if before[key]["chars"] != after[key]["chars"]:
                raise SystemExit(f"block {key} has {before[key]['chars']} chars before and {after[key]['chars']} after; the chunking differs")
        fixed = [key for key in keys if not before[key]["top1_exact"] and after[key]["top1_exact"]]
        broken = [key for key in keys if before[key]["top1_exact"] and not after[key]["top1_exact"]]
        chars = sum(before[key]["chars"] for key in keys)
        result[split] = {
            "blocks": len(keys),
            "chars": chars,
            "top1_exact_fixed": len(fixed),
            "top1_exact_broken": len(broken),
            "top1_exact_mcnemar_p": mcnemar_exact(len(fixed), len(broken)),
            "first_choice_chars": [sum(before[key]["first_choice_chars"] for key in keys), sum(after[key]["first_choice_chars"] for key in keys)],
        }
    return result


def parse_tops(text: str) -> list:
    tops = sorted({int(part) for part in text.split(",") if part})
    if not tops or tops[0] < 1:
        raise argparse.ArgumentTypeError("--top takes positive ranks, e.g. 1,5")
    return tops


def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    convert = commands.add_parser("convert", help="pair two convert_eval --dump files")
    convert.add_argument("before", type=Path)
    convert.add_argument("after", type=Path)
    convert.add_argument("--set", type=Path, help="the evaluation set the dumps were run on, for the same-code tags")
    convert.add_argument("--grid", choices=GRIDS, help="the keyboard the dumps were typed on; its same-code tag defines the same-code bucket (required with --set)")
    convert.add_argument("--top", type=parse_tops, default=[1, 5])
    convert.add_argument("--list", type=int, default=40, help="how many fixed/broken cases to print per list")
    convert.add_argument("--json", type=Path, help="write the full comparison here")
    convert.add_argument("--gate", action="store_true", help="exit 1 when the main bucket fails the plan's hard gate (requires --set and --grid)")
    convert.add_argument("--alpha", type=float, default=0.05)
    typing = commands.add_parser("typing", help="compare two typing_sim reports, ignoring the resource identity")
    typing.add_argument("before", type=Path)
    typing.add_argument("after", type=Path)
    typing.add_argument("--dumps", type=Path, nargs=2, metavar=("BEFORE_DUMP", "AFTER_DUMP"))
    typing.add_argument("--json", type=Path)
    args = parser.parse_args(argv)

    if args.command == "convert":
        if args.set and not args.grid:
            parser.error("--set needs --grid: the same-code bucket depends on the keyboard the dumps were typed on")
        if args.gate and not args.set:
            parser.error("--gate needs --set and --grid: without the same-code tags every case lands in the gated bucket, and plan 4.3 keeps the same-code bucket out of the gate")
        tags = load_tags(args.set) if args.set else None
        result = compare_convert(load_dump(args.before), load_dump(args.after), tags, args.grid or "full", args.top)
        print_convert(result, args.top, args.list)
        if args.json:
            args.json.write_text(json.dumps(result, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
        failures = gate_failures(result, args.top, args.alpha)
        for failure in failures:
            print(f"gate: {failure}", file=sys.stderr)
        return 1 if args.gate and failures else 0

    result = compare_typing_reports(json.loads(args.before.read_text(encoding="utf-8")), json.loads(args.after.read_text(encoding="utf-8")))
    if args.dumps:
        result["blocks"] = compare_typing_dumps(load_typing_dump(args.dumps[0]), load_typing_dump(args.dumps[1]))
    for split, values in result["headline"].items():
        print(split + ": " + "  ".join(f"{name} {x} -> {y}" for name, (x, y) in values.items()))
    for split, values in result.get("blocks", {}).items():
        print(f"{split} blocks {values['blocks']}: top1_exact fixed {values['top1_exact_fixed']} broken {values['top1_exact_broken']} McNemar p={values['top1_exact_mcnemar_p']:.3g}; first_choice_chars {values['first_choice_chars'][0]} -> {values['first_choice_chars'][1]} of {values['chars']}")
    if result["identical"]:
        print("typing reports identical apart from config.resources.generation/source_commit")
    else:
        print(f"{len(result['changed_fields'])} field(s) differ, e.g. " + ", ".join(result["changed_fields"][:10]))
    if args.json:
        args.json.write_text(json.dumps(result, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return 0 if result["identical"] else 1


if __name__ == "__main__":
    sys.exit(main())
