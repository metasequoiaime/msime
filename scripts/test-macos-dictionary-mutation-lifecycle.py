#!/usr/bin/env python3
"""检查 macOS 个人词典的异步变更是否绑定当前页面代次。"""

from pathlib import Path
import sys


def main() -> int:
    source = Path(sys.argv[1]) if len(sys.argv) == 2 else Path("platforms/macos/src/dictionary/DictionaryWindowController.mm")
    text = source.read_text(encoding="utf-8")
    required = (
        "- (BOOL)isCurrentDictionaryMutation:(NSUInteger)generation kind:(NSString *)kind offset:(NSUInteger)offset",
        "NSUInteger mutationGeneration = controller.refreshGeneration;",
        "NSString *mutationKind = [[controller selectedKind] copy];",
        "[current isCurrentDictionaryMutation:mutationGeneration kind:mutationKind offset:mutationOffset]",
    )
    missing = [item for item in required if item not in text]
    if missing:
        raise AssertionError(f"macOS dictionary mutation callbacks miss lifecycle guard(s): {missing}")
    if text.count("[current isCurrentDictionaryMutation:mutationGeneration kind:mutationKind offset:mutationOffset]") < 4:
        raise AssertionError("macOS dictionary mutation callbacks do not guard every async operation")
    print("macOS dictionary mutations are bound to the current page generation")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
