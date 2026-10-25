#!/usr/bin/env python3
"""校验鸿蒙 18 键键面、共享引擎分组和统计键位保持一致。"""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
GROUPS = "q we rt y u io p a sd fg h jk l z xc v bn m".split()


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def check() -> None:
    engine = read("crates/engine/src/nine_key.rs")
    block = re.search(r"const EIGHTEEN_KEY_GROUPS: \[&str; 18\] = \[(.*?)\];", engine, re.S)
    assert block, "引擎缺少 18 键分组"
    assert re.findall(r'"([a-z]+)"', block[1]) == GROUPS, "引擎分组不一致"
    assert sorted("".join(GROUPS)) == list("abcdefghijklmnopqrstuvwxyz"), "字母必须完整且不重复"

    harmony = read("platforms/harmony/entry/src/main/ets/keyboard/input/EighteenKeyLayout.ts")
    assert re.findall(r'"([a-z]+)"', harmony)[:18] == GROUPS, "鸿蒙键面分组不一致"
    expected_ids = ["Eighteen" + group.upper() for group in GROUPS]
    for path in [
        "crates/client-services/src/typing_statistics.rs",
        "platforms/harmony/entry/src/main/ets/keyboard/KeyIdPolicy.ts",
    ]:
        assert re.findall(r'"(Eighteen[A-Z]+)"', read(path)) == expected_ids, f"{path} 的统计标识不一致"
    print("鸿蒙 18 键：7＋6＋5 分组、26 字母与统计白名单一致")


if __name__ == "__main__":
    check()
