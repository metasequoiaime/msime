#!/usr/bin/env python3
"""脚本插件提交进仓库的生成数据没有漂移：逐个运行 plugins/script/tools/gen_*.py --check。

每个生成器的 --check 重新生成后和仓库里的文件逐字节比较。只读仓库内数据的生成器总能检查；要下载上游数据的（例如 gen_common_chars.py 要锁定的 Unihan zip）在缓存里没有那份数据时打印 skip 并以 0 退出，设了 MSIME_FETCH=1 才下载。

两处调用它：scripts/run-checks.sh 按文件名自动发现它（contracts 工作流不联网，所以那里 common-chars.bin 的检查总是 skip）；scripts/test-script-plugins.sh 也调用它，ci-platforms.yml 的 Script plugins 作业给它设 MSIME_FETCH=1 和缓存，common-chars.bin 在那里真正核对。设了 MSIME_REQUIRE_SCRIPT_PLUGINS=1（那个作业这样设）时，任何生成器打印 skip 都算失败，免得下载被悄悄关掉后检查又退回 skip。
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

failed = 0
generators = sorted((ROOT / "plugins/script/tools").glob("gen_*.py"))
for generator in generators:
    name = generator.relative_to(ROOT)
    try:
        result = subprocess.run([sys.executable, str(generator), "--check"], cwd=ROOT, capture_output=True, text=True, timeout=300)
    except subprocess.TimeoutExpired:
        print(f"FAIL {name}: --check 超过 300 秒")
        failed = 1
        continue
    output = (result.stdout + result.stderr).strip()
    print(f"{name}: {output}")
    if result.returncode != 0:
        print(f"FAIL {name}: --check 退出码 {result.returncode}")
        failed = 1
    elif output.startswith("skip") and os.environ.get("MSIME_REQUIRE_SCRIPT_PLUGINS") == "1":
        print(f"FAIL {name}: MSIME_REQUIRE_SCRIPT_PLUGINS=1 时不能跳过（设 MSIME_FETCH=1 让它下载数据）")
        failed = 1
if not generators:
    print("skip: plugins/script/tools 下没有生成器")
sys.exit(failed)
