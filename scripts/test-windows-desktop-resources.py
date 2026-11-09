#!/usr/bin/env python3
"""检查 Windows 发布只准备一次并复用已校验的桌面资源。"""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release-windows.yml"


def job_block(workflow: str, name: str) -> str:
    marker = f"  {name}:\n"
    start = workflow.index(marker)
    following = workflow[start + len(marker) :]
    match = re.search(r"^  [A-Za-z0-9_-]+:\n", following, re.MULTILINE)
    end = start + len(marker) + (match.start() if match else len(following))
    return workflow[start:end]


workflow = WORKFLOW.read_text(encoding="utf-8")
resources = job_block(workflow, "desktop-resources")
release = job_block(workflow, "release")

checks = {
    "资源 job 固定到 source 提交": 'with: {ref: "${{ needs.source.outputs.sha }}", persist-credentials: false}' in resources,
    "资源 job 使用短 Cargo 路径": "./platforms/windows/Use-ShortCargoHome.ps1" in resources,
    "资源 job 运行一次 install_resources": "cargo run --quiet --locked -p msime-client-core --example install_resources" in resources,
    "资源 job 使用 install_resources 返回的代次目录": "if (-not (Test-Path -LiteralPath $reported -PathType Container))" in resources,
    "资源 job 将代次内容导出到 artifact 根目录": "Copy-Item -Path (Join-Path $reported '*') -Destination $artifact -Recurse -Force" in resources,
    "资源 job 上传带版本和提交的 artifact": "msime-desktop-resources-${{ needs.source.outputs.version }}-${{ needs.source.outputs.sha }}" in resources,
    "release 等待资源 job": "needs: [source, rust, official-plugins, desktop-resources]" in workflow,
    "release 下载资源 artifact": "actions/download-artifact" in release and "msime-desktop-resources-${{ needs.source.outputs.version }}-${{ needs.source.outputs.sha }}" in release,
    "release 设置共享资源目录": '"MSIME_DESKTOP_RESOURCES=$resources" >> $env:GITHUB_ENV' in release,
    "release 不再运行 install_resources": "install_resources" not in release,
}

failed = [name for name, passed in checks.items() if not passed]
if failed:
    raise SystemExit("Windows desktop resource workflow contract failed: " + ", ".join(failed))

print("Windows desktop resource workflow: one verified resource artifact is prepared and reused by every edition")
