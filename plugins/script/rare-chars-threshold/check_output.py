#!/usr/bin/env python3
"""核对 rare-chars-threshold 取默认设置时和 hide-rare-chars 逐行相同，由 scripts/test-script-plugins.sh 在 check_run.py 之后调用。

这个插件的夹具是第一条自己的事件加上 hide-rare-chars 的全部事件。两项设置都取默认值（`scope = "any"`、`keep_rows = 1`）时，承诺的行为就是 hide-rare-chars：这里把第 2 行起的输出和 hide-rare-chars 的 expected.jsonl 逐行比较（`seq` 错开一位，第 1 行多一条初始化日志），免得两个插件以后悄悄分开。

usage: check_output.py <fixture.jsonl> <granted.jsonl>
"""
import json
import sys
from pathlib import Path

HIDE = Path(__file__).resolve().parent.parent / "hide-rare-chars"


def read(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def comparable(row: dict) -> dict:
    return {key: value for key, value in row.items() if key not in ("seq", "fuel", "init_fuel")}


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise SystemExit(__doc__.strip().splitlines()[-1])
    events, outputs = read(Path(argv[0])), read(Path(argv[1]))
    hide_events, hide_outputs = read(HIDE / "fixture.jsonl"), read(HIDE / "expected.jsonl")
    problems = []
    if events[1:] != hide_events:
        problems.append("夹具第 2 行起应该和 hide-rare-chars/fixture.jsonl 相同")
    for index, (mine, theirs) in enumerate(zip(outputs[1:], hide_outputs), start=2):
        if comparable(mine) != comparable(theirs):
            problems.append(f"第 {index} 行和 hide-rare-chars 第 {index - 1} 行不同：{comparable(mine)} / {comparable(theirs)}")
    if len(outputs) - 1 != len(hide_outputs):
        problems.append(f"输出有 {len(outputs)} 行，hide-rare-chars 有 {len(hide_outputs)} 行")
    for problem in problems:
        print(f"FAIL rare-chars-threshold (defaults): {problem}")
    if not problems:
        print(f"ok rare-chars-threshold: defaults match hide-rare-chars on {len(hide_outputs)} events")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
