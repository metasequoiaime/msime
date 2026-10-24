# 原生滚轮回归：只操作调用者打开的合成设置窗口，不改系统滚轮设置。
Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class SettingsWheelHitProbe {
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] private struct MouseInput {
        public int X, Y;
        public uint Data, Flags, Time;
        public IntPtr ExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)] private struct Input {
        public uint Type;
        public MouseInput Mouse;
    }
    [DllImport("user32.dll", SetLastError = true)] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll", SetLastError = true)] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll", SetLastError = true)] private static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] private static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] private static extern IntPtr GetAncestor(IntPtr window, uint flags);
    [DllImport("user32.dll", SetLastError = true)] private static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam, uint flags, uint timeout, out UIntPtr result);
    // 定向消息只作诊断；它绕过系统输入路由，不能代替真实滚轮的通过条件。
    public static string DirectedWheel(IntPtr owner, int x, int y, int delta, bool child) {
        if (!SetCursorPos(x, y)) throw new Win32Exception(Marshal.GetLastWin32Error());
        System.Threading.Thread.Sleep(50);
        var target = child ? WindowFromPoint(new Point { X = x, Y = y }) : owner;
        if (target == IntPtr.Zero || GetAncestor(target, 2) != owner)
            throw new InvalidOperationException("Directed wheel target is outside the synthetic settings window");
        uint coordinates = unchecked((uint)((x & 0xffff) | ((y & 0xffff) << 16)));
        UIntPtr result;
        if (SendMessageTimeoutW(target, 0x020A, new UIntPtr(unchecked((uint)delta << 16)), new IntPtr((long)coordinates), 3, 2000, out result) == IntPtr.Zero)
            throw new InvalidOperationException("Directed wheel delivery failed or timed out");
        return target.ToInt64().ToString("X");
    }
    public static void Wheel(int x, int y, int delta) {
        if (!SetCursorPos(x, y)) throw new Win32Exception(Marshal.GetLastWin32Error());
        System.Threading.Thread.Sleep(50);
        var input = new Input { Mouse = new MouseInput { Data = unchecked((uint)delta), Flags = 0x0800 } };
        if (SendInput(1, new[] { input }, Marshal.SizeOf(typeof(Input))) != 1)
            throw new Win32Exception(Marshal.GetLastWin32Error());
    }
}
'@

function Assert-SettingsWheelHitRegions($window, [IntPtr]$handle, [string]$label) {
    $original = $window.Current.BoundingRectangle
    $cursor = New-Object SettingsWheelHitProbe+Point
    if (-not [SettingsWheelHitProbe]::GetCursorPos([ref]$cursor)) { throw 'Wheel probe requires an interactive desktop' }
    $scale = [SettingsWheelHitProbe]::GetDpiForWindow($handle) / 96.0
    if ($scale -le 0) { throw 'Wheel probe could not read the window DPI' }
    $results = New-Object 'System.Collections.Generic.List[object]'
    $comboPattern = $null
    $pattern = $null

    # UIA 只设置起点并读结果；每次被测滚动均由真实鼠标滚轮输入驱动。
    function Read-WheelOffset {
        $previous = $pattern.Current.VerticalScrollPercent
        $stable = 0
        for ($attempt = 0; $attempt -lt 30; $attempt++) {
            Start-Sleep -Milliseconds 100
            $next = $pattern.Current.VerticalScrollPercent
            if ([Math]::Abs($next - $previous) -lt 0.001) { $stable++ } else { $stable = 0 }
            if ($stable -ge 3) { return $next }
            $previous = $next
        }
        throw 'Wheel scroll animation did not settle'
    }
    # UIA 的矩形使用物理像素；百分比须按实际滚动范围换算，避免把亚像素舍入当成边界移动。
    function Get-WheelRangePixels {
        $viewSize = $pattern.Current.VerticalViewSize
        $height = $content.Current.BoundingRectangle.Height
        if ($viewSize -le 0 -or $viewSize -ge 100 -or $height -le 0) { throw 'Wheel fixture has no measurable scroll range' }
        return $height * (100 / $viewSize - 1)
    }
    function Set-WheelOffset([double]$percent) {
        $pattern.SetScrollPercent(-1, $percent)
        $actual = Read-WheelOffset
        if ([Math]::Abs($actual - $percent) / 100 * (Get-WheelRangePixels) -gt 1) { throw "Wheel starting offset was not applied: $percent -> $actual" }
    }
    function Send-CheckedWheel([string]$region, [double]$x, [double]$y, [int]$delta, [bool]$moves) {
        $before = Read-WheelOffset
        [SettingsWheelHitProbe]::Wheel([int]$x, [int]$y, $delta)
        Start-Sleep -Milliseconds 150
        $after = Read-WheelOffset
        $range = Get-WheelRangePixels
        $changePixels = ($after - $before) / 100 * $range
        $results.Add([pscustomobject]@{region = $region; delta = $delta; before = $before; after = $after;
            viewportPixels = $content.Current.BoundingRectangle.Height; viewSize = $pattern.Current.VerticalViewSize;
            rangePixels = $range; changePixels = $changePixels})
        # 阳性对照同样要求超过一个物理像素，边界和弹出层最多允许一个栅格像素的舍入。
        $valid = if ($moves) { if ($delta -lt 0) { $changePixels -gt 1 } else { $changePixels -lt -1 } }
                 else { [Math]::Abs($changePixels) -le 1 }
        if (-not $valid) { throw "Native wheel $label $region failed: $before -> $after ($changePixels px)" }
        Write-Host "Native wheel $label $region passed: $before -> $after ($changePixels px)"
    }
    function Send-PageWheel([string]$region, [double]$x, [double]$y, [int]$delta, [bool]$moves) {
        $bounds = $content.Current.BoundingRectangle
        if ($x -le $bounds.Left -or $x -ge $bounds.Right -or $y -le $bounds.Top -or $y -ge $bounds.Bottom) {
            throw "Wheel region is outside the content viewport: $region"
        }
        Send-CheckedWheel $region $x $y $delta $moves
    }
    try {
        # 缩小高度保证当前快捷键页确有滚动范围；原窗口尺寸和指针位置在最后恢复。
        if (-not [SettingsWindowProbe]::SetWindowPos($handle, [IntPtr]::Zero, [int]$original.X, [int]$original.Y,
                [int]$original.Width, [int](600 * $scale), 0x0040)) { throw 'Wheel probe could not resize the window' }
        if ([SettingsWheelHitProbe]::GetForegroundWindow() -ne $handle -and
                -not [SettingsWheelHitProbe]::SetForegroundWindow($handle)) { throw 'Wheel probe could not activate its settings window' }
        Start-Sleep -Milliseconds 500
        $content = Find-ById $window 'settings-content'
        $pattern = $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
        if (-not $pattern.Current.VerticallyScrollable) { throw 'Wheel fixture has no vertical scroll range' }
        Set-WheelOffset 0
        $heading = Find-ByName $content '输入模式切换'
        if (-not $heading -or $heading.Current.IsOffscreen) { throw 'Wheel fixture group heading is not visible' }
        $bounds = $content.Current.BoundingRectangle
        $middle = $bounds.Left + $bounds.Width / 2
        $title = $heading.Current.BoundingRectangle
        # 卡片内部先作为阳性对照，避免把没有投递成功的输入当成命中检测回归。
        Send-PageWheel 'card-down-control' $middle ($title.Bottom + 32 * $scale) -120 $true

        # 新版快捷键页只剩一个组；在输入页的两个实际卡片之间验证 issue 中的空白区域。
        Open-Page $window 'typing'
        Select-PageTab $window 'input-tab-advanced' 'Shift+字母进入快捷模式'
        $content = Find-ById $window 'settings-content'
        $pattern = $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
        if (-not $pattern.Current.VerticallyScrollable) { throw 'Inter-card fixture has no vertical scroll range' }
        Set-WheelOffset 0
        $between = Find-ByName $content '逐个模式'
        if (-not $between -or $between.Current.IsOffscreen) { throw 'Inter-card fixture heading is not visible' }
        $title = $between.Current.BoundingRectangle
        Send-PageWheel 'inter-card-down' $middle ($title.Top - 10 * $scale) -120 $true
        $title = (Find-ByName $content '逐个模式').Current.BoundingRectangle
        Send-PageWheel 'inter-card-up' $middle ($title.Top - 10 * $scale) 120 $true

        # 对照报告中的窗口定向输入：分别投递给顶层窗口与指针下的子窗口，仅记录，不把它算作原生滚轮验收。
        if ($label -eq 'light-1000') {
            foreach ($route in @('root', 'child')) {
                foreach ($area in @('card', 'gap')) {
                    Set-WheelOffset 0
                    $title = (Find-ByName $content $(if ($area -eq 'card') { '快捷模式' } else { '逐个模式' })).Current.BoundingRectangle
                    $y = if ($area -eq 'card') { $title.Bottom + 32 * $scale } else { $title.Top - 10 * $scale }
                    $before = Read-WheelOffset
                    $target = [SettingsWheelHitProbe]::DirectedWheel($handle, [int]$middle, [int]$y, -120, ($route -eq 'child'))
                    $after = Read-WheelOffset
                    $results.Add([pscustomobject]@{region = "diagnostic-directed-$route-$area"; delta = -120; before = $before; after = $after; target = $target})
                    Write-Host "Directed wheel diagnostic $route $area`: $before -> $after (target=$target)"
                }
            }
        }

        Open-Page $window 'shortcuts'
        Select-PageTab $window 'shortcuts-tab-settings' 'Ctrl+Space（由 Windows 管理）'
        $content = Find-ById $window 'settings-content'
        $pattern = $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
        Set-WheelOffset 0
        $bounds = $content.Current.BoundingRectangle
        $middle = $bounds.Left + $bounds.Width / 2
        $title = (Find-ByName $content '输入模式切换').Current.BoundingRectangle
        Send-PageWheel 'group-gap-down' $middle ($title.Top - 10 * $scale) -120 $true
        $title = (Find-ByName $content '输入模式切换').Current.BoundingRectangle
        Send-PageWheel 'group-gap-up' $middle ($title.Top - 10 * $scale) 120 $true
        foreach ($side in @('left', 'right')) {
            Set-WheelOffset 0
            $x = if ($side -eq 'left') { $bounds.Left + 8 * $scale } else { $bounds.Right - 24 * $scale }
            $y = $bounds.Top + $bounds.Height / 2
            Send-PageWheel "$side-padding-down" $x $y -120 $true
            Send-PageWheel "$side-padding-up" $x $y 120 $true
        }
        Set-WheelOffset 0
        Send-PageWheel 'top-boundary' $middle ($bounds.Top + 8 * $scale) 120 $false
        Send-PageWheel 'top-padding-down' $middle ($bounds.Top + 8 * $scale) -120 $true
        Set-WheelOffset 100
        Send-PageWheel 'bottom-boundary' $middle ($bounds.Bottom - 12 * $scale) -120 $false
        Send-PageWheel 'bottom-padding-up' $middle ($bounds.Bottom - 12 * $scale) 120 $true

        # 展开的下拉列表自己接收滚轮，外层页面不能同时跟着滚动。
        Set-WheelOffset 0
        $name = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '切换中英文')
        $type = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ComboBox)
        $combo = $content.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
            (New-Object System.Windows.Automation.AndCondition($name, $type)))
        if (-not $combo) { throw 'Wheel fixture language switch ComboBox is missing' }
        $comboPattern = $combo.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        $comboPattern.Expand()
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        $item = $null
        do {
            $items = $combo.FindAll([System.Windows.Automation.TreeScope]::Descendants,
                (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)))
            foreach ($candidate in $items) {
                if (-not $candidate.Current.IsOffscreen -and $candidate.Current.BoundingRectangle.Height -gt 0) { $item = $candidate; break }
            }
            if (-not $item) { Start-Sleep -Milliseconds 100 }
        } while (-not $item -and [DateTime]::UtcNow -lt $deadline)
        if (-not $item) { throw 'Wheel fixture ComboBox popup is not visible' }
        $itemBounds = $item.Current.BoundingRectangle
        Send-CheckedWheel 'combo-popup' ($itemBounds.Left + $itemBounds.Width / 2) ($itemBounds.Top + $itemBounds.Height / 2) -120 $false
        if ($comboPattern.Current.ExpandCollapseState -ne [System.Windows.Automation.ExpandCollapseState]::Expanded) {
            throw 'Wheel unexpectedly closed the ComboBox popup'
        }
    } finally {
        try {
            if ($comboPattern -and $comboPattern.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) {
                $comboPattern.Collapse()
            }
            if ($pattern -and $pattern.Current.VerticallyScrollable) { Set-WheelOffset 0 }
            [IO.File]::WriteAllText((Join-Path $outputPath "$label-wheel.json"), ($results | ConvertTo-Json -Depth 4))
        } finally {
            [SettingsWheelHitProbe]::SetCursorPos($cursor.X, $cursor.Y) | Out-Null
            if (-not [SettingsWindowProbe]::SetWindowPos($handle, [IntPtr]::Zero, [int]$original.X, [int]$original.Y,
                    [int]$original.Width, [int]$original.Height, 0x0040)) { throw 'Wheel probe could not restore the window size' }
        }
    }
}
