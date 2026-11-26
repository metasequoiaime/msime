# 「?」气泡（main.cpp 的 info_tip、InfoTipHost.h）点开之后要能被普通操作收起：点页面别处、按 Esc、滚轮、换页、再点同一颗问号都收起，点开另一颗时原来那颗先收起。
# 计数读的是桌面上的 ToolTip 自动化元素，必须在真实 WinUI 窗口里跑；这些气泡是独立弹出窗口，不在宿主窗口的 UIA 子树里。
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SettingsInfoTipKeys {
    [DllImport("user32.dll")] static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    public static void Escape() {
        keybd_event(0x1B, 0, 0, UIntPtr.Zero);
        System.Threading.Thread.Sleep(40);
        keybd_event(0x1B, 0, 2, UIntPtr.Zero);
    }
}
'@

function Get-SettingsInfoTip($window) {
    $kind = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ToolTip)
    $owner = $window.Current.ProcessId
    return @([System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, $kind) |
        Where-Object { $_.Current.ProcessId -eq $owner })
}

# 按钮的读屏名称是「<标题>的说明：<正文>」，正文会随文案变化，所以按标题前缀找，不按全文比对。
function Find-InfoTipButton($window, [string]$title) {
    $kind = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
    $prefix = "$($title)的说明："
    foreach ($button in $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $kind)) {
        if ($button.Current.Name.StartsWith($prefix)) { return $button }
    }
    return $null
}

function Invoke-InfoTipClick($element) {
    $bounds = $element.Current.BoundingRectangle
    if ($bounds.Width -le 0 -or $bounds.Height -le 0) { throw '要点的元素没有可点击范围' }
    [SettingsWindowProbe]::MoveMouse([int]($bounds.X + $bounds.Width / 2), [int]($bounds.Y + $bounds.Height / 2))
    Start-Sleep -Milliseconds 200
    [SettingsWindowProbe]::MouseButton($true)
    Start-Sleep -Milliseconds 60
    [SettingsWindowProbe]::MouseButton($false)
    Start-Sleep -Milliseconds 700
}

# 指针停在内容区右下角的空白处：那里没有别的「?」，悬停产生的气泡不会混进计数，也不会误点到开关。
function Set-InfoTipPointerAway($window) {
    $bounds = (Find-ById $window 'settings-content').Current.BoundingRectangle
    [SettingsWindowProbe]::MoveMouse([int]($bounds.X + $bounds.Width - 40), [int]($bounds.Y + $bounds.Height - 40))
    Start-Sleep -Milliseconds 800
}

function Assert-SettingsInfoTipCount($window, [int]$expected, [string]$step) {
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $tips = @(Get-SettingsInfoTip $window)
        if ($tips.Count -eq $expected) { return $tips }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    Save-Window $window 'info-tip-failure.png'
    $names = ($tips | ForEach-Object { $_.Current.Name }) -join ' / '
    throw "「?」气泡没有在该收起时收起（$($step)）：应该有 $($expected) 个，实际 $($tips.Count) 个：$($names)"
}

# 点开一颗「?」并返回它固定的正文，正文用来核对气泡是不是这一行自己的。
function Open-SettingsInfoTip($window, [string]$title) {
    $button = Find-InfoTipButton $window $title
    if (-not $button) { throw "找不到「$($title)」旁边的「?」" }
    $name = $button.Current.Name
    $text = $name.Substring($name.IndexOf([char]0xFF1A) + 1)
    # 窗口不在前台时这一次点击只用来激活窗口，落不到按钮上；气泡没出来就说明那一下被吞了，再点一次不会把没开的气泡关掉。
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        [SettingsWindowProbe]::SetForegroundWindow([IntPtr]$window.Current.NativeWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 200
        Invoke-InfoTipClick $button
        if (@(Get-SettingsInfoTip $window).Count -gt 0) { break }
        $button = Find-InfoTipButton $window $title
        if (-not $button) { break }
    }
    Set-InfoTipPointerAway $window
    return $text
}

function Assert-InfoTipDismissal($window) {
    Open-Page $window 'typing'
    Select-PageTab $window 'input-tab-advanced' '启用模糊音'
    Start-Sleep -Milliseconds 600
    Set-InfoTipPointerAway $window
    Assert-SettingsInfoTipCount $window 0 '刚打开「高级选项」页' | Out-Null

    # 点一下「?」把气泡固定住，这是唯一的打开方式；之后每一种普通操作都要能把它收起。
    $shortcut = Open-SettingsInfoTip $window 'Shift+字母进入快捷模式'
    $tips = @(Assert-SettingsInfoTipCount $window 1 '点开「Shift+字母进入快捷模式」的「?」')
    if ($tips[0].Current.Name -ne $shortcut) { throw "气泡内容不是这一行的说明：$($tips[0].Current.Name)" }

    # 点页面上的普通文字就该收起：iOS、Android 和共享设置页都是点别处收起。
    $label = Find-ByName (Find-ById $window 'settings-content') '快捷模式'
    if (-not $label) { throw '「高级选项」页缺少「快捷模式」分组标题' }
    Invoke-InfoTipClick $label
    Assert-SettingsInfoTipCount $window 0 '点页面别处' | Out-Null

    Open-SettingsInfoTip $window 'Shift+字母进入快捷模式' | Out-Null
    Assert-SettingsInfoTipCount $window 1 '再次点开「Shift+字母进入快捷模式」的「?」' | Out-Null
    [SettingsWindowProbe]::SetForegroundWindow([IntPtr]$window.Current.NativeWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 200
    [SettingsInfoTipKeys]::Escape()
    Assert-SettingsInfoTipCount $window 0 '按 Esc' | Out-Null

    # 气泡位置只在打开时量一次，滚动后它就不再贴着所在那行了，所以滚动也要收起。滚轮用真实鼠标输入驱动，与「工具提示滚动后离开」一致。
    Open-SettingsInfoTip $window 'Shift+字母进入快捷模式' | Out-Null
    Assert-SettingsInfoTipCount $window 1 '滚动前点开「Shift+字母进入快捷模式」的「?」' | Out-Null
    $bounds = (Find-ById $window 'settings-content').Current.BoundingRectangle
    [SettingsWheelHitProbe]::Wheel([int]($bounds.X + 60), [int]($bounds.Y + 120), -120)
    Assert-SettingsInfoTipCount $window 0 '滚轮' | Out-Null
    $scroll = (Find-ById $window 'settings-content').GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
    if ($scroll.Current.VerticallyScrollable) { $scroll.SetScrollPercent(-1, 0) }
    Start-Sleep -Milliseconds 400

    # 再点同一颗「?」是关掉，不是叠第二层。
    Open-SettingsInfoTip $window 'Shift+字母进入快捷模式' | Out-Null
    Assert-SettingsInfoTipCount $window 1 '点同一颗「?」之前' | Out-Null
    $same = Find-InfoTipButton $window 'Shift+字母进入快捷模式'
    if (-not $same) { throw '「Shift+字母进入快捷模式」旁边的「?」点开后消失了' }
    Invoke-InfoTipClick $same
    Assert-SettingsInfoTipCount $window 0 '再点同一颗「?」' | Out-Null

    # 点开第二颗「?」时第一颗必须先收起，否则几颗气泡会像截图里那样叠在一起。
    Open-SettingsInfoTip $window 'Shift+字母进入快捷模式' | Out-Null
    Assert-SettingsInfoTipCount $window 1 '点开第一颗「?」' | Out-Null
    $fuzzy = Open-SettingsInfoTip $window '启用模糊音'
    $tips = @(Assert-SettingsInfoTipCount $window 1 '点开第二颗「?」')
    if ($tips[0].Current.Name -ne $fuzzy) { throw "剩下的是第一颗「?」的气泡：$($tips[0].Current.Name)" }

    Open-Page $window 'candidate'
    Assert-SettingsInfoTipCount $window 0 '切到别的页面' | Out-Null
    Open-Page $window 'typing'
    Write-Host 'Info tip: dismissing on click, Esc, scroll and page switch; one bubble at a time'
}
