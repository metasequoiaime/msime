#!/usr/bin/env python3
"""系统设置里的「使用大写锁定键切换 ABC」只作用于声明了 TICapsLockLanguageSwitchCapable 的输入源。

缺这个键不会让安装或注册失败：水杉照常出现在输入菜单里、照常能打字，只是 Caps Lock 永远拿不到系统那层切换——短按它只会打开大写锁定，切到 ABC 之后也切不回水杉。用户把水杉自己的「单按 Caps Lock 切换中英文」关掉、想交给系统时，就正好撞上这一格（#11800）。TIS 运行时把它读成 kTISPropertyInputSourceDoesCapsLockSwitch；苹果自家的中文、日文输入法和鼠须管、豆包都写了它。

所以这里只要两件事：这个键存在，且是布尔真。写成字符串 "true" 或 <false/> 在系统看来都等于没打开，而 plist 语法照样合法，安装、注册、其他检查都不会报。
"""

import plistlib
import sys
from pathlib import Path

KEY = "TICapsLockLanguageSwitchCapable"


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {Path(sys.argv[0]).name} <Info.plist>", file=sys.stderr)
        return 2
    with Path(sys.argv[1]).open("rb") as handle:
        plist = plistlib.load(handle)

    failures: list[str] = []
    if KEY not in plist:
        failures.append(f"Info.plist 没有 {KEY}：系统的「使用大写锁定键切换 ABC」不会作用到水杉（#11800）")
    elif not isinstance(plist[KEY], bool) or not plist[KEY]:
        failures.append(f"Info.plist 的 {KEY} 是 {plist[KEY]!r}，必须是布尔真，否则系统仍不会用 Caps Lock 切换输入源")

    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
