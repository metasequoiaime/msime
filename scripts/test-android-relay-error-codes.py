#!/usr/bin/env python3
"""Android 自己跑中继的端到端加密驱动时，驱动报的错要按 Rust 那条路同样归类。

`CloudClipboardRelayPolicy.driverFailureCode`（platforms/android/java/app/msime/android/clipboard/CloudClipboardRelayPolicy.java）照抄了 `crates/client-core/src/account/clipboard.rs` 的 `CLIPBOARD_ERROR_CODES`：已知码原样交给界面，其余按状态码归成 `invalid_cloud_clipboard`、`not_found` 或 `relay_unavailable`。Rust 那边加了码而 Java 没跟上，Android 上那个码就会被说成「连不上中继」。这里核对两份清单完全相同。

Java 清单里的 `CODE_*` 常量按同一个类里的定义展开。
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
RUST = ROOT / "crates/client-core/src/account/clipboard.rs"
JAVA = ROOT / "platforms/android/java/app/msime/android/clipboard/CloudClipboardRelayPolicy.java"


def main() -> int:
    rust_text = RUST.read_text(encoding="utf-8")
    rust_block = re.search(r"const CLIPBOARD_ERROR_CODES: &\[&str\] = &\[(.*?)\];", rust_text, re.DOTALL)
    java_text = JAVA.read_text(encoding="utf-8")
    java_block = re.search(r"KNOWN_CODES = Arrays\.asList\((.*?)\);", java_text, re.DOTALL)
    if rust_block is None or java_block is None:
        print("没有读到 CLIPBOARD_ERROR_CODES 或 KNOWN_CODES", file=sys.stderr)
        return 1
    rust_body = re.sub(r"//[^\n]*", "", rust_block.group(1))
    rust = set(re.findall(r'"([a-z_0-9]+)"', rust_body))
    constants = dict(re.findall(r'static final String (CODE_[A-Z_]+) = "([a-z_0-9]+)";', java_text))
    java = set()
    for item in (part.strip() for part in java_block.group(1).split(",")):
        if not item:
            continue
        if item.startswith('"'):
            java.add(item.strip('"'))
        elif item in constants:
            java.add(constants[item])
        else:
            print(f"KNOWN_CODES 里认不出的项: {item}", file=sys.stderr)
            return 1
    ok = True
    for label, missing in (("Rust 有、Java 没有", rust - java), ("Java 有、Rust 没有", java - rust)):
        if missing:
            ok = False
            print(f"{label}: {', '.join(sorted(missing))}", file=sys.stderr)
    if not ok:
        return 1
    print(f"android relay error codes: {len(rust)} known codes match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
