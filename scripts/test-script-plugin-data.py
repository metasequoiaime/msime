#!/usr/bin/env python3
"""脚本插件提交进仓库的生成数据没有漂移：逐个运行 plugins/script/tools/gen_*.py --check。

每个生成器的 --check 重新生成后和仓库里的文件逐字节比较。只读仓库内数据的生成器总能检查；要下载上游数据的（例如 gen_common_chars.py 要锁定的 Unihan zip）在缓存里没有那份数据时打印 skip 并以 0 退出，设了 MSIME_FETCH=1 才下载。所以这个检查在 contracts 工作流里不联网也能跑，scripts/test-script-plugins.sh 也调用它。
"""
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
if not generators:
    print("skip: plugins/script/tools 下没有生成器")
sys.exit(failed)
