#!/usr/bin/env python3
"""生成 `typing_sim` 用的打字流评测集（`resources/eval/typing-*.tsv`）。

`typing_sim` 模拟一个用户从头到尾按顺序打一整段文字流，学习一直开着，所以评测集是「一行一条消息、按发出的先后排好」的文字，而不是互相独立的用例。这个脚本只做三件事：取文字、切词、标拼音。它不跑引擎，也不按引擎的成败筛任何一行：按「当前打得对不对」筛出来的集合量不出真实的准确率（`resources/eval/README.md` 收割集一节）。

两种来源：

  tatoeba  Tatoeba 的普通话句子导出（`cmn_sentences_detailed.tsv`，CC BY 2.0 FR），固定种子随机抽样，只留简体、4 到 40 个汉字的句子，按去掉标点后的文字去重。每句是一条独立的消息，前 `--warmup` 条是预热。CC BY 要求署名跟着文本走，所以输出多一列 `author`，是导出文件第 4 列里这句的 Tatoeba 用户名；文件头写明许可的地址和本仓库加了什么。
  persona  为本仓库手写的单人聊天记录（`# day N` 分天，`@对象` 分会话，其余每行一条消息），随仓库以 GPL-3.0 发布。前 `--warmup-days` 天是预热，之后逢 3 的倍数的那天进 dev，其余进 test。

切词用 jieba（MIT），拼音用 pypinyin（MIT）按整句标，再按词归组：整句标能用上下文（还钱 huan，还是 hai），逐词标会把单独切出来的「还」一律标成 hai。两者都不是仓库依赖，跑这个脚本的人自己装（`pip install jieba==0.42.1 pypinyin opencc-python-reimplemented`）；仓库只收生成好的文件。

`--readings` 是人工核对过的读音改正，一行 `id<TAB>词序号<TAB>拼音`，生成时覆盖 pypinyin 的读音。要核对哪些词，用 `typing_sim --lint-readings` 列出与引擎自己的读音（`hanzi_to_pinyin`）不一致的词。

usage:
  scripts/build-typing-set.py tatoeba <cmn_sentences_detailed.tsv> --seed 20261028 --count 1500 --warmup 400 [--drop <ids>] [--readings <file>]
  scripts/build-typing-set.py persona <persona-chat.txt> --warmup-days 8 [--readings <file>]
"""

import argparse
import random
import re
import sys

HAN = re.compile(r"[一-鿿]")


def han_count(text):
    return len(HAN.findall(text))


def is_han(text):
    return bool(text) and all("一" <= ch <= "鿿" for ch in text)


def tokens_for(text, overrides, row_id):
    """切词并标拼音，返回 `词=拼音` 或 `~原样文字` 的列表。"""
    import jieba
    from pypinyin import Style, lazy_pinyin

    jieba.setLogLevel(60)
    # 非汉字按字符各占一项，这样读音和原文逐字对齐。
    readings = lazy_pinyin(text, style=Style.NORMAL, errors=lambda chars: [None] * len(chars))
    if len(readings) != len(text):
        raise SystemExit(f"{row_id}: pinyin does not align with the text")
    tokens = []
    position = 0
    index = 0
    for word in jieba.lcut(text, HMM=True):
        start = position
        position += len(word)
        if word.isspace():
            continue
        if is_han(word):
            pinyin = "'".join(readings[start:position])
            pinyin = overrides.pop((row_id, index), pinyin)
            tokens.append(f"{word}={pinyin}")
        else:
            # 汉字和别的字符混在一个词里（jieba 很少这样切）时拆开，汉字照样要打。
            for piece in re.findall(r"[一-鿿]+|[^一-鿿]+", word):
                offset = start + word.index(piece)
                if is_han(piece):
                    pinyin = "'".join(readings[offset : offset + len(piece)])
                    pinyin = overrides.pop((row_id, index), pinyin)
                    tokens.append(f"{piece}={pinyin}")
                else:
                    tokens.append("~" + piece.replace(" ", "_"))
                index += 1
            continue
        index += 1
    return tokens


def load_overrides(path):
    overrides = {}
    if not path:
        return overrides
    for line in open(path, encoding="utf-8"):
        if not line.strip() or line.startswith("#"):
            continue
        row_id, index, pinyin = line.rstrip("\n").split("\t")[:3]
        overrides[(row_id, int(index))] = pinyin
    return overrides


def tatoeba_rows(args):
    from opencc import OpenCC

    t2s = OpenCC("t2s")
    drop = set(args.drop.split(",")) if args.drop else set()
    seen, rows = set(), []
    for line in open(args.source, encoding="utf-8"):
        parts = line.rstrip("\n").split("\t")
        if len(parts) < 4 or parts[1] != "cmn":
            continue
        sid, text = parts[0], parts[2].strip()
        # 导出里没有作者的句子（孤儿句）用户名是 `\N`，写成 `-`。
        author = parts[3] if parts[3] not in ("", "\\N") else "-"
        han = han_count(text)
        # 只收简体：OpenCC 繁转简之后不变的句子。
        if han < 4 or han > 40 or t2s.convert(text) != text:
            continue
        key = re.sub(r"[^一-鿿A-Za-z0-9]", "", text)
        if key in seen:
            continue
        seen.add(key)
        rows.append((int(sid), text, author))
    eligible = len(rows)
    rows.sort()
    random.Random(args.seed).shuffle(rows)
    rows = rows[: args.count]
    kept = []
    for position, (sid, text, author) in enumerate(rows):
        if str(sid) in drop:
            continue
        if position < args.warmup:
            split = "warmup"
        else:
            split = "dev" if sid % 3 == 0 else "test"
        kept.append((f"t{sid}", f"t{sid}", split, text, author))
    header = [
        "# Tatoeba 普通话句子（https://tatoeba.org），许可 CC BY 2.0 FR（https://creativecommons.org/licenses/by/2.0/fr/）。每行一句，id 去掉前缀 t 就是 Tatoeba 的句子编号，原文在 https://tatoeba.org/sentences/show/<编号>；最后一列 author 是这句在 Tatoeba 上的作者（用户名，导出里没有作者的写作 -）。",
        "# 本仓库的改动：抽样和分 warmup/dev/test（规则见下），加了切词和拼音（tokens 列，人工改正过部分读音）；text 列是导出里的原句，只去掉了首尾空白。",
        f"# 抽样：导出文件 cmn_sentences_detailed.tsv 里简体、4 到 40 个汉字、去重后的 {eligible} 句，按句子编号排序后用种子 {args.seed} 打乱，取前 {args.count} 句；前 {args.warmup} 句是预热（warmup），其余句子编号能被 3 整除的进 dev，其余进 test。",
        f"# 人工评审只看两件事：是不是正确的中文，适不适合放进公开仓库。去掉 {len(rows) - len(kept)} 句（病句、错字、性、暴力、政治话题、个人信息），没有一句是按引擎的输出去掉的。",
    ]
    return header, kept


def persona_rows(args):
    day = 0
    conversation = 0
    message = 0
    rows = []
    for line in open(args.source, encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        if line.startswith("# day "):
            day = int(line[len("# day ") :])
            conversation = 0
            continue
        if line.startswith("@"):
            conversation += 1
            message = 0
            continue
        message += 1
        doc = f"d{day:02d}c{conversation}"
        if day <= args.warmup_days:
            split = "warmup"
        else:
            split = "dev" if day % 3 == 0 else "test"
        rows.append((f"{doc}m{message}", doc, split, line, None))
    header = [
        "# 手写的单人聊天记录：一个在杭州做产品经理的人一个多月里发出的消息，按天、按会话排好，只有本人发的那一侧。为这份评测集而写，人名、公司和项目是虚构的，地名只用了杭州常见的地方，随仓库以 GPL-3.0 发布。",
        f"# 第 1 到 {args.warmup_days} 天是预热（warmup），之后天数是 3 的倍数的进 dev，其余进 test；doc 是一天里的一段会话。",
        "# 写好之后、跑引擎之前就定稿了，没有按引擎的输出改过一个字。",
    ]
    return header, rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=["tatoeba", "persona"])
    parser.add_argument("source")
    parser.add_argument("--seed", type=int, default=20261028)
    parser.add_argument("--count", type=int, default=1500)
    parser.add_argument("--warmup", type=int, default=400)
    parser.add_argument("--warmup-days", type=int, default=8)
    parser.add_argument("--drop", default="")
    parser.add_argument("--readings", default="")
    args = parser.parse_args()

    header, rows = tatoeba_rows(args) if args.kind == "tatoeba" else persona_rows(args)
    ids = {row[0] for row in rows}
    # 改正文件两份集合共用，只取这一份里的行。
    overrides = {key: value for key, value in load_overrides(args.readings).items() if key[0] in ids}
    applied = len(overrides)
    out = []
    for row_id, doc, split, text, author in rows:
        tokens = tokens_for(text, overrides, row_id)
        line = f"{row_id}\t{doc}\t{split}\t{text}\t{' '.join(tokens)}"
        out.append(line if author is None else f"{line}\t{author}")
    if overrides:
        raise SystemExit(f"unused reading overrides: {sorted(overrides)[:5]}")
    counts = {}
    for _, _, split, text, _ in rows:
        counts.setdefault(split, [0, 0])
        counts[split][0] += 1
        counts[split][1] += han_count(text)
    summary = ", ".join(f"{split} {n} 行 {chars} 字" for split, (n, chars) in sorted(counts.items()))
    print("\n".join(header))
    print("# 切词 jieba 0.42.1，拼音 pypinyin 按整句标再按词归组（ü 写作 v）；" + f"人工改正读音 {applied} 处（--readings）。" + f"{summary}。")
    print("# 生成：scripts/build-typing-set.py " + " ".join(sys.argv[1:2]) + " <source> " + " ".join(a for a in sys.argv[3:] if not a.startswith("/")))
    print("id\tdoc\tsplit\ttext\ttokens" + ("\tauthor" if args.kind == "tatoeba" else ""))
    print("\n".join(out))


if __name__ == "__main__":
    main()
