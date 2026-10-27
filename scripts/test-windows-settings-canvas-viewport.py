#!/usr/bin/env python3
"""验证真实画布滚动策略能识别被 UIA 裁到视口边缘的图片。"""

from pathlib import Path
import shutil
import subprocess


ROOT = Path(__file__).resolve().parent.parent
pwsh = shutil.which("pwsh")
if not pwsh:
    print("skipped: pwsh is required for canvas viewport regressions")
    raise SystemExit(0)

script = r"""
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$tokens = $null
$errors = $null
$path = Join-Path (Get-Location) 'platforms/windows/tests/tools/settings_ui.ps1'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Settings smoke parse failed' }
$function = @($ast.EndBlock.Statements | Where-Object {
    $_ -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
    $_.Name -eq 'Get-CanvasPreviewScrollDirection'
})
if ($function.Count -ne 1) { throw 'Canvas viewport policy missing' }
Invoke-Expression $function[0].Extent.Text
function Bounds([double]$top, [double]$bottom) {
    return [pscustomobject]@{ Top = $top; Bottom = $bottom; Height = $bottom - $top }
}
$viewport = Bounds 48 620
$screen = Bounds 0 600
# UIA 会返回裁剪后的矩形；贴底或贴顶都不能当成已经完整进入视口。
$cases = @(
    @{ Name = '完整图片'; Top = 100; Bottom = 200; Expected = 0 },
    @{ Name = '正文底部裁剪'; Top = 608; Bottom = 620; Expected = 1 },
    @{ Name = '屏幕底部裁剪'; Top = 588; Bottom = 600; Expected = 1 },
    @{ Name = '屏幕之外'; Top = 600; Bottom = 612; Expected = 1 },
    @{ Name = '正文顶部裁剪'; Top = 48; Bottom = 60; Expected = -1 },
    @{ Name = '底部余量不足'; Top = 500; Bottom = 595; Expected = 1 },
    @{ Name = '顶部余量不足'; Top = 52; Bottom = 100; Expected = -1 },
    @{ Name = '尚未排版'; Top = 0; Bottom = 0; Expected = 1 }
)
foreach ($case in $cases) {
    $actual = Get-CanvasPreviewScrollDirection (Bounds $case.Top $case.Bottom) $viewport $screen
    if ($actual -ne $case.Expected) { throw "$($case.Name)：期望 $($case.Expected)，实际 $actual" }
}
Write-Host "画布可见视口滚动回归：$($cases.Count) 项通过"
"""
result = subprocess.run(
    [pwsh, "-NoProfile", "-NonInteractive", "-Command", script],
    cwd=ROOT,
    capture_output=True,
    text=True,
    timeout=20,
)
if result.returncode:
    raise SystemExit(result.stdout + result.stderr)
print(result.stdout.strip())
