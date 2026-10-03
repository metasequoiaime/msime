#!/usr/bin/env python3
"""让 Windows TIP 里 V 模式符号、网址模式符号和网址触发词的拷贝与 Engine 保持一致。

The Engine decides which keys the V (expression) mode spells, and publishes them in View.spelling_symbols; the Windows Server reads them from there. The TSF DLL cannot: it decides whether a key is composition input or a candidate selection before the Server has answered, so `platforms/windows/tsf/Global/LocalModeKeyPolicy.h` keeps its own copy. A symbol added on one side alone splits the two: the TIP would put an operator into its keystroke buffer that the Engine never saw, or send a digit as a selection the Engine expected as input, and the composition the user sees stops matching the one that commits.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ENGINE = ROOT / "crates/engine/src/local/expression.rs"
URL_ENGINE = ROOT / "crates/engine/src/local/url.rs"
TIP = ROOT / "platforms/windows/tsf/Global/LocalModeKeyPolicy.h"


def main() -> int:
    engine = re.search(r'pub const SPELLING_SYMBOLS: &str = "([^"]*)";', ENGINE.read_text(encoding="utf-8"))
    if not engine:
        print(f"{ENGINE.relative_to(ROOT)}: SPELLING_SYMBOLS not found", file=sys.stderr)
        return 1
    tip = re.search(r'inline constexpr wchar_t ExpressionSpellingSymbols\[\] = L"([^"]*)";', TIP.read_text(encoding="utf-8"))
    if not tip:
        print(f"{TIP.relative_to(ROOT)}: ExpressionSpellingSymbols not found", file=sys.stderr)
        return 1
    if engine.group(1) != tip.group(1):
        print(
            f"expression symbols differ: Engine {engine.group(1)!r}, Windows TIP {tip.group(1)!r}",
            file=sys.stderr,
        )
        return 1
    print(f"Windows TIP spells the Engine's V symbols: {engine.group(1)}")
    return check_url()


def check_url() -> int:
    """网址模式：符号表和触发词两份拷贝必须相同，否则 TIP 会把 Engine 当作网址输入的键读成翻页或选词，或者在 Engine 没有进入网址模式时把触发键放进键击缓冲。"""
    engine_text = URL_ENGINE.read_text(encoding="utf-8")
    tip_text = TIP.read_text(encoding="utf-8")
    engine = re.search(r'pub const SPELLING_SYMBOLS: &str = "([^"]*)";', engine_text)
    tip = re.search(r'inline constexpr wchar_t UrlSpellingSymbols\[\] = L"([^"]*)";', tip_text)
    if not engine or not tip:
        print("URL SPELLING_SYMBOLS or UrlSpellingSymbols not found", file=sys.stderr)
        return 1
    if engine.group(1) != tip.group(1):
        print(f"URL symbols differ: Engine {engine.group(1)!r}, Windows TIP {tip.group(1)!r}", file=sys.stderr)
        return 1
    engine_triggers = re.search(r"const TRIGGERS: \[\(&str, &str\); \d+\] = \[(.*?)\];", engine_text)
    tip_triggers = re.search(r"inline constexpr UrlTrigger UrlTriggers\[\] = \{(.*?)\};", tip_text)
    if not engine_triggers or not tip_triggers:
        print("URL TRIGGERS or UrlTriggers not found", file=sys.stderr)
        return 1
    engine_pairs = re.findall(r'\("([^"]*)", "([^"]*)"\)', engine_triggers.group(1))
    tip_pairs = re.findall(r'\{L"([^"]*)", L"([^"]*)"\}', tip_triggers.group(1))
    if not engine_pairs or engine_pairs != tip_pairs:
        print(f"URL triggers differ: Engine {engine_pairs!r}, Windows TIP {tip_pairs!r}", file=sys.stderr)
        return 1
    print(f"Windows TIP spells the Engine's URL symbols {engine.group(1)} and triggers {engine_pairs}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
