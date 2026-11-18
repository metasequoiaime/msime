$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'settings_capture.ps1')

function Bounds([double]$left, [double]$top, [double]$width, [double]$height) {
    return [pscustomobject]@{ Left = $left; Top = $top; Width = $width; Height = $height;
        Right = $left + $width; Bottom = $top + $height }
}
function Expect-Refused($bounds, $area) {
    try { Get-SettingsCapturePlan $bounds $area | Out-Null } catch { return }
    throw 'Clipped or invalid screenshots must be refused'
}

$small = Bounds 0 0 1024 728
# 合成复现失败截图的尺寸；起点只是测试值，780 像素的高度即使移到原点也放不进工作区。
Expect-Refused (Bounds 124 156 1000 780) $small
Expect-Refused (Bounds 0 0 1100 600) $small
Expect-Refused (Bounds 0 0 0 600) $small
Expect-Refused (Bounds 0 0 -10 600) $small
Expect-Refused (Bounds ([double]::NaN) 0 1000 600) $small
Expect-Refused (Bounds 0 0 ([double]::PositiveInfinity) 600) $small

foreach ($case in @(
    @{ Window = (Bounds 124 156 1000 700); Area = $small; X = 24; Y = 28 },
    @{ Window = (Bounds 12 20 800 600); Area = $small; X = 12; Y = 20 },
    @{ Window = (Bounds -20 -10 800 600); Area = $small; X = 0; Y = 0 },
    @{ Window = (Bounds 0 0 1024 728); Area = $small; X = 0; Y = 0 },
    @{ Window = (Bounds -100 -20 1000 700); Area = (Bounds -1280 -100 1280 984); X = -1000; Y = -20 },
    @{ Window = (Bounds 124 156 1000 780); Area = (Bounds 0 0 1920 1040); X = 124; Y = 156 }
)) {
    $plan = Get-SettingsCapturePlan $case.Window $case.Area
    if ($plan.X -ne $case.X -or $plan.Y -ne $case.Y) { throw "Expected position $($case.X),$($case.Y); actual $($plan.X),$($plan.Y)" }
    if ($plan.Width -ne $case.Window.Width -or $plan.Height -ne $case.Window.Height) { throw 'Screenshot preparation changed the regression window size' }
}
Write-Host 'Settings screenshot bounds: clipped desktop, placement, negative monitor origins and unchanged dimensions pass'
