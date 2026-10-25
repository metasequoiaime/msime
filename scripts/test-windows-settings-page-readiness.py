#!/usr/bin/env python3
"""用受控 UIA 名称序列运行真实的 Windows 设置页首断言。"""

from pathlib import Path
import shutil
import subprocess


ROOT = Path(__file__).resolve().parent.parent
pwsh = shutil.which("pwsh")
if not pwsh:
    print("skipped: pwsh is required for settings page readiness regressions")
    raise SystemExit(0)

# 只替换 UIA 查询边界；等待与文案断言直接从验收脚本加载，不启动 WinUI 或读取用户设置。
script = r"""
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$tokens = $null
$errors = $null
$path = Join-Path (Get-Location) 'platforms/windows/tests/tools/settings_ui.ps1'
$ast = [System.Management.Automation.Language.Parser]::ParseFile($path, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Settings smoke parse failed' }
foreach ($statement in $ast.EndBlock.Statements) {
    if ($statement -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
        $statement.Name -in @('Assert-PageIntro', 'Wait-PageIntroName')) {
        Invoke-Expression $statement.Extent.Text
    }
}
Add-Type 'namespace System.Windows.Automation { public class ElementNotAvailableException : System.Exception {} }'
# 最后一项要等满超时；真实验收的 15 秒在这里只是白等，2 秒足够覆盖前几项的几轮 200 毫秒重试。
$script:pageIntroNameSeconds = 2
$script:names = [System.Collections.Generic.Queue[string]]::new()
$script:stale = $false
$script:missing = $true
$script:loadError = $false
function Find-ById($parent, [string]$id) {
    if ($id -eq 'settings-content') { return [pscustomobject]@{ Id = $id } }
    if ($id -eq 'settings-load-error' -and $script:loadError) { return [pscustomobject]@{ Id = $id } }
    if ($id -eq 'settings-page-account') {
        if ($script:missing) {
            $script:missing = $false
            return $null
        }
        if ($script:stale) {
            $script:stale = $false
            throw [System.Windows.Automation.ElementNotAvailableException]::new()
        }
        $name = if ($script:names.Count) { $script:names.Dequeue() } else { '' }
        return [pscustomobject]@{ Current = [pscustomobject]@{ Name = $name } }
    }
    return $null
}
function Expect-Failure([scriptblock]$action, [string]$message) {
    try { & $action } catch {
        if ($_.Exception.Message -notlike $message) { throw }
        return
    }
    throw "Expected failure: $message"
}
$expected = '管理账号与跨设备设置同步。'
foreach ($name in @('', '   ', $expected)) { $script:names.Enqueue($name) }
Assert-PageIntro ([pscustomobject]@{}) 'account' $expected
if ($script:names.Count) { throw 'Page assertion returned before its name was available' }
Write-Host 'Settings page assertion waits for its accessible name'
$script:stale = $true
$script:names.Enqueue($expected)
Assert-PageIntro ([pscustomobject]@{}) 'account' $expected
Write-Host 'Settings page assertion refreshes unavailable nodes'
$script:names.Enqueue('错误的非空说明。')
$script:names.Enqueue($expected)
Expect-Failure { Assert-PageIntro ([pscustomobject]@{}) 'account' $expected } '*does not follow its tab*'
if ($script:names.Count -ne 1) { throw 'Wrong nonempty text was silently retried' }
$script:loadError = $true
Expect-Failure { Assert-PageIntro ([pscustomobject]@{}) 'account' $expected } '*preferences were rejected*'
if ($script:names.Count -ne 1) { throw 'Load error was silently retried' }
$script:loadError = $false
$script:names.Clear()
Write-Host 'Settings page assertion preserves text and load failure checks'
Expect-Failure { Assert-PageIntro ([pscustomobject]@{}) 'account' $expected } '*did not expose a name: account*'
Write-Host 'Settings page assertion times out when the name stays empty'
"""

result = subprocess.run(
    [pwsh, "-NoProfile", "-NonInteractive", "-Command", script],
    cwd=ROOT,
    capture_output=True,
    text=True,
    timeout=40,
)
if result.returncode:
    raise SystemExit(result.stdout + result.stderr)
print(result.stdout.strip())
