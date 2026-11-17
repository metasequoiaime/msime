#!/usr/bin/env python3
"""执行设置截图边界的真实 PowerShell 行为回归。"""

from pathlib import Path
import shutil
import subprocess


ROOT = Path(__file__).resolve().parents[1]
pwsh = shutil.which("pwsh")
if not pwsh:
    print("skipped: pwsh is required for settings screenshot bounds regressions")
    raise SystemExit(0)

subprocess.run(
    [pwsh, "-NoProfile", "-NonInteractive", "-File",
     str(ROOT / "platforms/windows/tests/tools/settings_capture_test.ps1")],
    cwd=ROOT,
    check=True,
    timeout=30,
)
