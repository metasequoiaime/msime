# UIA 返回物理像素；只移动合成窗口，不缩小用例要求的尺寸。
function Get-SettingsCapturePlan($bounds, $area) {
    foreach ($value in @($bounds.Left, $bounds.Top, $bounds.Width, $bounds.Height)) {
        if ([double]::IsNaN($value) -or [double]::IsInfinity($value)) { throw 'Settings capture bounds are not finite' }
    }
    $width = [int][Math]::Ceiling($bounds.Width)
    $height = [int][Math]::Ceiling($bounds.Height)
    if ($width -le 0 -or $height -le 0 -or $width -gt $area.Width -or $height -gt $area.Height) {
        throw "Settings screenshot needs ${width}x${height}, but the monitor work area is $($area.Width)x$($area.Height)"
    }
    return [pscustomobject]@{
        X = [int][Math]::Floor([Math]::Min([Math]::Max($bounds.Left, $area.Left), $area.Right - $width))
        Y = [int][Math]::Floor([Math]::Min([Math]::Max($bounds.Top, $area.Top), $area.Bottom - $height))
        Width = $width
        Height = $height
    }
}

function Get-SettingsCaptureBounds($element) {
    $bounds = $element.Current.BoundingRectangle
    $handle = [IntPtr]$element.Current.NativeWindowHandle
    $area = [System.Windows.Forms.Screen]::FromHandle($handle).WorkingArea
    $plan = Get-SettingsCapturePlan $bounds $area
    if ($plan.X -ne $bounds.Left -or $plan.Y -ne $bounds.Top) {
        if (-not [SettingsWindowProbe]::SetWindowPos($handle, [IntPtr]::Zero, $plan.X, $plan.Y, 0, 0, 0x0015)) {
            throw 'Could not move the synthetic settings window into the monitor work area'
        }
        Start-Sleep -Milliseconds 200
        $bounds = $element.Current.BoundingRectangle
    }
    $rectangle = [Drawing.Rectangle]::FromLTRB([int][Math]::Floor($bounds.Left), [int][Math]::Floor($bounds.Top),
        [int][Math]::Ceiling($bounds.Right), [int][Math]::Ceiling($bounds.Bottom))
    if (-not $area.Contains($rectangle)) { throw 'Settings screenshot would be clipped by the monitor work area' }
    return $rectangle
}
