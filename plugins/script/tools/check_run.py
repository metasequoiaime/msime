#!/usr/bin/env python3
"""核对一个插件的 msime-pack run 输出，由 scripts/test-script-plugins.sh 调用。

- 授予全部能力的输出去掉 fuel、init_fuel 两个字段后，和插件目录里的 expected.jsonl 逐行相同（command 行的 rows 也在其中逐行核对）。fuel 随编译器版本和优化选项变化，不能直接比；--update 时改为把去掉后的结果写进 expected.jsonl。
- 每一行的 fuel 不超过所在钩子单次上限的一半（candidates 62 500，before_commit 和 command 50 000，其余 12 500）。单次投递被本轮配额截断时，用掉一半以上就算一次失败，所以插件的最坏情况要留在一半以内。
- 不授予任何能力时，每一行的 status 都是 ok（或者 not_subscribed），没有任何修改动作，command 行没有给出任何行，candidates 行没有插入任何行，存储也没有被改动。

`<plugin-dir>` 也可以是插件的 `cases/<name>/` 目录，核对的是那里的 expected.jsonl。

usage: check_run.py <plugin-dir> <granted.jsonl> <ungranted.jsonl> [--update]
"""
import json
import sys
from pathlib import Path

# crates/script-runtime/src/limits.rs 的单次钩子上限的一半。
FUEL_CEILING = {"candidates": 62_500, "before_commit": 50_000, "command": 50_000}
OBSERVE_CEILING = 12_500


def read(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def stripped(row: dict) -> str:
    return json.dumps({key: value for key, value in row.items() if key not in ("fuel", "init_fuel")}, ensure_ascii=False)


def main(argv: list[str]) -> int:
    update = "--update" in argv
    arguments = [argument for argument in argv if argument != "--update"]
    if len(arguments) != 3:
        raise SystemExit(__doc__.strip().splitlines()[-1])
    plugin, granted, ungranted = Path(arguments[0]), Path(arguments[1]), Path(arguments[2])
    expected_path = plugin / "expected.jsonl"
    problems: list[str] = []

    rows = read(granted)
    actual = [stripped(row) for row in rows]
    if update:
        expected_path.write_text("".join(line + "\n" for line in actual), encoding="utf-8")
        print(f"wrote {expected_path}")
    elif not expected_path.is_file():
        problems.append(f"{expected_path} 不存在（用 --update 生成，逐行核对后提交）")
    else:
        expected = expected_path.read_text(encoding="utf-8").splitlines()
        for index in range(max(len(expected), len(actual))):
            want = expected[index] if index < len(expected) else "<没有这一行>"
            got = actual[index] if index < len(actual) else "<没有这一行>"
            if want != got:
                problems.append(f"第 {index + 1} 行不一致\n  expected: {want}\n  actual:   {got}")

    for row in rows:
        ceiling = FUEL_CEILING.get(row["event"], OBSERVE_CEILING)
        if row.get("fuel", 0) > ceiling:
            problems.append(f"seq {row['seq']} {row['event']} 用了 {row['fuel']} fuel，超过 {ceiling}")

    for row in read(ungranted):
        if row["status"] not in ("ok", "not_subscribed") or row["actions"] or row.get("rows") or row.get("inserts") or "storage" in row:
            problems.append(f"不授权时 seq {row['seq']} 应该什么都不做：{stripped(row)}")

    for problem in problems:
        print(f"FAIL {plugin.name}: {problem}")
    if not problems:
        peak = max((row.get("fuel", 0) for row in rows), default=0)
        print(f"ok {plugin.name}: {len(rows)} events match expected.jsonl, peak fuel {peak}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
