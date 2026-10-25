#!/usr/bin/env python3
"""生成符号模式（Shift+S）的分组码表。

分组码 `xl`（希腊字母）、`sx`（数学符号）、`szq`（带圈数字）、`jt`（箭头）等来自 rime-wanxiang
的 `wanxiang_symbols.yaml`（amzxyz 与贡献者，CC BY 4.0）固定发布里的 `symbols:` 段，每行是
`'/编码': [符号, ...]`。上游那张表是「码 → 码位集合」的事实性映射，本仓库只取分组码、中文组名和
码位，不取注释文字与表情、元素周期表、XCompose 三段（S 模式只能输入字母，那三段的编码含标点或
属于表情面板）。输入不直连上游，取自词库仓 msime-dictionary 的 sources/symbols/：那里收的是上游同一
提交的逐字节副本，符号数据统一由词库仓保管。锁文件是 resources/symbol-groups.lock.json（固定词库仓提交，
另记上游提交供署名），许可证与改动说明在 resources/licenses/rime-wanxiang-CC-BY-4.0.txt。

输出一张 UTF-8 TSV，提交进仓库，由 crates/engine/src/local/symbol_groups.rs 用 include_str!
编进引擎：

- crates/engine/src/local/symbol_groups.tsv：每行 `编码<TAB>中文组名<TAB>符号[<空格>符号...]`。
  顺序即分组码表里的先后；同一编码内按上游列出的顺序。符号之间用一个空格分隔，所以符号本身
  不含空格（上游里带空格的条目一个都没有，数目写成断言）。

--check 重新生成后和仓库里的表逐字节比较，不写文件。输入不在缓存里时 --check 打印 skip 并以
0 退出（契约检查不联网），设了 MSIME_FETCH=1 才下载。

usage: build_symbol_groups.py [--cache <directory>] [--check]   （缓存默认 target/symbol-groups-cache）
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.dont_write_bytecode = True  # 导入 scripts/ 下的脚本时不留下 __pycache__
from fetch_wordbooks import digest, fetch  # noqa: E402  （按锁下载并校验长度和 sha256）

LOCK = ROOT / "resources/symbol-groups.lock.json"
OUTPUT = ROOT / "crates/engine/src/local/symbol_groups.tsv"
DEFAULT_CACHE = ROOT / "target/symbol-groups-cache"

# 一个分组码一行：`    '/编码': [符号, ...]`，四格缩进，编码用单引号或双引号。
GROUP_LINE = re.compile(r"""^ {4}(['"])/(.*?)\1:\s*\[(.*)\]\s*(?:#.*)?$""")
# S 模式只能输入字母；数字编码（万象的 /1）不取，它们由符号名那半边覆盖。
CODE = re.compile(r"[A-Za-z]+\Z")

# 分组码 → 中文组名。组名是本节自定的界面文字，不是上游的注释；编码与码位来自上游。
NAMES = {
    # 数学
    "sx": "数学符号", "dy": "大于", "xy": "小于", "yw": "因为", "sy": "所以",
    "sb": "上标", "xb": "下标",
    # 序号
    "szq": "带圈数字", "szh": "带括号数字", "szd": "带点数字", "lmd": "罗马数字大写",
    "lm": "罗马数字小写", "zmq": "带圈字母", "zmh": "带括号字母", "hzq": "带圈汉字",
    "hzh": "带括号汉字", "szm": "苏州码", "num": "编号",
    # 字母
    "xl": "希腊字母小写", "xld": "希腊字母大写", "ey": "俄文字母小写", "eyd": "俄文字母大写",
    "py": "拼音字母", "pyd": "拼音字母大写", "sd": "声调", "zy": "注音符号", "yb": "英文音标",
    # 数学字母
    "sxc": "数学粗体", "sxx": "数学斜体", "sxz": "数学粗斜体", "sxh": "数学花体",
    "sxhc": "数学花体粗体", "sxg": "数学哥特体", "sxgc": "数学哥特粗体", "sxs": "数学双线体",
    "sxw": "数学无衬线体", "sxwc": "数学无衬线粗体", "sxwx": "数学无衬线斜体",
    "sxwz": "数学无衬线粗斜体", "sxd": "数学等宽体", "xlc": "希腊字母粗体",
    "xlx": "希腊字母斜体", "xlz": "希腊字母粗斜体", "xlwc": "希腊字母无衬线粗体",
    "xlwz": "希腊字母无衬线粗斜体",
    # 单位
    "dw": "单位", "hb": "货币", "yf": "月份", "rf": "日期", "sj": "时间",
    # 箭头与图形
    "jt": "箭头", "jh": "几何图形", "fk": "方块", "wjx": "五角星", "xh": "星号",
    "yd": "圆点", "ih": "叉号",
    # 标点
    "dd": "点号", "fj": "非夹注符号", "jz": "夹注符号", "hj": "行间符号", "bd": "常用标点",
    "bdz": "竖排标点", "bdzy": "中英标点",
    # 杂项
    "fh": "常用符号", "dn": "电脑按键", "xq": "象棋", "mj": "麻将", "sz": "骰子", "pk": "扑克",
    "yy": "音乐", "lx": "性别", "tt": "天体", "xz": "星座", "lssg": "六十四卦", "txj": "太玄经",
    # 汉字部件
    "jg": "汉字结构", "pp": "偏旁", "kx": "康熙部首", "bh": "笔画",
    # 假名与韩文
    "jm": "平假名", "pjm": "片假名", "jmq": "带圈假名", "jmbj": "半角假名", "hw": "韩文字母",
    "hwq": "带圈韩文", "hwh": "带括号韩文",
}

# 上游数据里需要处理的异常，数目写成断言：上游或分组变了就让生成失败，提醒人工复核。
EXPECTED_GROUPS = len(NAMES)
# 上游里带空格的符号一个都没有；出现了就说明解析把一项拆成了两项，输出会坏掉。
EXPECTED_SYMBOLS_WITH_SPACES = 0


def flow_items(text: str) -> list:
    """YAML 行内序列 `a, 'b', "c"`：单引号里的 `''` 是一个单引号，双引号里认 `\\"` 与 `\\\\`。"""
    items, index, length = [], 0, len(text)
    while index < length:
        while index < length and text[index] == " ":
            index += 1
        if index >= length:
            break
        quote = text[index]
        if quote in "'\"":
            index += 1
            value = []
            while True:
                if index >= length:
                    raise ValueError("unterminated quote in a symbol group")
                char = text[index]
                if quote == "'" and char == "'":
                    if index + 1 < length and text[index + 1] == "'":
                        value.append("'")
                        index += 2
                        continue
                    index += 1
                    break
                if quote == '"' and char == "\\" and index + 1 < length:
                    value.append(text[index + 1])
                    index += 2
                    continue
                if quote == '"' and char == '"':
                    index += 1
                    break
                value.append(char)
                index += 1
            items.append("".join(value).strip())
            while index < length and text[index] == " ":
                index += 1
            if index < length and text[index] != ",":
                raise ValueError("expected ',' after a quoted symbol")
            index += 1
        else:
            end = text.find(",", index)
            end = length if end < 0 else end
            items.append(text[index:end].strip())
            index = end + 1
    return [item for item in items if item]


def parse_groups(text: str) -> dict:
    """`symbols:` 段里的分组码表；同名编码出现两次时后者覆盖前者（上游没有这种情况）。"""
    lines = text.splitlines()
    try:
        start = lines.index("symbols:")
    except ValueError as error:
        raise ValueError("no top-level symbols: section") from error
    groups = {}
    for line in lines[start + 1 :]:
        match = GROUP_LINE.match(line)
        if not match:
            continue
        code = match.group(2)
        if not CODE.match(code):
            continue
        items = flow_items(match.group(3))
        if items:
            groups[code] = items
    return groups


def rows(text: str) -> list:
    groups = parse_groups(text)
    missing = sorted(set(NAMES) - set(groups))
    if missing:
        raise ValueError(f"the pinned release has no group {missing}")
    spaced = [symbol for code in NAMES for symbol in groups[code] if " " in symbol or "\t" in symbol]
    if len(spaced) != EXPECTED_SYMBOLS_WITH_SPACES:
        raise ValueError(f"{len(spaced)} symbols contain whitespace: {spaced[:5]}")
    return [(code, NAMES[code], groups[code]) for code in NAMES]


def render(rows: list) -> str:
    # 头注释只写格式、来源和写法规则；组名与码位由下面的行自己说明。
    head = [
        "# 符号模式（Shift+S）的分组码：编码、中文组名、符号。",
        "# 分组码与码位来自 rime-wanxiang 的 wanxiang_symbols.yaml（CC BY 4.0，固定发布见 resources/symbol-groups.lock.json），",
        "# 只取「码 → 码位集合」这类事实性映射，中文组名与排序由本项目自定；署名与改动说明在 resources/licenses/rime-wanxiang-CC-BY-4.0.txt。",
        "# 格式：编码<TAB>中文组名<TAB>符号[<空格>符号...]；符号之间用一个空格分隔，所以符号本身不含空格。",
        "# 由 scripts/build_symbol_groups.py 生成，不要手工编辑；写法规则由 symbol_groups.rs 的单元测试检查。",
        "",
    ]
    return "\n".join(head) + "".join(f"{code}\t{name}\t{' '.join(symbols)}\n" for code, name, symbols in rows)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--check", action="store_true")
    arguments = parser.parse_args()
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    artifact = lock["artifacts"][0]
    if arguments.check and not (arguments.cache / artifact["name"]).is_file():
        if not __import__("os").environ.get("MSIME_FETCH"):
            print("skip: the pinned symbols table is not cached (set MSIME_FETCH=1 to download)")
            return 0
    try:
        source = fetch(artifact, arguments.cache)
    except (OSError, RuntimeError) as error:
        if arguments.check:
            print(f"skip: could not fetch the pinned symbols table: {error}")
            return 0
        raise
    text = source.read_text(encoding="utf-8")
    rendered = render(rows(text))
    if arguments.check:
        current = OUTPUT.read_text(encoding="utf-8")
        if current != rendered:
            print(f"FAIL: {OUTPUT.relative_to(ROOT)} differs from the pinned generator output; run scripts/build_symbol_groups.py")
            return 1
        print(f"symbol groups: {OUTPUT.relative_to(ROOT)} matches the generator and the pinned source ({digest(source)})")
        return 0
    OUTPUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}: {EXPECTED_GROUPS} groups, {sum(len(r[2]) for r in rows(text))} symbols")
    return 0


if __name__ == "__main__":
    sys.exit(main())
