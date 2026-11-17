# 只捕获本脚本的合成窗口，验证真实屏幕边界与角落像素。
param([Parameter(Mandatory = $true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing, System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SettingsWindowProbe {
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
}
'@
. (Join-Path $PSScriptRoot 'settings_capture.ps1')
$tokens = $null
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'settings_ui.ps1'), [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Settings smoke parse failed' }
$capture = @($ast.EndBlock.Statements | Where-Object {
    $_ -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $_.Name -eq 'Save-Window'
})
if ($capture.Count -ne 1) { throw 'Settings screenshot entry missing' }
Invoke-Expression $capture[0].Extent.Text
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$outputPath = (Resolve-Path $OutputDirectory).Path
$form = New-Object Windows.Forms.Form
try {
    $form.FormBorderStyle = [Windows.Forms.FormBorderStyle]::None
    $form.StartPosition = [Windows.Forms.FormStartPosition]::Manual
    $form.Size = [Drawing.Size]::new(640, 480)
    $form.TopMost = $true
    $form.Text = 'Synthetic settings capture fixture'
    # 色带覆盖全部角落，屏幕抓图若被裁剪，右下角不会保持预期颜色。
    $colors = @([Drawing.Color]::Red, [Drawing.Color]::Lime, [Drawing.Color]::Blue, [Drawing.Color]::Yellow)
    for ($index = 0; $index -lt 4; $index++) {
        $panel = New-Object Windows.Forms.Panel
        $panel.Location = [Drawing.Point]::new(($index % 2) * 320, [int][Math]::Floor($index / 2) * 240)
        $panel.Size = [Drawing.Size]::new(320, 240)
        $panel.BackColor = $colors[$index]
        $form.Controls.Add($panel)
    }
    # 添加色条，让生产函数原有的非空白颜色检查也走完整。
    for ($index = 0; $index -lt 12; $index++) {
        $panel = New-Object Windows.Forms.Panel
        $panel.Location = [Drawing.Point]::new((40 + $index * 40), 120)
        $panel.Size = [Drawing.Size]::new(35, 220)
        $panel.BackColor = [Drawing.Color]::FromArgb(255, 20 * $index, 200 - 10 * $index, 50 + 15 * $index)
        $form.Controls.Add($panel)
        $panel.BringToFront()
    }
    $form.Show()
    $area = [Windows.Forms.Screen]::FromHandle($form.Handle).WorkingArea
    if (-not [SettingsWindowProbe]::SetWindowPos($form.Handle, [IntPtr]::Zero, $area.Right - 580, $area.Bottom - 400, 0, 0, 0x0015)) { throw 'Fixture move failed' }
    [Windows.Forms.Application]::DoEvents()
    $element = [Windows.Automation.AutomationElement]::FromHandle($form.Handle)
    $before = $element.Current.BoundingRectangle
    if ($before.Right -le $area.Right -and $before.Bottom -le $area.Bottom) { throw 'Fixture did not reproduce offscreen placement' }
    Save-Window $element 'complete-synthetic-window.png'
    $bounds = Get-SettingsCaptureBounds $element
    [Windows.Forms.Application]::DoEvents()
    Start-Sleep -Milliseconds 300
    if ($bounds.Width -ne 640 -or $bounds.Height -ne 480 -or -not $area.Contains($bounds)) { throw 'Fixture was resized or remains clipped' }
    $bitmap = [Drawing.Bitmap]::FromFile((Join-Path $outputPath 'complete-synthetic-window.png'))
    try {
        $points = @(@(10, 10), @(630, 10), @(10, 470), @(630, 470))
        for ($index = 0; $index -lt 4; $index++) {
            if ($bitmap.GetPixel($points[$index][0], $points[$index][1]).ToArgb() -ne $colors[$index].ToArgb()) { throw 'Screenshot corner pixels were clipped or occluded' }
        }
    } finally { $bitmap.Dispose() }
    Write-Host 'Settings capture: real offscreen window moved without resize; four screenshot corners pass'
} finally { $form.Close(); $form.Dispose() }
