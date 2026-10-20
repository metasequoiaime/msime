#!/usr/bin/env python3
"""核对 docs/plugin-architecture.html 与 scripts/generate_plugin_architecture.py 的生成结果逐字节一致。

原理图由脚本生成后提交，手改 HTML、或者改了脚本却没重新生成，都会让两者对不上；这里把脚本生成到临时目录再比较，不一致时提示重新生成。
"""

import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "scripts", "generate_plugin_architecture.py")
COMMITTED = os.path.join(ROOT, "docs", "plugin-architecture.html")


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "plugin-architecture.html")
        subprocess.run([sys.executable, SCRIPT, out], check=True)
        with open(out, "rb") as f:
            generated = f.read()
    with open(COMMITTED, "rb") as f:
        committed = f.read()
    if generated != committed:
        print("docs/plugin-architecture.html 与生成脚本的输出不一致，请运行 python3 scripts/generate_plugin_architecture.py 后提交结果", file=sys.stderr)
        return 1
    print("plugin-architecture: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
