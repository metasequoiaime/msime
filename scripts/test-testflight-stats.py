#!/usr/bin/env python3
"""在仓库契约门禁中运行无需真实凭据的 TestFlight 汇总测试。"""
import shutil
import subprocess
from pathlib import Path

if __name__ == "__main__":
    node = shutil.which("node")
    if node is None:
        print("skipped: node not installed")
    else:
        raise SystemExit(subprocess.call([node, "--test", str(Path(__file__).with_name("export-testflight-stats.test.mjs"))]))
