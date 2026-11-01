# 用独立合成设置运行真实 WinUI 宿主，检查全部页面并保存浅深色与窄窗口截图。
param(
    [Parameter(Mandatory = $true)][string]$Program,
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [switch]$InputOnly,
    [switch]$VoiceHotkeyOnly
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing, System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SettingsWindowProbe {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern bool SetDllDirectory(string path);
    [DllImport("msime_host_api.dll", CallingConvention = CallingConvention.Cdecl)] static extern IntPtr msime_client_default_preferences();
    [DllImport("msime_host_api.dll", CallingConvention = CallingConvention.Cdecl)] static extern void msime_client_string_free(IntPtr value);
    public static string DefaultPreferences() {
        IntPtr value = msime_client_default_preferences();
        try {
            int length = 0;
            while (Marshal.ReadByte(value, length) != 0) length++;
            byte[] bytes = new byte[length];
            Marshal.Copy(value, bytes, 0, length);
            return System.Text.Encoding.UTF8.GetString(bytes);
        } finally { msime_client_string_free(value); }
    }
    [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MinMaxInfo {
        public Point Reserved, MaxSize, MaxPosition, MinTrackSize, MaxTrackSize;
    }
    [DllImport("user32.dll")] private static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wparam, ref MinMaxInfo info);
    public static MinMaxInfo TrackLimits(IntPtr window) {
        var info = new MinMaxInfo();
        SendMessage(window, 0x0024, IntPtr.Zero, ref info);
        return info;
    }
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] static extern IntPtr GetWindowDpiAwarenessContext(IntPtr window);
    [DllImport("user32.dll")] static extern bool AreDpiAwarenessContextsEqual(IntPtr first, IntPtr second);
    public static bool PerMonitorV2(IntPtr window) {
        return AreDpiAwarenessContextsEqual(GetWindowDpiAwarenessContext(window), new IntPtr(-4));
    }
    [DllImport("user32.dll", EntryPoint = "SystemParametersInfoW", SetLastError = true)]
    static extern bool GetParameter(uint action, uint parameter, out uint value, uint flags);
    [DllImport("user32.dll", EntryPoint = "SystemParametersInfoW", SetLastError = true)]
    static extern bool SetParameter(uint action, uint parameter, IntPtr value, uint flags);
    public static uint WheelRouting() {
        uint value;
        if (!GetParameter(0x201C, 0, out value, 0)) throw new System.ComponentModel.Win32Exception();
        return value;
    }
    public static void WheelRouting(uint value) {
        if (!SetParameter(0x201D, 0, new IntPtr(value), 2)) throw new System.ComponentModel.Win32Exception();
        if (WheelRouting() != value) throw new InvalidOperationException("Wheel routing did not change");
    }
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [StructLayout(LayoutKind.Sequential)] private struct MouseInput {
        public int X, Y;
        public uint Data, Flags, Time;
        public IntPtr ExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)] private struct Input { public uint Type; public MouseInput Mouse; }
    [DllImport("user32.dll", SetLastError = true)] private static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] private static extern short GetAsyncKeyState(int key);
    [DllImport("user32.dll")] private static extern int GetSystemMetrics(int index);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] private static extern IntPtr GetAncestor(IntPtr window, uint flags);
    [DllImport("user32.dll")] private static extern IntPtr WindowFromPoint(Point point);
    private static uint GuiThread(IntPtr window) {
        uint process;
        uint thread = GetWindowThreadProcessId(window, out process);
        if (thread == 0) throw new InvalidOperationException("Synthetic settings window has no GUI thread");
        return thread;
    }
    public static IntPtr MouseTarget(IntPtr owner, int x, int y) {
        var target = WindowFromPoint(new Point { X = x, Y = y });
        if (target == IntPtr.Zero || GetAncestor(target, 2) != owner)
            throw new InvalidOperationException("Slider point does not hit the synthetic settings window");
        return target;
    }
    public static string MouseThreads(IntPtr owner, IntPtr target) {
        return "root=" + GuiThread(owner) + ", target=" + GuiThread(target) + ", hwnd=" + target.ToInt64().ToString("X");
    }
    private static void SendMouse(MouseInput mouse) {
        var input = new Input { Mouse = mouse };
        if (SendInput(1, new[] { input }, Marshal.SizeOf(typeof(Input))) != 1)
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
    public static bool LeftButtonDown() { return (GetAsyncKeyState(1) & 0x8000) != 0; }
    public static void MouseButton(bool pressed) {
        SendMouse(new MouseInput { Flags = pressed ? 0x0002u : 0x0004u });
    }
    public static void MoveMouse(int x, int y) {
        int left = GetSystemMetrics(76), top = GetSystemMetrics(77);
        int width = GetSystemMetrics(78), height = GetSystemMetrics(79);
        if (width <= 1 || height <= 1) throw new InvalidOperationException("Invalid virtual desktop bounds");
        SendMouse(new MouseInput {
            X = (int)Math.Round((x - left) * 65535.0 / (width - 1)),
            Y = (int)Math.Round((y - top) * 65535.0 / (height - 1)),
            Flags = 0x0001u | 0x4000u | 0x8000u
        });
    }
    [DllImport("user32.dll")] static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
    public static void Wheel(int delta) { mouse_event(0x0800, 0, 0, unchecked((uint)delta), UIntPtr.Zero); }
}
'@
. (Join-Path $PSScriptRoot 'settings_wheel.ps1')
. (Join-Path $PSScriptRoot 'settings_snapshot.ps1')
$programPath = (Resolve-Path -LiteralPath $Program).Path
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
$state = Join-Path ([IO.Path]::GetTempPath()) ('msime-settings-ui-' + [Guid]::NewGuid())
$oldState = $env:MSIME_CLIENT_STATE_DIR
$oldHostOptions = $env:MSIME_CLIENT_HOST_OPTIONS
$pages = @('typing', 'lexicon', 'shortcuts', 'expression', 'voice', 'osk', 'themes', 'candidate', 'toolbar', 'clip', 'ai', 'plugins', 'stats', 'account', 'dev', 'about')
$process = $null
# 用仓库许可明确的合成皮肤验证真实设置卡片；颜色块让缺图和旧模式都能被屏幕像素发现。
# 屏幕比要的尺寸小时，SetWindowPos 撑不到要的大小，之后的「宽窗口」断言会在不相干的地方失败（Depot 的 Windows 机型主屏是 800×600，窗口停在最小尺寸 820×620）。设完尺寸先读回来，撑不到就直接说清楚是屏幕的问题。
function Assert-WindowSize([IntPtr]$handle, [int]$width, [int]$height) {
    $bounds = [System.Windows.Automation.AutomationElement]::FromHandle($handle).Current.BoundingRectangle
    if ($bounds.Width -lt $width -or $bounds.Height -lt $height) {
        $screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
        throw "Settings window is $([int]$bounds.Width)x$([int]$bounds.Height), not ${width}x${height}: the $($screen.Width)x$($screen.Height) screen is too small for this check"
    }
}

function New-CanvasPreviewFixture {
    $skins = Join-Path $state 'skins'
    New-Item -ItemType Directory -Force $skins | Out-Null
    $source = Join-Path $PSScriptRoot '../../../../crates/client-services/tests/fixtures/candidate-skins/verify-canvas'
    Copy-Item -LiteralPath $source -Destination $skins -Recurse
    $package = Join-Path $skins 'verify-canvas'
    $manifest = Join-Path $package 'skin.toml'
    $body = [IO.File]::ReadAllText($manifest).Replace('name = "星河 · 画布"', 'name = "合成画布"').Replace('slice_dip = [16]', 'slice_dip = [4]')
    [IO.File]::WriteAllText($manifest, $body, (New-Object Text.UTF8Encoding($false)))
    foreach ($mode in @('light', 'dark')) {
        $colors = if ($mode -eq 'light') { @(0xff0000, 0x00ff00, 0x0000ff, 0xffff00, 0x00ffff, 0xff00ff, 0x804000, 0x408000, 0x400080) }
                  else { @(0x123456, 0x234567, 0x345678, 0x456789, 0x56789a, 0x6789ab, 0x789abc, 0x89abcd, 0x9abcde) }
        $bitmap = New-Object Drawing.Bitmap(12, 12)
        try {
            for ($y = 0; $y -lt 12; $y++) {
                for ($x = 0; $x -lt 12; $x++) {
                    $rgb = $colors[[int][Math]::Floor($y / 4) * 3 + [int][Math]::Floor($x / 4)]
                    $bitmap.SetPixel($x, $y, [Drawing.Color]::FromArgb(255, ($rgb -shr 16) -band 255, ($rgb -shr 8) -band 255, $rgb -band 255))
                }
            }
            $bitmap.Save((Join-Path $package "assets/panel-$mode.png"), [Drawing.Imaging.ImageFormat]::Png)
        } finally { $bitmap.Dispose() }
    }
}
function Get-CanvasPreviewScrollDirection($bounds, $viewport, $screen) {
    if ($bounds.Height -le 0) { return 1 }
    # UIA 返回的矩形可能已经被正文裁剪；留出余量才能识别贴边的残片。
    $top = [Math]::Max($viewport.Top, $screen.Top)
    $bottom = [Math]::Min($viewport.Bottom, $screen.Bottom)
    if ($bounds.Top -le $top + 8) { return -1 }
    if ($bounds.Bottom -ge $bottom - 8 -or $bounds.Height -lt 15) { return 1 }
    return 0
}
function Assert-CanvasPreview($window, [string]$mode) {
    Open-Page $window 'themes'
    $search = Find-ById $window 'settings-search'
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
    $input = $search.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
    if (-not $input -and $search.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) { $input = $search }
    if (-not $input) { throw 'Settings search input is missing' }
    # 主题页照 macOS 是一次一张卡片的轮播，合成皮肤的卡片不在打开页面时显示的那张上：经搜索结果打开它，轮播停在那张。皮肤目录扫描完以前索引里还没有它，清空再输入，直到结果列表里出现。
    $hit = $null
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $input.SetFocus()
        $value = $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $value.SetValue('')
        $value.SetValue('合成画布')
        Start-Sleep -Milliseconds 300
        $results = Find-ById $window 'settings-search-results'
        $hit = if ($results) { Find-ByName $results '合成画布' } else { $null }
    } while (-not $hit -and [DateTime]::UtcNow -lt $deadline)
    if (-not $hit) { throw 'Canvas skin did not reach the settings search' }
    $input.SetFocus()
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    foreach ($layout in @('horizontal', 'vertical')) {
        $expected = if ($mode -eq 'light') { 0x00ffff } else { 0x56789a }
        $ready = $false
        $lastObservation = '元素不存在'
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        do {
            $image = Find-ById $window "canvas-preview-verify-canvas-$layout"
            if ($image) { $lastObservation = "离屏=$($image.Current.IsOffscreen)，矩形=$($image.Current.BoundingRectangle)" }
            # 搜索只定位卡片，短窗口里预览仍可能在视口下方；先滚到图片完整可见，再读取真实屏幕像素。
            $content = Find-ById $window 'settings-content'
            [object]$scroll = $null
            if ($image -and $content -and $content.TryGetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern, [ref]$scroll) -and $scroll.Current.VerticallyScrollable) {
                $bounds = $image.Current.BoundingRectangle
                $viewport = $content.Current.BoundingRectangle
                $screen = [System.Windows.Forms.Screen]::FromHandle([IntPtr]$window.Current.NativeWindowHandle).WorkingArea
                $visibleTop = [Math]::Max($viewport.Top, $screen.Top)
                $visibleBottom = [Math]::Min($viewport.Bottom, $screen.Bottom)
                $lastObservation += "，正文=$viewport，可采样纵向=$visibleTop..$visibleBottom，滚动=$($scroll.Current.VerticalScrollPercent)"
                $direction = Get-CanvasPreviewScrollDirection $bounds $viewport $screen
                if ($direction -ne 0) {
                    $direction = if ($direction -lt 0) { [System.Windows.Automation.ScrollAmount]::SmallDecrement } else { [System.Windows.Automation.ScrollAmount]::SmallIncrement }
                    $scroll.Scroll([System.Windows.Automation.ScrollAmount]::NoAmount, $direction)
                    Start-Sleep -Milliseconds 200
                    continue
                }
            }
            if ($image -and -not $image.Current.IsOffscreen) {
                $bounds = $image.Current.BoundingRectangle
                if ($bounds.Width -ge 40 -and $bounds.Height -ge 15) {
                    $bitmap = New-Object Drawing.Bitmap([int]$bounds.Width, [int]$bounds.Height)
                    $graphics = [Drawing.Graphics]::FromImage($bitmap)
                    try {
                        $graphics.CopyFromScreen([int]$bounds.X, [int]$bounds.Y, 0, 0, $bitmap.Size)
                        $count = 0
                        for ($y = 0; $y -lt $bitmap.Height; $y++) {
                            for ($x = 0; $x -lt $bitmap.Width; $x++) {
                                if (($bitmap.GetPixel($x, $y).ToArgb() -band 0xffffff) -eq $expected) { $count++ }
                            }
                        }
                        $lastObservation += "，目标像素数=$count"
                        if ($count -ge 8) {
                            $bitmap.Save((Join-Path $outputPath "$mode-canvas-$layout.png"), [Drawing.Imaging.ImageFormat]::Png)
                            $ready = $true
                        }
                    } finally {
                        $graphics.Dispose()
                        $bitmap.Dispose()
                    }
                }
            }
            if (-not $ready) { Start-Sleep -Milliseconds 200 }
        } while (-not $ready -and [DateTime]::UtcNow -lt $deadline)
        if (-not $ready) {
            Save-Window $window "$mode-canvas-$layout-failed.png"
            throw "Canvas $mode $layout panel pixels are absent or from the wrong mode：$lastObservation"
        }
    }
    Save-Window $window "$mode-canvas-settings.png"
    $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('')
    Write-Host "$mode canvas native card pixels passed in both layouts"
}
function Find-ById($parent, [string]$id) {
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $id)
    return $parent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Assert-NativeInput($window, [IntPtr]$handle) {
    if (-not [SettingsWindowProbe]::PerMonitorV2($handle)) { throw 'Settings window is not PerMonitorV2 DPI aware' }
    # 固定为最小窗口与内容较长的工具栏页，避免默认输入页的短标签页没有滚动范围。
    if (-not [SettingsWindowProbe]::SetWindowPos($handle, [IntPtr]::Zero, 0, 0, 800, 560, 0x0040)) { throw 'Wheel fixture resize failed' }
    Open-Page $window 'toolbar'
    $routing = [SettingsWindowProbe]::WheelRouting()
    $cursor = New-Object SettingsWindowProbe+Point
    if (-not [SettingsWindowProbe]::GetCursorPos([ref]$cursor)) { throw 'Could not save cursor position' }
    try {
        foreach ($mode in @(0, 2)) {
            # 同时覆盖关闭和开启「悬停时滚动非活动窗口」；不持久化系统设置，退出时恢复。
            [SettingsWindowProbe]::WheelRouting($mode)
            [SettingsWindowProbe]::SetForegroundWindow($handle) | Out-Null
            # 导航和窗口缩放都会异步提交布局；每次重取 UIA 节点，直到真实正文出现滚动范围。
            $scroll = $null
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                $content = Find-ById $window 'settings-content'
                if ($content) {
                    try {
                        [object]$pattern = $null
                        if ($content.TryGetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern, [ref]$pattern) -and $pattern.Current.VerticallyScrollable) {
                            $scroll = $pattern
                            break
                        }
                    } catch [System.Windows.Automation.ElementNotAvailableException] {
                        # 页面重建时重新查询，不能把失效节点当成已完成布局。
                    }
                }
                Start-Sleep -Milliseconds 100
            } while ([DateTime]::UtcNow -lt $deadline)
            if (-not $scroll) {
                Save-Window $window "wheel-fixture-routing-$mode.png"
                throw 'Wheel fixture page does not overflow after layout'
            }
            $scroll.SetScrollPercent(-1, 0)
            Start-Sleep -Milliseconds 250
            $bounds = $content.Current.BoundingRectangle
            if (-not [SettingsWindowProbe]::SetCursorPos([int]$bounds.X + 10, [int]($bounds.Y + $bounds.Height / 2))) { throw 'Could not position wheel over the content margin' }
            if ([SettingsWindowProbe]::GetForegroundWindow() -ne $handle) { throw 'Settings window could not acquire foreground for wheel input' }
            $before = $scroll.Current.VerticalScrollPercent
            [SettingsWindowProbe]::Wheel(-360)
            $deadline = [DateTime]::UtcNow.AddSeconds(5)
            do {
                Start-Sleep -Milliseconds 100
                $after = $scroll.Current.VerticalScrollPercent
            } while ($after -le $before -and [DateTime]::UtcNow -lt $deadline)
            if ($after -le $before) { throw "Mouse wheel did not scroll the native page (routing=$mode)" }
            Write-Host "Native wheel routing=$mode moved from $before to $after"
        }
    } finally {
        [SettingsWindowProbe]::WheelRouting($routing)
        [SettingsWindowProbe]::SetCursorPos($cursor.X, $cursor.Y) | Out-Null
    }
}
function Find-ByName($parent, [string]$name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
    return $parent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
# 设置项的解释性说明收在标题旁的「?」里（main.cpp 的 info_tip）：按钮名称是「<标题>说明」，说明本身是它的 HelpText，悬停或聚焦时才以 ToolTip 显示，平时不在界面上。
function Find-InfoTip($parent, [string]$title, [string]$text) {
    $nameCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, "$($title)说明")
    $helpCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::HelpTextProperty, $text)
    $condition = New-Object System.Windows.Automation.AndCondition($nameCondition, $helpCondition)
    return $parent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Find-CheckBox($parent, [string]$name) {
    $nameCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
    $typeCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::CheckBox)
    $condition = New-Object System.Windows.Automation.AndCondition($nameCondition, $typeCondition)
    return $parent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Assert-SelectedAccent($element) {
    if ($element.Current.IsOffscreen) { throw 'Selected control is outside the screenshot' }
    $bounds = $element.Current.BoundingRectangle
    # 只检查控件左侧的选中标记，侧栏或正文里的品牌绿不能替它通过。
    $bitmap = New-Object Drawing.Bitmap([int][Math]::Min(32, $bounds.Width), [int]$bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen([int]$bounds.X, [int]$bounds.Y, 0, 0, $bitmap.Size)
        $green = 0
        for ($x = 0; $x -lt $bitmap.Width; $x++) {
            for ($y = 0; $y -lt $bitmap.Height; $y++) {
                $color = $bitmap.GetPixel($x, $y)
                if ($color.G -gt 80 -and $color.G -gt $color.R * 1.3 -and $color.B -lt $color.G * 0.9) { $green++ }
            }
        }
        if ($green -lt 8) { throw 'Selected control still uses the system accent instead of brand green' }
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}
# 页首名称最多等多久。`scripts/test-windows-settings-page-readiness.py` 只加载下面两个函数来测它们，把它调短：那里有一项要等满超时，按 15 秒等，加上 pwsh 冷启动和 Add-Type，在并行跑检查的 CI 机上会超过外层的 40 秒。
$script:pageIntroNameSeconds = 15
function Wait-PageIntroName($window, [string]$page) {
    # 节点先出现、名称稍后才可读；每轮重新查询，返回当次名称，避免断言再读到重建中的节点。
    $deadline = [DateTime]::UtcNow.AddSeconds($script:pageIntroNameSeconds)
    do {
        try {
            $content = Find-ById $window 'settings-content'
            if ($content) {
                if (Find-ById $content 'settings-load-error') { throw 'Synthetic preferences were rejected by the real host' }
                $intro = Find-ById $content "settings-page-$page"
                if ($intro) {
                    $name = $intro.Current.Name
                    if (-not [string]::IsNullOrWhiteSpace($name)) { return $name }
                }
            }
        } catch [System.Windows.Automation.ElementNotAvailableException] {
            # 页面异步重建会让刚查到的节点失效，下一轮重新查询。
        }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Page did not expose a name: $page"
}
function Open-Page($window, [string]$page) {
    # 下拉框等弹出层刚关闭时，UIA 树要稍晚才把侧栏交回来；等待导航项而非固定延时。
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $item = Find-ById $window $page
        if ($item) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $item) { throw "Missing native page: $page" }
    $item.GetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern).ScrollIntoView()
    # 滚动会重建虚拟化的导航节点；重新查到且可选中才继续，不能立即解引用暂时消失的节点。
    $selected = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $item = Find-ById $window $page
        if ($item) {
            try {
                [object]$selection = $null
                if ($item.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$selection)) {
                    $selection.Select()
                    $selected = $true
                }
            } catch [System.Windows.Automation.ElementNotAvailableException] {
                # 查找与选择之间节点也可能失效，下一次轮询重新取得。
            }
        }
        if (-not $selected) { Start-Sleep -Milliseconds 200 }
    } while (-not $selected -and [DateTime]::UtcNow -lt $deadline)
    if (-not $selected) { throw "Native page could not be selected after scrolling: $page" }
    Wait-PageIntroName $window $page | Out-Null
}
function Wait-WubiControl($window, [string]$id) {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $content = Find-ById $window 'settings-content'
        $control = if ($content) { Find-ById $content $id } else { $null }
        if ($control -and $control.Current.IsEnabled) { return $control }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing enabled Wubi control: $id"
}
function Assert-WubiCandidateSelection($window) {
    $toggle = Wait-WubiControl $window 'wubi-candidate-selection'
    $pattern = $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    if ($pattern.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::Off) { throw 'Wubi shortcuts must default to off' }
    $pattern.Toggle()
    $second = Wait-WubiControl $window 'wubi-second-candidate-key'
    $third = Wait-WubiControl $window 'wubi-third-candidate-key'
    $secondValue = $second.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $thirdValue = $third.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    if ($secondValue.Current.Value -ne ';' -or $thirdValue.Current.Value -ne "'") { throw 'Wubi shortcut defaults are incorrect' }
    $path = Join-Path $state 'preferences.json'
    $beforeEdit = (Read-SettingsSnapshotText $path)
    # 交换键位的中间状态会重复；只有按保存时才能成对进入共享偏好库。
    $secondValue.SetValue("'")
    $thirdValue.SetValue(';')
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'Editing Wubi shortcuts saved an intermediate pair' }
    $save = Wait-WubiControl $window 'wubi-save-candidate-keys'
    $save.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $keys = $saved.preferences.wubi_candidate_selection
    } while (($keys.second_key -ne "'" -or $keys.third_key -ne ';') -and [DateTime]::UtcNow -lt $deadline)
    if (-not $keys.enabled -or $keys.second_key -ne "'" -or $keys.third_key -ne ';') { throw 'Wubi shortcut pair was not saved together' }
    Open-Page $window 'expression'
    Open-Page $window 'typing'
    $second = Wait-WubiControl $window 'wubi-second-candidate-key'
    $third = Wait-WubiControl $window 'wubi-third-candidate-key'
    if ($second.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne "'" -or
        $third.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne ';') { throw 'Wubi shortcut controls did not reload saved keys' }
    $toggle = Wait-WubiControl $window 'wubi-candidate-selection'
    $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $keys = $saved.preferences.wubi_candidate_selection
    } while ($keys.enabled -and [DateTime]::UtcNow -lt $deadline)
    if ($keys.enabled -or $keys.second_key -ne "'" -or $keys.third_key -ne ';') { throw 'Disabling Wubi shortcuts lost custom keys' }
    Write-Host 'Wubi shortcut toggle, atomic swap, reload and retained keys passed'
}
function Assert-CommandLineCandidates($window) {
    # 「命令行查询候选」（#9663）缺省关闭；经实际开关打开再关上，每次只改 diagnostic_log.command_line_candidates，同组的两个日志开关保持合成偏好里的值。
    $content = Find-ById $window 'settings-content'
    $name = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '命令行查询候选')
    $toggleable = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::IsTogglePatternAvailableProperty, $true)
    $toggle = $content.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.AndCondition($name, $toggleable)))
    if (-not $toggle -or -not $toggle.Current.IsEnabled) { throw 'Maintenance page is missing the 命令行查询候选 switch' }
    $pattern = $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    if ($pattern.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::Off) { throw '命令行查询候选 must default to off' }
    $path = Join-Path $state 'preferences.json'
    $before = ((Read-SettingsSnapshotText $path) | ConvertFrom-Json).preferences.diagnostic_log
    if ($before.command_line_candidates -ne $false) { throw 'Synthetic preferences do not start with command_line_candidates off' }
    foreach ($expected in @($true, $false)) {
        $pattern.Toggle()
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $diagnostics = ((Read-SettingsSnapshotText $path) | ConvertFrom-Json).preferences.diagnostic_log
        } while ($diagnostics.command_line_candidates -ne $expected -and [DateTime]::UtcNow -lt $deadline)
        if ($diagnostics.command_line_candidates -ne $expected) { throw "命令行查询候选 was not saved as $expected" }
        if ($diagnostics.server -ne $before.server -or $diagnostics.tsf -ne $before.tsf) { throw 'Saving 命令行查询候选 changed the diagnostic log switches' }
    }
    Write-Host '命令行查询候选 default, save and retained diagnostic log switches passed'
}
function Assert-PinyinAutocorrect($window, [string]$scheme) {
    # 「高级选项」的全拼纠错 / 双拼纠错组（#10210）照共享页：只在全拼、双拼方案下出现，标题和说明按方案写，两个开关都写 quanpin 下的字段、默认开。全拼、双拼方案下都经实际开关各关一次再打开，每次只改自己的字段，quanpin 下其他字段保持原值；关着时整页重画一次，开关要读回关；其他方案下这一组不画。
    $titles = @{
        quanpin = @('全拼纠错', '全拼邻键纠错', '全拼乱序纠错')
        shuangpin = @('双拼纠错', '双拼邻键纠错', '双拼乱序纠错')
    }
    $descriptions = @{
        quanpin = '把前后颠倒的两个字母纠回来，例如 hsang → shang、gau → gua。关闭后不再按乱序猜测拼写。两个开关都关闭时，漏打、多打字母的纠错（shng → shang）也一起关闭。'
        shuangpin = '把两键编码前后颠倒的音节纠回来，同样只用于三个音节以上的整句。关闭后不再按乱序猜测编码。'
    }
    Select-PageTab $window 'input-tab-advanced' '启用模糊音'
    $content = Find-ById $window 'settings-content'
    foreach ($family in @('quanpin', 'shuangpin')) {
        foreach ($name in $titles[$family]) {
            $shown = [bool](Find-ByName $content $name)
            if ($shown -ne ($family -eq $scheme)) { throw "Autocorrect row $name shown=$shown under scheme $scheme" }
        }
    }
    if ($titles.ContainsKey($scheme) -and -not (Find-InfoTip $content $titles[$scheme][2] $descriptions[$scheme])) { throw "Autocorrect transposition description does not follow the shared page under $scheme" }
    if ($titles.ContainsKey($scheme)) {
        $path = Join-Path $state 'preferences.json'
        $toggleable = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::IsTogglePatternAvailableProperty, $true)
        foreach ($row in @(@($titles[$scheme][1], 'autocorrect_neighbor'), @($titles[$scheme][2], 'autocorrect_transposition'))) {
            $name = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $row[0])
            $condition = New-Object System.Windows.Automation.AndCondition($name, $toggleable)
            $toggle = (Find-ById $window 'settings-content').FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
            if (-not $toggle -or -not $toggle.Current.IsEnabled) { throw "Advanced tab is missing the $($row[0]) switch" }
            $pattern = $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
            if ($pattern.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::On) { throw "$($row[0]) must default to on" }
            $before = ((Read-SettingsSnapshotText $path) | ConvertFrom-Json).preferences.quanpin
            if ($before.($row[1]) -ne $true) { throw "Synthetic preferences do not start with quanpin.$($row[1]) on" }
            foreach ($expected in @($false, $true)) {
                $pattern.Toggle()
                $deadline = [DateTime]::UtcNow.AddSeconds(10)
                do {
                    Start-Sleep -Milliseconds 200
                    $quanpin = ((Read-SettingsSnapshotText $path) | ConvertFrom-Json).preferences.quanpin
                } while ($quanpin.($row[1]) -ne $expected -and [DateTime]::UtcNow -lt $deadline)
                if ($quanpin.($row[1]) -ne $expected) { throw "$($row[0]) was not saved as quanpin.$($row[1]) = $expected" }
                foreach ($field in $before.PSObject.Properties) {
                    if ($field.Name -ne $row[1] -and $quanpin.($field.Name) -ne $field.Value) { throw "Saving $($row[0]) changed quanpin.$($field.Name)" }
                }
                if (-not $expected) {
                    # 存着的值与回退值（开）不同时整页重画，开关必须读回关，证明它读的是 quanpin 下这个字段而不是只显示回退值。
                    Select-PageTab $window 'input-tab-scheme' '中文方案'
                    Select-PageTab $window 'input-tab-advanced' '启用模糊音'
                    $toggle = (Find-ById $window 'settings-content').FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
                    if (-not $toggle) { throw "Advanced tab lost the $($row[0]) switch after a redraw" }
                    $pattern = $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
                    if ($pattern.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::Off) { throw "$($row[0]) does not read back quanpin.$($row[1]) = false after a redraw" }
                }
            }
        }
        Write-Host "$scheme autocorrect switches default on, read back and save their own quanpin fields and keep the rest"
    }
    Select-PageTab $window 'input-tab-scheme' '中文方案'
}
function Find-CandidatePageSizeSlider($window) {
    $nameCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '每页候选项数量')
    $typeCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Slider)
    $condition = New-Object System.Windows.Automation.AndCondition($nameCondition, $typeCondition)
    return $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Show-CandidatePageSizeSlider($window) {
    $content = Find-ById $window 'settings-content'
    $scroll = $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
    $seekFromTop = $false
    $lastBounds = $null
    $lastPercent = -1.0
    $stable = 0
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $slider = Find-CandidatePageSizeSlider $window
        if (-not $slider) { throw '候选数量滑块缺失' }
        $bounds = $slider.Current.BoundingRectangle
        $viewport = $content.Current.BoundingRectangle
        # 留出预览高度变化的空间，避免拖动后这一行再次被挤出视口。
        $visible = -not $slider.Current.IsOffscreen -and $bounds.Height -gt 0 -and
            $bounds.Top -ge $viewport.Top -and $bounds.Bottom -le $viewport.Bottom
        if ($visible -and (-not $scroll.Current.VerticallyScrollable -or
            $bounds.Bottom -le $viewport.Top + $viewport.Height / 2 -or
            $scroll.Current.VerticalScrollPercent -ge 100)) {
            # Scroll 返回时滚动动画仍可能在移动；坐标与偏移连续稳定后才能按下滑块柄。
            $percent = $scroll.Current.VerticalScrollPercent
            if ($null -ne $lastBounds -and $bounds.Equals($lastBounds) -and
                [Math]::Abs($percent - $lastPercent) -lt 0.001) { $stable++ } else { $stable = 0 }
            if ($stable -ge 3) { return $slider }
            $lastBounds = $bounds
            $lastPercent = $percent
        } else {
            $stable = 0
            $lastBounds = $null
            if (-not $scroll.Current.VerticallyScrollable) { throw '候选数量滑块不可见且正文无法滚动' }
            # 离屏元素可能没有可用矩形，从顶部向下找，不能由空矩形推断滚动方向。
            if (-not $seekFromTop) {
                $scroll.SetScrollPercent(-1, 0)
                $seekFromTop = $true
            } else {
                $scroll.Scroll([System.Windows.Automation.ScrollAmount]::NoAmount, [System.Windows.Automation.ScrollAmount]::SmallIncrement)
            }
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw '候选数量滑块未滚动到可视区域'
}
function Find-SliderValue($window, $slider) {
    $bounds = $slider.Current.BoundingRectangle
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
    $labels = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    $found = $null
    foreach ($label in $labels) {
        if ($label.Current.IsOffscreen -or $label.Current.Name -notmatch '^\d+$') { continue }
        $rect = $label.Current.BoundingRectangle
        if ($rect.Left -ge $bounds.Right -and $rect.Left -lt $bounds.Right + 100 -and
            $rect.Top -lt $bounds.Bottom -and $rect.Bottom -gt $bounds.Top) {
            if ($found) { throw '滑块右侧存在多个数值标签，无法唯一定位' }
            $found = $label
        }
    }
    return $found
}
function Assert-CandidatePageSizeLabel($window, $slider, [int]$expected) {
    $range = $slider.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern)
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    while ($range.Current.Value -ne $expected -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
    if ($range.Current.Value -ne $expected) { throw "候选数量滑块未变为 $expected，实际为 $($range.Current.Value)" }
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $bounds = $slider.Current.BoundingRectangle
        $viewport = (Find-ById $window 'settings-content').Current.BoundingRectangle
        if ($slider.Current.IsOffscreen -or $bounds.Height -le 0 -or
            $bounds.Top -lt $viewport.Top -or $bounds.Bottom -gt $viewport.Bottom) { throw '数值变化后候选数量滑块离开可视区域' }
        $label = Find-SliderValue $window $slider
        if ($label -and -not $label.Current.IsOffscreen -and $label.Current.Name -eq [string]$expected) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $label -or $label.Current.IsOffscreen -or $label.Current.Name -ne [string]$expected) { throw "候选数量右侧标签未更新为 $expected" }
    if ($slider.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern).Current.Value -ne $expected) { throw '候选数量标签与滑块数值不一致' }
}
function Wait-CandidatePageSizeSaved([int]$expected) {
    $path = Join-Path $state 'preferences.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $saved = $null
    do {
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        if ($saved.preferences.candidate_page_size -eq $expected) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "候选数量未保存：期望 $expected，文件里是 $($saved.preferences.candidate_page_size)，revision=$($saved.revision)"
}
function Find-SliderThumb($slider) {
    # WinUI 默认模板把滑块柄放在 Raw view，只遍历当前滑块，不能扫描右侧标签。
    $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $pending = New-Object 'System.Collections.Generic.Stack[System.Windows.Automation.AutomationElement]'
    $pending.Push($slider)
    $found = $null
    while ($pending.Count -gt 0) {
        $node = $pending.Pop()
        if ($node.Current.ControlType -eq [System.Windows.Automation.ControlType]::Thumb -and -not $node.Current.IsOffscreen) {
            if ($found) { throw '滑块内存在多个可见滑块柄' }
            $found = $node
        }
        $child = $walker.GetFirstChild($node)
        while ($child) {
            $pending.Push($child)
            $child = $walker.GetNextSibling($child)
        }
    }
    return $found
}
function Assert-CandidatePageSizeDrag($window, [string]$screenshotPrefix) {
    $handle = [IntPtr]$window.Current.NativeWindowHandle
    [SettingsWindowProbe]::SetForegroundWindow($handle) | Out-Null
    if ([SettingsWindowProbe]::GetForegroundWindow() -ne $handle) { throw '拖动检查要求合成设置窗口在前台' }
    $cursor = New-Object SettingsWindowProbe+Point
    if (-not [SettingsWindowProbe]::GetCursorPos([ref]$cursor)) { throw '拖动检查需要交互桌面' }
    try {
        foreach ($expected in @(9, 3)) {
            $slider = Show-CandidatePageSizeSlider $window
            if (-not $slider -or $slider.Current.IsOffscreen -or -not $slider.Current.IsEnabled) { throw '候选数量滑块不可拖动' }
            $thumb = Find-SliderThumb $slider
            if (-not $thumb -or $thumb.Current.IsOffscreen) { throw '候选数量滑块缺少可见滑块柄' }
            $bounds = $slider.Current.BoundingRectangle
            $grip = $thumb.Current.BoundingRectangle
            $startX = [int]($grip.Left + $grip.Width / 2)
            $y = [int]($grip.Top + $grip.Height / 2)
            $endX = if ($expected -eq 9) { [int]($bounds.Right - 1) } else { [int]($bounds.Left + 1) }
            Write-Host "候选数量拖动起点=($startX,$y)，终点=($endX,$y)，滑块=$bounds，滑块柄=$grip"
            if (-not [SettingsWindowProbe]::SetCursorPos($startX, $y)) { throw '无法将鼠标放到滑块柄上' }
            $inputTarget = [SettingsWindowProbe]::MouseTarget($handle, $startX, $y)
            Write-Host "候选数量鼠标命中线程：$([SettingsWindowProbe]::MouseThreads($handle, $inputTarget))"
            try {
                if ([SettingsWindowProbe]::LeftButtonDown()) { throw '拖动前鼠标左键已按下' }
                [SettingsWindowProbe]::MouseButton($true)
                # WinUI 的指针捕获不保证出现在 HWND 捕获字段；按下与移动统一进 SendInput 队列，保持输入顺序。
                $deadline = [DateTime]::UtcNow.AddSeconds(5)
                while (-not [SettingsWindowProbe]::LeftButtonDown() -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
                if (-not [SettingsWindowProbe]::LeftButtonDown()) { throw '鼠标左键按下未进入系统输入状态' }
                # 分步输入形成真实拖动；读取标签前先完成第一次数值变化。
                for ($step = 1; $step -le 8; $step++) {
                    $x = [int]($startX + ($endX - $startX) * $step / 8)
                    [SettingsWindowProbe]::MoveMouse($x, $y)
                    Start-Sleep -Milliseconds 25
                }
                if (-not [SettingsWindowProbe]::LeftButtonDown()) { throw '读取拖动数值前鼠标左键已松开' }
                Assert-CandidatePageSizeLabel $window $slider $expected
                if ($expected -eq 9) { Save-Window $window "$screenshotPrefix-candidate-page-size-drag-held-9.png" }
            } finally {
                [SettingsWindowProbe]::MouseButton($false)
            }
            Wait-CandidatePageSizeSaved $expected
            Assert-CandidatePageSizeLabel $window $slider $expected
        }
    } finally {
        [SettingsWindowProbe]::SetCursorPos($cursor.X, $cursor.Y) | Out-Null
    }
    Write-Host '候选数量真实鼠标拖动：按住、松开及保存验证通过'
}
function Assert-CandidatePageSize($window, [string]$screenshotPrefix) {
    foreach ($expected in @(3, 9, 4, 6)) {
        $slider = Show-CandidatePageSizeSlider $window
        if (-not $slider -or -not $slider.Current.IsEnabled) { throw '候选数量滑块不可用' }
        $savedBefore = (Read-SettingsSnapshotText (Join-Path $state 'preferences.json')) | ConvertFrom-Json
        Write-Host "候选数量 $screenshotPrefix 操作前：滑块=$($slider.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern).Current.Value)，文件=$($savedBefore.preferences.candidate_page_size)，revision=$($savedBefore.revision)，目标=$expected"
        $slider.SetFocus()
        $range = $slider.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern)
        $range.SetValue($expected)
        # 检查同一个可见数值标签，不能用滑块 tooltip 或保存成功替代界面更新。
        Assert-CandidatePageSizeLabel $window $slider $expected
        Wait-CandidatePageSizeSaved $expected
        if ($expected -eq 9) { Save-Window $window "$screenshotPrefix-candidate-page-size-9.png" }
    }
    Write-Host '候选数量 UIA 数值变更、可见标签与保存验证通过，开始真实鼠标拖动'
    Assert-CandidatePageSizeDrag $window $screenshotPrefix
    # 拖动最后停在 3；恢复成 6 后再检查切页，保持后续合成页面的基线一致。
    $slider = Show-CandidatePageSizeSlider $window
    $slider.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern).SetValue(6)
    Assert-CandidatePageSizeLabel $window $slider 6
    Wait-CandidatePageSizeSaved 6
    Open-Page $window 'toolbar'
    Open-Page $window 'candidate'
    Select-PageTab $window 'candidate-tab-layout' '候选项排列方式'
    $slider = Show-CandidatePageSizeSlider $window
    if (-not $slider) { throw '切页后候选数量滑块缺失' }
    $slider.SetFocus()
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        $label = Find-SliderValue $window $slider
        if ($label) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $slider -or -not $label -or $label.Current.Name -ne '6' -or
        $slider.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern).Current.Value -ne 6) { throw '切页后候选数量未恢复已保存值' }
    Write-Host '候选数量滑块、右侧数值、保存及切页恢复验证通过'
}
function Select-ComboChoice($window, [string]$id, [string]$label) {
    $combo = Find-ById $window $id
    if (-not $combo) { throw "Missing combo: $id" }
    $combo.SetFocus()
    $combo.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $item = Find-ByName $combo $label
    } while (-not $item -and [DateTime]::UtcNow -lt $deadline)
    if (-not $item) { throw "Missing combo option: $label" }
    $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
    $combo.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Collapse()
}
# 仅操作独立合成窗口，验证录键经过原生事件、共享预检及真实偏好保存。
function Assert-VoiceToggleRecorder($window, [IntPtr]$handle) {
    function Read-VoiceBinding {
        $saved = (Read-SettingsSnapshotText (Join-Path $state 'preferences.json')) | ConvertFrom-Json
        $field = $saved.preferences.keybindings.PSObject.Properties['toggle_voice_input']
        if ($field) { return [string]$field.Value }
        return ''
    }
    function Wait-VoiceBinding([string]$expected) {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            if ((Read-VoiceBinding) -eq $expected) { return }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "语音绑定保存值不符：期望 $expected，实际 $(Read-VoiceBinding)"
    }
    function Wait-VoiceRecorder([string]$expected = '') {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $button = Find-ById $window 'voice-toggle-recorder'
            if ($button -and -not $button.Current.IsOffscreen -and $button.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and $button.Current.IsEnabled -and
                (-not $expected -or $button.Current.Name -eq "语音输入开始／结束：$expected")) { return $button }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        $actual = if ($button) { "Name=$($button.Current.Name), Offscreen=$($button.Current.IsOffscreen), Focus=$($button.Current.HasKeyboardFocus)" } else { '控件不存在' }
        Save-Window $window 'voice-hotkey-failed.png'
        throw "语音录键控件缺失或显示值不符：期望 $expected，实际 $actual"
    }
    function Show-VoiceRecorder {
        # 滚动是一次准备动作；录键中只读状态，避免 UIA 滚动把焦点从按钮移走并取消录键。
        $content = Find-ById $window 'settings-content'
        if ($content) {
            [object]$scroll = $null
            if ($content.TryGetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern, [ref]$scroll) -and $scroll.Current.VerticallyScrollable) {
                $scroll.SetScrollPercent(-1, 100)
            }
        }
        return Wait-VoiceRecorder
    }
    function Record-VoiceKeys([string]$keys) {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            # 页面延后重画可能取消录键；这里只重试尚未送键的准备动作，不重放已投递的组合键。
            $button = Show-VoiceRecorder
            [SettingsWindowProbe]::SetForegroundWindow($handle) | Out-Null
            $button.SetFocus()
            if ([SettingsWindowProbe]::GetForegroundWindow() -ne $handle) { throw '合成设置窗口未获得前台焦点' }
            $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
            $ready = $true
            foreach ($sample in @(1, 2)) {
                Start-Sleep -Milliseconds 100
                $current = Find-ById $window 'voice-toggle-recorder'
                if (-not $current -or $current.Current.IsOffscreen -or -not $current.Current.HasKeyboardFocus -or
                    $current.Current.Name -ne '语音输入开始／结束：请按组合键…') { $ready = $false; break }
            }
            if ($ready -and [SettingsWindowProbe]::GetForegroundWindow() -eq $handle) {
                [System.Windows.Forms.SendKeys]::SendWait($keys)
                Write-Host "语音录键已投递：$keys"
                return
            }
        } while ([DateTime]::UtcNow -lt $deadline)
        throw '语音录键准备阶段未能稳定获得键盘焦点，尚未投递组合键'
    }
    Wait-VoiceBinding ''
    Open-Page $window 'voice'
    Select-PageTab $window 'voice-tab-input' '服务配置'
    Record-VoiceKeys '^%{F8}'
    Wait-VoiceBinding 'Ctrl+Alt+F8'
    Show-VoiceRecorder | Out-Null
    Wait-VoiceRecorder 'Ctrl+Alt+F8' | Out-Null
    Start-Sleep -Milliseconds 150
    Save-Window $window 'voice-hotkey-bound.png'
    Record-VoiceKeys '{ESC}'
    Wait-VoiceBinding 'Ctrl+Alt+F8'
    Record-VoiceKeys '^{F9}'
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $conflict = Find-ByName $window '与现有快捷键冲突，请换一个组合键'
        if ($conflict) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $conflict) { throw '原生录键没有显示固定语音键冲突' }
    Wait-VoiceBinding 'Ctrl+Alt+F8'
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Open-Page $window 'about'
    Open-Page $window 'voice'
    Select-PageTab $window 'voice-tab-input' '服务配置'
    Show-VoiceRecorder | Out-Null
    Wait-VoiceRecorder 'Ctrl+Alt+F8' | Out-Null
    Record-VoiceKeys '{BACKSPACE}'
    Wait-VoiceBinding ''
    Show-VoiceRecorder | Out-Null
    Wait-VoiceRecorder '未绑定' | Out-Null
    Start-Sleep -Milliseconds 150
    Save-Window $window 'voice-hotkey-cleared.png'
    Write-Host '语音原生录键、取消、固定键冲突、切页恢复与解绑通过'
}
function Assert-SentenceModelSettings($window, [string]$theme) {
    Open-Page $window 'typing'
    Select-PageTab $window 'input-tab-candidates' '语法模型'
    $models = @(
        @{value = 'wanxiang'; label = '万象 LTS（约 380 MB，CC-BY-4.0）'; pack = 'grammar-model-wanxiang'},
        @{value = 'baguwen_word'; label = '八股文·词级（约 197 MB，LGPL-3.0）'; pack = 'grammar-model-baguwen'},
        @{value = 'baguwen_compact'; label = '八股文·词级紧凑（约 39 MB，LGPL-3.0）'; pack = 'grammar-model-baguwen-compact'},
        @{value = 'none'; label = '不使用'; pack = ''}
    )
    $path = Join-Path $state 'preferences.json'
    foreach ($width in @(1000, 800)) {
        $handle = [IntPtr]$window.Current.NativeWindowHandle
        if (-not [SettingsWindowProbe]::SetWindowPos($handle, [IntPtr]::Zero, 0, 0, $width, 780, 0x0040)) { throw 'Model fixture resize failed' }
        Assert-WindowSize $handle $width 780
        foreach ($model in $models) {
            # 模型设置在页面底部，必须滚到控件可见再操作和截图。
            $content = Find-ById $window 'settings-content'
            $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).SetScrollPercent(-1, 100)
            Start-Sleep -Milliseconds 250
            $combo = Find-ById $window 'sentence-grammar-model'
            if (-not $combo -or $combo.Current.IsOffscreen) { throw 'Grammar model selector is not visible' }
            $check = Find-ById $window 'sentence-grammar-model-check-updates'
            if (-not $check) { throw 'Grammar model update check is missing' }
            $combo.SetFocus()
            $combo.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 100
                $item = Find-ByName $combo $model.label
            } while (-not $item -and [DateTime]::UtcNow -lt $deadline)
            if (-not $item) { throw "Missing grammar model option: $($model.value)" }
            # 选择后页面会重建，不再调用旧下拉框的 Collapse。
            $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
            $ready = $false
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 150
                try {
                    $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
                    $property = $saved.preferences.sentence_association.PSObject.Properties['grammar_model']
                    $value = if ($property) { $property.Value } else { 'none' }
                    $ready = $value -eq $model.value
                    foreach ($choice in $models) {
                        if (-not $choice.pack) { continue }
                        $download = Find-ById $window "sentence-model-download-$($choice.pack)"
                        if ($choice.pack -eq $model.pack) {
                            $ready = $ready -and $null -ne $download -and $download.Current.IsEnabled
                        } else {
                            $ready = $ready -and $null -eq $download
                        }
                    }
                } catch [System.Windows.Automation.ElementNotAvailableException] {
                    $ready = $false
                }
            } while (-not $ready -and [DateTime]::UtcNow -lt $deadline)
            if (-not $ready) { throw "Grammar model selection did not save and refresh its download action: $($model.value)" }
            $content = Find-ById $window 'settings-content'
            $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).SetScrollPercent(-1, 100)
            Start-Sleep -Milliseconds 250
            Save-Window $window "$theme-$width-model-$($model.value).png"
            # 导航后重新读取持久化值；整个专项不点击下载，不访问模型服务。
            Open-Page $window 'toolbar'
            Open-Page $window 'typing'
            Select-PageTab $window 'input-tab-candidates' '语法模型'
            $combo = Find-ById $window 'sentence-grammar-model'
            $selection = @($combo.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection())
            if ($selection.Count -ne 1 -or $selection[0].Current.Name -ne $model.label) { throw 'Grammar model selection was not retained after navigation' }
        }
    }
    Write-Host "$theme grammar model choices, persisted values and immediate download actions passed"
}
function Start-SettingsWindow([string]$route) {
    # 进程先记进脚本作用域的 $process，窗口没打开时 finally 也能把它收掉。
    $script:process = Start-Process -FilePath $programPath -ArgumentList "--route=$route" -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(60)
    do {
        $script:process.Refresh()
        if ($script:process.HasExited) { throw "Settings exited before opening: $($script:process.ExitCode)" }
        if ($script:process.MainWindowHandle -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($script:process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Settings window did not open in 60 seconds' }
    return [System.Windows.Automation.AutomationElement]::FromHandle($script:process.MainWindowHandle)
}
function Wait-PluginEditorElement([scriptblock]$find, [string]$what) {
    # 编辑器的输入只就地更新提示和按钮，加减行、保存和读取会整页重画，之前取到的节点随之失效：每次重新查找，失效的当作还没出现。
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        try {
            $element = & $find
            if ($element) { return $element }
        } catch [System.Windows.Automation.ElementNotAvailableException] {
        }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "$what did not appear"
}
function Assert-PluginEditor($window, [string]$prefix) {
    # 「新建插件」先选类型，再填名字和一条短语：编码写成大写时保存按钮上方报编码的规则、不能保存，改好后「保存并安装」由宿主按导入的规则装进合成数据目录，打开这个包的详情，详情里有「编辑」；从详情再打开编辑器读回原内容，没有改动时返回不用确认，回到详情、再回到列表。
    $invoke = { param($element) $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
    $value = { param($element) $element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern) }
    & $invoke (Wait-PluginEditorElement { Find-ById $window 'plugin-create' } 'The 新建插件 button')
    Wait-PluginEditorElement { Find-ById $window 'plugin-create-wordbook' } 'The editor kind picker' | Out-Null
    foreach ($kind in @('phrase_table', 'command_table', 'symbol_set')) {
        if (-not (Find-ById $window "plugin-create-$kind")) { throw "The editor kind picker is missing $kind" }
    }
    Save-Window $window "$prefix-plugin-editor-kinds.png"
    & $invoke (Find-ById $window 'plugin-create-phrase_table')
    $name = Wait-PluginEditorElement { Find-ById $window 'plugin-editor-name' } 'The phrase table form'
    if (-not (Find-ByName $window '新建短语表')) { throw 'The editor does not title the new phrase table' }
    if ((& $value (Find-ByName $window '许可证')).Current.Value -ne 'CC0-1.0' -or (& $value (Find-ByName $window '版本')).Current.Value -ne '1.0.0') { throw 'The editor does not default to CC0-1.0 and 1.0.0' }
    (& $value $name).SetValue('合成短语')
    (& $value (Find-ByName $window '第 1 条短语的编码')).SetValue('DH')
    (& $value (Find-ByName $window '第 1 条短语的文字')).SetValue('合成电话')
    Wait-PluginEditorElement {
        $warning = Find-ById $window 'plugin-editor-warning'
        if ($warning -and $warning.Current.Name -eq '第 1 条短语的编码只能是 1 到 32 个小写字母。') { $warning }
    } 'The first problem above the save button' | Out-Null
    if ((Find-ById $window 'plugin-editor-save').Current.IsEnabled) { throw 'An invalid draft can be saved' }
    Save-Window $window "$prefix-plugin-editor-issue.png"
    (& $value (Find-ByName $window '第 1 条短语的编码')).SetValue('dh')
    $save = Wait-PluginEditorElement {
        $button = Find-ById $window 'plugin-editor-save'
        if ($button -and $button.Current.IsEnabled) { $button }
    } 'An enabled 保存并安装'
    & $invoke $save
    $edit = Wait-PluginEditorElement { Find-ById $window 'plugin-edit' } 'The saved pack detail'
    $manifests = @(Get-ChildItem -LiteralPath (Join-Path $state 'plugins\phrase_table') -Filter 'plugin.toml' -Recurse)
    if ($manifests.Count -ne 1) { throw "Expected one saved phrase table, found $($manifests.Count)" }
    $manifest = [IO.File]::ReadAllText($manifests[0].FullName, [Text.Encoding]::UTF8)
    foreach ($text in @('合成短语', 'dh', '合成电话', 'CC0-1.0')) {
        if (-not $manifest.Contains($text)) { throw "The saved plugin.toml is missing $text" }
    }
    if ($manifests[0].Directory.Name -notmatch '^my-[0-9a-f]{8}$') { throw "Unexpected generated id $($manifests[0].Directory.Name)" }
    Start-Sleep -Milliseconds 350
    Save-Window $window "$prefix-plugin-editor-saved.png"
    & $invoke $edit
    Wait-PluginEditorElement {
        $field = Find-ById $window 'plugin-editor-name'
        if ($field -and (& $value $field).Current.Value -eq '合成短语') { $field }
    } 'The edited pack read back into the form' | Out-Null
    if (-not (Find-ByName $window '编辑「合成短语」')) { throw 'The editor does not title the pack it edits' }
    if ((& $value (Find-ByName $window '第 1 条短语的文字')).Current.Value -ne '合成电话') { throw 'The editor did not read back the phrase' }
    if ((Find-ById $window 'plugin-editor-save').Current.IsEnabled) { throw 'An unchanged pack can be saved' }
    & $invoke (Find-ByName $window '返回我的插件')
    Wait-PluginEditorElement { Find-ById $window 'plugin-edit' } 'The detail after leaving the editor' | Out-Null
    & $invoke (Wait-PluginEditorElement { Find-ByName $window '返回我的插件' } 'The way back from the detail')
    Wait-PluginEditorElement { Find-ById $window 'plugin-create' } 'The plugin library after the detail' | Out-Null
    Write-Host 'Plugin editor created, saved, read back and left a phrase table'
}
function Select-PageTab($window, [string]$id, [string]$expected) {
    # 页内标签照 macOS 的 useSettingsTabs：按钮 ID 是 <前缀>-<标签 id>，选中的那个 ItemStatus 是「已选择」；切换后整页重画，等这块内容里的一个名字出现，而不是固定延时。
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $tab = Find-ById $window $id
        if ($tab -and $tab.Current.IsEnabled) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $tab) { throw "Missing page tab: $id" }
    if ($tab.Current.ItemStatus -ne '已选择') { $tab.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $tab = Find-ById $window $id
        $content = Find-ById $window 'settings-content'
        $shown = $content -and (Find-ByName $content $expected)
    } while ((-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $shown) -and [DateTime]::UtcNow -lt $deadline)
    if (-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $shown) { throw "Page tab $id did not show $expected" }
}
function Assert-PageIntro($window, [string]$page, [string]$expected) {
    # 页首引导随页内标签换成那个标签的说明（SettingsMascotIntro.h）。
    $name = Wait-PageIntroName $window $page
    if ($name -ne $expected) { throw "Page intro of $page does not follow its tab: expected '$expected'" }
}
function Wait-RelayControl($window, [string]$id) {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $control = Find-ById $window $id
        if ($control -and $control.Current.IsEnabled) { return $control }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Missing enabled cloud clipboard control: $id"
}
function Assert-CloudClipboardRelaySettings($window) {
    function Read-RelayPreference($document) {
        $property = $document.preferences.PSObject.Properties['cloud_clipboard_relay']
        if ($property) { return $property.Value }
        return $null
    }
    $path = Join-Path $state 'preferences.json'
    $beforeEdit = (Read-SettingsSnapshotText $path)
    $scroll = (Find-ById $window 'settings-content').GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
    if ($scroll.Current.VerticallyScrollable) { $scroll.SetScrollPercent(-1, 100) }
    Select-ComboChoice $window 'cloud-clipboard-source' '自建中继'
    $url = Wait-RelayControl $window 'cloud-clipboard-relay-url'
    $token = Wait-RelayControl $window 'cloud-clipboard-relay-token'
    if (-not $token.Current.IsPassword) { throw 'Relay token is not masked' }
    $url.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('http://relay.example.test')
    $token.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-relay-token')
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'Relay draft was persisted before saving' }
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-RelayControl $window 'settings-notice' | Out-Null
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'Invalid HTTP relay passed Rust preference validation' }
    # 校验失败后窗口按共享设置的冲突处理重新读取，重新选择中继再填写合成配置；只使用本机拒绝连接的端口，不发送真实数据。
    Select-ComboChoice $window 'cloud-clipboard-source' '自建中继'
    $url = Wait-RelayControl $window 'cloud-clipboard-relay-url'
    $token = Wait-RelayControl $window 'cloud-clipboard-relay-token'
    $url.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('http://127.0.0.1:9/msime')
    $token.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-relay-token')
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $relay = Read-RelayPreference $saved
    } while (($relay.url -ne 'http://127.0.0.1:9/msime' -or $relay.token -ne 'synthetic-relay-token') -and [DateTime]::UtcNow -lt $deadline)
    if ($relay.url -ne 'http://127.0.0.1:9/msime' -or $relay.token -ne 'synthetic-relay-token') { throw 'Relay address and token were not saved together' }
    Open-Page $window 'typing'
    Open-Page $window 'clip'
    $url = Wait-RelayControl $window 'cloud-clipboard-relay-url'
    if ($url.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne 'http://127.0.0.1:9/msime') { throw 'Reload lost the relay configuration' }
    $scroll = (Find-ById $window 'settings-content').GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
    if ($scroll.Current.VerticallyScrollable) { $scroll.SetScrollPercent(-1, 100) }
    Select-ComboChoice $window 'cloud-clipboard-source' '水杉账号'
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $relay = Read-RelayPreference $saved
    } while ($relay -and [DateTime]::UtcNow -lt $deadline)
    if ($relay) { throw 'Switching to account retained relay credentials' }
    Write-Host 'Cloud clipboard relay masking, draft, Rust validation, atomic save, reload and account switch passed'
}
function Assert-CloudClipboardWebDavSettings($window) {
    # WebDAV 只用合成凭据和本机拒绝连接的 9 号端口：测试连接答 relay_unavailable，不发出真实请求。
    $unavailable = '连不上网盘，请检查地址和网络；证书须由公网信任的机构签发'
    $webdavUrl = 'http://127.0.0.1:9/dav'
    $path = Join-Path $state 'preferences.json'
    function Read-CloudPreference([string]$key) {
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $property = $saved.preferences.PSObject.Properties[$key]
        if ($property) { return $property.Value }
        return $null
    }
    function Show-CloudGroup($window) {
        $scroll = (Find-ById $window 'settings-content').GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
        if ($scroll.Current.VerticallyScrollable) { $scroll.SetScrollPercent(-1, 100) }
    }
    function Wait-WebDavResult($window, [string]$expected) {
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 200
            $result = Find-ById $window 'cloud-clipboard-webdav-result'
            $name = if ($result) { $result.Current.Name } else { '' }
        } while ($name -ne $expected -and [DateTime]::UtcNow -lt $deadline)
        if ($name -ne $expected) { throw "WebDAV probe result was '$name', expected '$expected'" }
    }
    # 关掉窗口，把合成的网盘配置直接写进偏好文件，再重新打开「剪贴板」页，模拟已经测试通过并保存过的状态。
    function Restart-WithSavedWebDav {
        $script:process.CloseMainWindow() | Out-Null
        if (-not $script:process.WaitForExit(10000) -or $script:process.ExitCode -ne 0) { throw 'Settings window failed to close before seeding WebDAV' }
        $document = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $document.preferences | Add-Member -NotePropertyName cloud_clipboard_webdav -NotePropertyValue ([pscustomobject]@{url = $webdavUrl; username = 'synthetic@example.test'; password = 'synthetic-app-password'}) -Force
        $document.revision = [uint64]$document.revision + 1
        [IO.File]::WriteAllText($path, ($document | ConvertTo-Json -Depth 64), (New-Object Text.UTF8Encoding($false)))
        $reopened = Start-SettingsWindow 'settings:input'
        # 只把窗口交回去：其余调用的输出都丢掉，免得混进返回值。
        Open-Page $reopened 'clip' | Out-Null
        Select-PageTab $reopened 'clip-tab-clipboard' '历史记录上限' | Out-Null
        Show-CloudGroup $reopened | Out-Null
        return $reopened
    }

    $beforeEdit = (Read-SettingsSnapshotText $path)
    Show-CloudGroup $window
    Select-ComboChoice $window 'cloud-clipboard-source' 'WebDAV'
    $url = Wait-RelayControl $window 'cloud-clipboard-webdav-url'
    $username = Wait-RelayControl $window 'cloud-clipboard-webdav-username'
    $password = Wait-RelayControl $window 'cloud-clipboard-webdav-password'
    if (-not $password.Current.IsPassword) { throw 'WebDAV app password is not masked' }
    Wait-RelayControl $window 'cloud-clipboard-webdav-reveal' | Out-Null
    $url.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($webdavUrl)
    $username.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic@example.test')
    $password.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-app-password')
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'WebDAV draft was persisted before saving' }
    (Wait-RelayControl $window 'cloud-clipboard-webdav-test').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-WebDavResult $window $unavailable
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'WebDAV connection test wrote preferences' }
    # 保存先跑测试连接，不通过就不写盘，输入留在草稿里。先改一下应用密码，让上一次的结果收起，看到的失败说明才是这次保存跑出来的。
    (Wait-RelayControl $window 'cloud-clipboard-webdav-password').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-app-password-b')
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ((Find-ById $window 'cloud-clipboard-webdav-result') -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    if (Find-ById $window 'cloud-clipboard-webdav-result') { throw 'Editing the WebDAV draft did not clear the previous probe result' }
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-WebDavResult $window $unavailable
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'WebDAV failing connection test did not block saving' }
    if ((Find-ById $window 'cloud-clipboard-webdav-url').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne $webdavUrl) { throw 'Failed WebDAV save lost the draft' }

    # 已保存的网盘：选项、状态说明和输入框按偏好回填，应用密码框留空也能保存（沿用已存的密码）。之后改存中继，网盘配置要一起删掉。
    $window = Restart-WithSavedWebDav
    $combo = Find-ById $window 'cloud-clipboard-source'
    $selection = @($combo.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection())
    if ($selection.Count -ne 1 -or $selection[0].Current.Name -ne 'WebDAV') { throw 'Saved WebDAV source is not selected' }
    $status = Find-ById $window 'cloud-clipboard-webdav-status'
    if (-not $status -or -not $status.Current.Name.Contains($webdavUrl)) { throw 'Saved WebDAV source status is missing' }
    if ((Wait-RelayControl $window 'cloud-clipboard-webdav-url').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne $webdavUrl) { throw 'Reload lost the WebDAV address' }
    if ((Wait-RelayControl $window 'cloud-clipboard-webdav-username').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne 'synthetic@example.test') { throw 'Reload lost the WebDAV account' }
    Wait-RelayControl $window 'cloud-clipboard-save-source' | Out-Null
    Select-ComboChoice $window 'cloud-clipboard-source' '自建中继'
    $relayUrl = Wait-RelayControl $window 'cloud-clipboard-relay-url'
    $relayToken = Wait-RelayControl $window 'cloud-clipboard-relay-token'
    $relayUrl.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('http://127.0.0.1:9/msime')
    $relayToken.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-relay-token')
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $relay = Read-CloudPreference 'cloud_clipboard_relay'
        $webdav = Read-CloudPreference 'cloud_clipboard_webdav'
    } while ((-not $relay -or $webdav) -and [DateTime]::UtcNow -lt $deadline)
    if (-not $relay -or $relay.url -ne 'http://127.0.0.1:9/msime') { throw 'Relay was not saved over WebDAV' }
    if ($webdav) { throw 'Saving a relay retained the WebDAV credentials' }

    # 再存一次网盘配置（替掉中继，模拟测试通过后的保存结果），改回水杉账号时中继和网盘都不留。
    $document = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
    $document.preferences.PSObject.Properties.Remove('cloud_clipboard_relay')
    [IO.File]::WriteAllText($path, ($document | ConvertTo-Json -Depth 64), (New-Object Text.UTF8Encoding($false)))
    $window = Restart-WithSavedWebDav
    Select-ComboChoice $window 'cloud-clipboard-source' '水杉账号'
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $relay = Read-CloudPreference 'cloud_clipboard_relay'
        $webdav = Read-CloudPreference 'cloud_clipboard_webdav'
    } while (($relay -or $webdav) -and [DateTime]::UtcNow -lt $deadline)
    if ($relay -or $webdav) { throw 'Switching to account retained relay or WebDAV credentials' }
    if (Find-ById $window 'cloud-clipboard-webdav-status') { throw 'Account source still shows the WebDAV status' }
    Write-Host 'Cloud clipboard WebDAV masking, draft, failing probe blocking save, reload, relay replacement and account switch passed'
}
function Assert-DictionarySyncSettings {
    # 词库页「云端词库」标签里的词库同步（「我的词条」经 WebDAV 网盘）。只用合成凭据和本机拒绝连接的 9 号端口：测试连接答 dictionary_sync_unavailable，不发出真实请求。调用前的窗口先关掉，结束时留一个打开的窗口给调用方收尾。
    $unavailable = '连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。'
    $webdavUrl = 'http://127.0.0.1:9/dav'
    $path = Join-Path $state 'preferences.json'
    function Read-SyncPreference {
        $saved = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
        $property = $saved.preferences.PSObject.Properties['dictionary_sync_webdav']
        if ($property) { return $property.Value }
        return $null
    }
    function Wait-SyncResult($window, [string]$expected) {
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        do {
            Start-Sleep -Milliseconds 200
            $result = Find-ById $window 'dictionary-sync-result'
            $name = if ($result) { $result.Current.Name } else { '' }
        } while ($name -ne $expected -and [DateTime]::UtcNow -lt $deadline)
        if ($name -ne $expected) { throw "Dictionary sync probe result was '$name', expected '$expected'" }
    }
    # 关掉窗口，按需改写偏好文件，再从 settings:dictionary 路由打开词库页的「云端词库」标签，滚到词库同步那几组。
    function Open-SyncTab([scriptblock]$edit) {
        $script:process.CloseMainWindow() | Out-Null
        if (-not $script:process.WaitForExit(10000) -or $script:process.ExitCode -ne 0) { throw 'Settings window failed to close before the dictionary sync check' }
        if ($edit) {
            $document = (Read-SettingsSnapshotText $path) | ConvertFrom-Json
            & $edit $document | Out-Null
            $document.revision = [uint64]$document.revision + 1
            [IO.File]::WriteAllText($path, ($document | ConvertTo-Json -Depth 64), (New-Object Text.UTF8Encoding($false)))
        }
        $opened = Start-SettingsWindow 'settings:dictionary'
        # 只把窗口交回去：其余调用的输出都丢掉，免得混进返回值。
        Select-PageTab $opened 'lexicon-tab-cloud' '打开云词库' | Out-Null
        $scroll = (Find-ById $opened 'settings-content').GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
        if ($scroll.Current.VerticallyScrollable) { $scroll.SetScrollPercent(-1, 100) | Out-Null }
        return $opened
    }

    $window = Open-SyncTab $null
    $beforeEdit = (Read-SettingsSnapshotText $path)
    $url = Wait-RelayControl $window 'dictionary-sync-url'
    $username = Wait-RelayControl $window 'dictionary-sync-username'
    $password = Wait-RelayControl $window 'dictionary-sync-password'
    if (-not $password.Current.IsPassword) { throw 'Dictionary sync app password is not masked' }
    Wait-RelayControl $window 'dictionary-sync-reveal' | Out-Null
    if (Find-ById $window 'dictionary-sync-saved') { throw 'Dictionary sync shows a saved drive before one was saved' }
    if (Find-ById $window 'dictionary-sync-now') { throw 'Dictionary sync offers 立即同步 before a drive was saved' }
    $url.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($webdavUrl)
    $username.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic@example.test')
    $password.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-app-password')
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'Dictionary sync draft was persisted before saving' }
    (Wait-RelayControl $window 'dictionary-sync-test').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-SyncResult $window $unavailable
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'Dictionary sync connection test wrote preferences' }
    # 保存先跑测试连接，不通过就不写盘，输入留在草稿里。先改一下应用密码，让上一次的结果收起，看到的失败说明才是这次保存跑出来的。
    (Wait-RelayControl $window 'dictionary-sync-password').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-app-password-b')
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ((Find-ById $window 'dictionary-sync-result') -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
    if (Find-ById $window 'dictionary-sync-result') { throw 'Editing the dictionary sync draft did not clear the previous probe result' }
    (Wait-RelayControl $window 'dictionary-sync-save').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-SyncResult $window $unavailable
    if ((Read-SettingsSnapshotText $path) -ne $beforeEdit) { throw 'Dictionary sync failing connection test did not block saving' }
    if ((Find-ById $window 'dictionary-sync-url').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne $webdavUrl) { throw 'Failed dictionary sync save lost the draft' }
    if (Read-SyncPreference) { throw 'Failed dictionary sync save wrote the drive' }

    # 已保存的网盘（模拟测试通过后的保存结果）：地址和账号按偏好回填，现状说明、状态、「立即同步」「停止同步」和重置都在；与云剪贴板的网盘互不影响。
    $window = Open-SyncTab { param($document) $document.preferences | Add-Member -NotePropertyName dictionary_sync_webdav -NotePropertyValue ([pscustomobject]@{url = $webdavUrl; username = 'synthetic@example.test'; password = 'synthetic-app-password'}) -Force }
    if ((Wait-RelayControl $window 'dictionary-sync-url').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne $webdavUrl) { throw 'Reload lost the dictionary sync address' }
    if ((Wait-RelayControl $window 'dictionary-sync-username').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne 'synthetic@example.test') { throw 'Reload lost the dictionary sync account' }
    Wait-RelayControl $window 'dictionary-sync-password' | Out-Null
    $saved = Find-ById $window 'dictionary-sync-saved'
    if (-not $saved -or -not $saved.Current.Name.Contains($webdavUrl)) { throw 'Saved dictionary sync drive status is missing' }
    foreach ($id in @('dictionary-sync-status', 'dictionary-sync-now', 'dictionary-sync-clear', 'dictionary-sync-reset', 'dictionary-sync-restart')) {
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        while (-not (Find-ById $window $id) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
        if (-not (Find-ById $window $id)) { throw "Saved dictionary sync is missing $id" }
    }
    Save-Window $window 'dictionary-sync-saved.png'
    $cloudWebDav = ((Read-SettingsSnapshotText $path) | ConvertFrom-Json).preferences.PSObject.Properties['cloud_clipboard_webdav']
    if ($cloudWebDav) { throw 'Dictionary sync drive leaked into the cloud clipboard drive' }

    # 收尾：去掉合成的网盘配置，再开一个窗口留给调用方关闭。
    $window = Open-SyncTab { param($document) $document.preferences.PSObject.Properties.Remove('dictionary_sync_webdav') }
    if (Find-ById $window 'dictionary-sync-saved') { throw 'Removed dictionary sync drive still shows as saved' }
    Write-Host 'Dictionary sync masking, draft, failing probe blocking save, reload and saved status passed'
}
function Save-Window($element, [string]$file) {
    $bounds = $element.Current.BoundingRectangle
    if ($bounds.Width -lt 600 -or $bounds.Height -lt 400) { throw 'Settings window bounds are invalid' }
    $bitmap = New-Object Drawing.Bitmap([int]$bounds.Width, [int]$bounds.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen([int]$bounds.X, [int]$bounds.Y, 0, 0, $bitmap.Size)
        $colors = New-Object 'System.Collections.Generic.HashSet[int]'
        for ($x = 10; $x -lt $bitmap.Width; $x += 37) {
            for ($y = 10; $y -lt $bitmap.Height; $y += 29) {
                $colors.Add($bitmap.GetPixel($x, $y).ToArgb()) | Out-Null
            }
        }
        if ($colors.Count -lt 8) { throw 'Screenshot is blank; the runner needs an interactive desktop' }
        $bitmap.Save((Join-Path $outputPath $file), [Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}
try {
    New-Item -ItemType Directory -Force $state | Out-Null
    $env:MSIME_CLIENT_STATE_DIR = $state
    if ($VoiceHotkeyOnly) {
        if (-not [SettingsWindowProbe]::SetDllDirectory([IO.Path]::GetDirectoryName($programPath))) { throw 'Settings DLL path could not be set' }
        $defaults = [SettingsWindowProbe]::DefaultPreferences() | ConvertFrom-Json
        if (-not $defaults.ok) { throw 'Default preferences could not be loaded' }
        $defaults.value.keybindings | Add-Member -NotePropertyName toggle_voice_input -NotePropertyValue '' -Force
        $document = @{format_version = 1; revision = 1; preferences = $defaults.value}
        [IO.File]::WriteAllText((Join-Path $state 'preferences.json'), ($document | ConvertTo-Json -Depth 64), (New-Object Text.UTF8Encoding($false)))
        $window = Start-SettingsWindow 'settings:voice'
        Assert-VoiceToggleRecorder $window $process.MainWindowHandle
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw '语音录键验证窗口未正常退出' }
        $process = $null
        exit 0
    }
    New-CanvasPreviewFixture
    if (-not [SettingsWindowProbe]::SetDllDirectory([IO.Path]::GetDirectoryName($programPath))) { throw 'Settings DLL path could not be set' }
    $defaults = [SettingsWindowProbe]::DefaultPreferences() | ConvertFrom-Json
    if (-not $defaults.ok) { throw 'Default preferences could not be loaded' }
    $themes = if ($InputOnly) { @('light') } else { @('light', 'dark') }
    foreach ($theme in $themes) {
        $preferences = $defaults.value
        $preferences.theme = $theme
        $preferences.scheme = 'quanpin'
        $preferences.default_ime_mode = 'chinese'
        $preferences.clipboard_history = $true
        $document = @{format_version = 1; revision = 1; preferences = $preferences}
        [IO.File]::WriteAllText((Join-Path $state 'preferences.json'), ($document | ConvertTo-Json -Depth 64))
        # 只读取合成历史；不触碰系统剪贴板，长记录用于检查正文与操作区是否互相挤占。
        $entries = @(
            @{text = 'Synthetic clipboard layout sample: a long entry remains readable in a narrow settings window.'; timestampMs = 1792027710000; pinned = $false},
            @{text = 'Second synthetic clipboard entry'; timestampMs = 1792027700000; pinned = $false}
        )
        [IO.File]::WriteAllText((Join-Path $state 'clipboard_history.json'), ($entries | ConvertTo-Json -Depth 8))
        $window = Start-SettingsWindow 'settings:input'
        # 检查窗口真正交给系统的拖动尺寸边界，不能只检查 XAML 内部的最小宽度。
        $limits = [SettingsWindowProbe]::TrackLimits($process.MainWindowHandle)
        if ($limits.MinTrackSize.X -lt 800 -or $limits.MinTrackSize.Y -lt 560) { throw 'Native window minimum size was not applied' }
        # 窗口标题与 macOS 设置窗口相同，写作「<产品名> · 设置」。
        if (-not $window.Current.Name.EndsWith(' · 设置')) { throw "Unexpected settings window title: $($window.Current.Name)" }
        Assert-NativeInput $window $process.MainWindowHandle
        if ($InputOnly) {
            $process.CloseMainWindow() | Out-Null
            if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Settings input probe failed to close cleanly' }
            $process = $null
            continue
        }
        foreach ($width in @(1000, 800)) {
            if (-not [SettingsWindowProbe]::SetWindowPos($process.MainWindowHandle, [IntPtr]::Zero, 0, 0, $width, 780, 0x0040)) { throw 'Window resize failed' }
            Assert-WindowSize $process.MainWindowHandle $width 780
            foreach ($page in $pages) {
                if ($process.HasExited) { throw "Settings exited on $page" }
                # 复用导航等待，既验证正文身份，也处理侧栏虚拟化。
                Open-Page $window $page
                $content = Find-ById $window 'settings-content'
                if ($content.Current.BoundingRectangle.Width -lt 480) { throw "No usable content column on $page" }
                # 与 macOS 一样，标题栏里只有页名，没有跨页面的历史返回按钮。
                if (Find-ByName $window '返回') { throw "Settings title bar still has a history back button on $page" }
                # 页首照 macOS 的水杉娘引导（SettingsMascotIntro.h）：除「关于」外，正文顶部是一句说明配插画，说明带 settings-page-<页面>；「关于」没有引导，这个 ID 落在品牌卡的产品名上。
                $introName = Wait-PageIntroName $window $page
                $content = Find-ById $window 'settings-content'
                if ($page -eq 'about') {
                    if (-not $window.Current.Name.StartsWith($introName + ' · ')) { throw "About page identifies itself as '$introName' instead of the product name" }
                } elseif (-not $introName.EndsWith('。')) {
                    throw "Page intro of $page is not a sentence: '$introName'"
                }
                if ($page -eq 'account' -and $introName -ne '管理账号与跨设备设置同步。') { throw "Account page intro does not follow macOS: '$introName'" }
                # 再等一轮呈现，避免 UIA 树已经更新而屏幕仍是上一页的画面。
                Start-Sleep -Milliseconds 500
                if ($page -eq 'clip') {
                    # 照 macOS 分「设置」「历史记录」两个标签，打开时是「设置」：这里有开关和云剪贴板，没有历史条目。
                    Select-PageTab $window 'clip-tab-clipboard' '历史记录上限'
                    $content = Find-ById $window 'settings-content'
                    if (-not (Find-ById $content 'cloud-clipboard-source')) { throw 'Clipboard settings tab is missing the cloud clipboard group' }
                    if (Find-ById $content 'clipboard-entry-0-preview') { throw 'Clipboard history rendered inside the settings tab' }
                    Assert-PageIntro $window 'clip' '设置剪贴板历史的保存与云端同步。'
                }
                if ($page -eq 'voice') {
                    # 照 macOS 分「识别与输入」「录音行为」两个标签，打开时是「识别与输入」：「识别」组里紧跟识别服务的是「服务配置 ›」一行，录音设备在另一个标签里，服务的接入收在子视图里。
                    Select-PageTab $window 'voice-tab-input' '结果提交策略'
                    $content = Find-ById $window 'settings-content'
                    if (-not (Find-ById $content 'voice-service')) { throw 'Voice input tab is missing the 服务配置 row' }
                    if (Find-ByName $content '录音设备') { throw 'Capture devices rendered inside the voice input tab' }
                    if (Find-ByName $content '文本润色') { throw 'Voice polish rendered outside the 服务配置 view' }
                    Assert-PageIntro $window 'voice' '配置语音识别、服务与输入快捷键。'
                }
                if ($page -eq 'osk') {
                    # 完整版提供手写：照 macOS 分「键盘」「手写」两个标签，打开时是「键盘」；「恢复默认」的说明只提这一页看得到的几项，不再提工具栏按钮。
                    Select-PageTab $window 'osk-tab-keyboard' '打开屏幕键盘'
                    $content = Find-ById $window 'settings-content'
                    if (-not (Find-InfoTip $content '恢复默认' '高度、间距和顶部语音入口回到默认')) { throw 'Screen keyboard tab is missing the reset row' }
                    if (Find-ByName $content '打开手写识别板') { throw 'Handwriting rendered inside the keyboard tab' }
                    Assert-PageIntro $window 'osk' '调整屏幕键盘布局与外观。'
                }
                if ($page -eq 'typing') {
                    # 照 macOS 分六个标签，打开时是「输入方案」：方案组的第一行是「中文方案」卡片。
                    Select-PageTab $window 'input-tab-scheme' '中文方案'
                }
                if ($page -eq 'shortcuts') {
                    # 照 macOS 分「按键设置」「按键速查」两个标签；选词与翻页在输入页，桌面上没有「恢复默认快捷键」。
                    Select-PageTab $window 'shortcuts-tab-settings' 'Ctrl+Space（由 Windows 管理）'
                    Assert-SettingsWheelHitRegions $window $process.MainWindowHandle "$theme-$width"
                    $content = Find-ById $window 'settings-content'
                    foreach ($name in @('恢复默认快捷键', '以词定字')) {
                        if (Find-ByName $content $name) { throw "Shortcuts settings tab still shows $name" }
                    }
                }
                if ($page -eq 'expression') {
                    # 照 macOS 分「标点设置」「候选释义」「翻译服务」三个标签；混输等输入习惯在输入页。
                    Select-PageTab $window 'expression-tab-punctuation' '引号样式'
                    if (Find-ByName (Find-ById $window 'settings-content') '中英混输') { throw 'Input habits still render on the punctuation page' }
                }
                if ($page -eq 'lexicon') {
                    # 照 macOS 分「查找词库」「管理词库」「云端词库」三个标签，打开时是「查找词库」：进页面就列出词条（翻页器里有状态），搜索框叫「词条或编码」，没有「查询」按钮；导入导出不在这个标签里。
                    Select-PageTab $window 'lexicon-tab-lookup' '词条或编码'
                    $content = Find-ById $window 'settings-content'
                    if (-not (Find-ById $content 'lexicon-search')) { throw 'Lexicon lookup tab is missing the search box' }
                    if (-not (Find-ById $content 'lexicon-page-status')) { throw 'Lexicon lookup tab is missing the pager status' }
                    if (Find-ByName $content '查询') { throw 'Lexicon lookup still waits for a 查询 button' }
                    if (Find-ById $content 'lexicon-export-all') { throw 'Import and export rendered inside the lookup tab' }
                }
                if ($page -eq 'about') {
                    # 照 macOS 分「版本更新」「帮助与反馈」「系统信息」「许可与隐私」四个标签，打开时是「版本更新」：品牌卡和「当前版本 / 检查更新」在这里，反馈表单不在；「复制下载链接」与 macOS 一样去掉了。
                    Select-PageTab $window 'about-tab-version' '当前版本'
                    $content = Find-ById $window 'settings-content'
                    if (-not (Find-ByName $content '检查更新')) { throw 'About version tab is missing the update check' }
                    if (-not (Find-ByName $content '更新通道')) { throw 'About version tab is missing the update channel' }
                    if (Find-ByName $content '复制下载链接') { throw 'About page still offers 复制下载链接' }
                    if (Find-ByName $content '反馈描述') { throw 'Feedback rendered inside the about version tab' }
                }
                if ($page -eq 'dev') {
                    # 照 macOS 的维护页依次是输入法服务、诊断日志、数据目录和 Windows 独有的设置文件，诊断日志之后还有「命令行工具」；只查行在不在，不点「重启」，免得重启跑验收这台机器上真正的 Server。开关的缺省值和保存见 Assert-CommandLineCandidates。
                    foreach ($name in @('输入法服务', '立即重启输入法服务', '诊断日志', '日志文件', 'TSF 端日志', '命令行工具', '命令行查询候选', '数据目录', '当前目录', '设置文件')) {
                        if (-not (Find-ByName $content $name)) { throw "Maintenance page is missing: $name" }
                    }
                }
                if ($page -eq 'candidate') {
                    # 照 macOS 分「布局」「字体与大小」「窗口样式」「预编辑」四个标签，打开时是「布局」；预览在无标题卡片里，「游戏」组不属于哪个标签、一直在。
                    Select-PageTab $window 'candidate-tab-layout' '候选项排列方式'
                    Assert-CandidatePageSize $window "$theme-$width"
                    $content = Find-ById $window 'settings-content'
                    foreach ($name in @('预览：固定样例随当前设置草稿变化，不代表实际输入候选。', '每页候选项数量', '跟随光标', '在游戏中显示候选窗口')) {
                        if (-not (Find-ByName $content $name)) { throw "Candidate layout tab is missing: $name" }
                    }
                    foreach ($name in @('字号', '行内预编辑', '更多选项')) {
                        if (Find-ByName $content $name) { throw "Candidate layout tab still shows $name" }
                    }
                }
                if ($page -eq 'toolbar') {
                    # 缩放和图标尺寸合成一个「工具栏大小」。
                    if (-not (Find-ByName $content '工具栏大小')) { throw 'Toolbar size select is missing' }
                    foreach ($name in @('工具栏缩放', '图标尺寸')) {
                        if (Find-ByName $content $name) { throw "Toolbar page still offers $name apart" }
                    }
                }
                if ($page -eq 'plugins') {
                    # 「我的插件」照 macOS 的插件库：两张快捷卡片、搜索框和底部折叠的导入说明，「新建插件」和导入按钮在「已安装」标题旁，不再有「导入插件」组的「从文件导入」一行。
                    foreach ($name in @('声音与效果', '@ 名单', '新建插件', '搜索已安装插件', '导入文件夹', '插件导入说明')) {
                        if (-not (Find-ByName $content $name)) { throw "Plugin library is missing $name" }
                    }
                    if (Find-ByName $content '从文件导入') { throw 'Plugin page still has the old import row' }
                }
                Save-Window $window "$theme-$width-$page.png"
                Write-Host "$theme $width $page rendered"
                if ($page -eq 'plugins' -and $theme -eq 'light' -and $width -eq 1000) {
                    # 插件编辑器只在第一轮走一遍：保存的包留在合成数据目录里，后几轮的插件库照常列出它。
                    Assert-PluginEditor $window "$theme-$width"
                }
                if ($page -eq 'clip') {
                    # 「历史记录」标签：合成的两条记录在第一页，不足一页时没有翻页器；操作按行内可用宽度排开或收进「更多操作」，两种都不能挤掉正文或伸出正文列。
                    Select-PageTab $window 'clip-tab-history' '历史记录'
                    Assert-PageIntro $window 'clip' '查看、复制和管理已保存的剪贴板记录。'
                    # 行宽要等第一轮布局后才决定操作是排开还是收进菜单；再等一轮呈现再量。
                    Start-Sleep -Milliseconds 500
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        $preview = Find-ById $window 'clipboard-entry-0-preview'
                        if ($preview) { break }
                        Start-Sleep -Milliseconds 200
                    } while ([DateTime]::UtcNow -lt $deadline)
                    if (-not $preview) { throw 'Synthetic clipboard history was not rendered' }
                    $clear = Find-ById $window 'clipboard-clear'
                    if (-not $clear) { throw 'Clear history is missing from the history tab' }
                    if ($clear.Current.BoundingRectangle.Bottom -gt $preview.Current.BoundingRectangle.Top) { throw 'Clear history is not above the clipboard list' }
                    if (Find-ById $window 'clipboard-page-status') { throw 'Pager shown for a single page of clipboard history' }
                    $content = Find-ById $window 'settings-content'
                    $copy = Find-ById $window 'clipboard-entry-0-copy'
                    $last = Find-ById $window 'clipboard-entry-0-remove'
                    if (-not $last) { $last = Find-ById $window 'clipboard-entry-0-actions' }
                    if (-not $copy -or -not $last) { throw 'Clipboard preview or actions are missing' }
                    $textBounds = $preview.Current.BoundingRectangle
                    if ($textBounds.Width -lt 180 -or $textBounds.Right -gt $copy.Current.BoundingRectangle.Left) { throw 'Clipboard actions crowded out the text preview' }
                    if ($last.Current.BoundingRectangle.Right -gt $content.Current.BoundingRectangle.Right) { throw 'Clipboard actions overflow the content column' }
                    $pin = Find-ById $window 'clipboard-entry-0-pin'
                    $more = Find-ById $window 'clipboard-entry-0-actions'
                    if ([bool]$pin -eq [bool]$more) { throw 'Clipboard actions must be either inline or in the overflow menu' }
                    Save-Window $window "$theme-$width-clip-history.png"
                    Select-PageTab $window 'clip-tab-clipboard' '历史记录上限'
                }
                if ($page -eq 'typing') {
                    # 其余五个标签逐个截图；「二三候选」照 macOS 不在设置里，偏好本身不动。拼音纠错不叫「自动纠错」，是「高级选项」里按方案出现的全拼纠错 / 双拼纠错组，见 Assert-PinyinAutocorrect。
                    foreach ($pair in @(@('mode', '默认中英文'), @('selection', '以词定字'), @('candidates', '云候选来源'), @('output', '中文输出'), @('advanced', '启用模糊音'))) {
                        Select-PageTab $window "input-tab-$($pair[0])" $pair[1]
                        $content = Find-ById $window 'settings-content'
                        foreach ($name in @('二三候选', '自动纠错')) {
                            if (Find-ByName $content $name) { throw "Input page still shows $name" }
                        }
                        Save-Window $window "$theme-$width-typing-$($pair[0]).png"
                    }
                    Select-PageTab $window 'input-tab-scheme' '中文方案'
                }
                if ($page -eq 'shortcuts') {
                    Select-PageTab $window 'shortcuts-tab-reference' '选择候选'
                    if (-not (Find-ByName (Find-ById $window 'settings-content') '打开屏幕键盘')) { throw 'Shortcut reference is missing the screen keyboard chord' }
                    Save-Window $window "$theme-$width-shortcuts-reference.png"
                    Select-PageTab $window 'shortcuts-tab-settings' 'Ctrl+Space（由 Windows 管理）'
                }
                if ($page -eq 'expression') {
                    Select-PageTab $window 'expression-tab-gloss' '显示英文释义'
                    Save-Window $window "$theme-$width-expression-gloss.png"
                    Select-PageTab $window 'expression-tab-providers' '候选词翻译服务'
                    Save-Window $window "$theme-$width-expression-providers.png"
                    Select-PageTab $window 'expression-tab-punctuation' '引号样式'
                }
                if ($page -eq 'lexicon') {
                    # 「管理词库」：导入与导出（导入、导出当前类型、导出全部）、词库更新（单独一行词库版本）、进入背单词子页的一行和学习数据；「云端词库」只有打开云词库面板的一行。
                    Select-PageTab $window 'lexicon-tab-manage' '导入与导出'
                    $content = Find-ById $window 'settings-content'
                    foreach ($id in @('lexicon-import', 'lexicon-export', 'lexicon-export-all', 'dictionary-update-status', 'lexicon-vocabulary')) {
                        if (-not (Find-ById $content $id)) { throw "Lexicon manage tab is missing $id" }
                    }
                    if (-not (Find-ByName $content '词库版本')) { throw 'Dictionary updates do not show the version on its own row' }
                    if (Find-ById $content 'lexicon-search') { throw 'Lookup rendered inside the manage tab' }
                    Save-Window $window "$theme-$width-lexicon-manage.png"
                    # 背单词是一行进入的子页，头上的「词库」回到「管理词库」。
                    (Find-ById $content 'lexicon-vocabulary').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        Start-Sleep -Milliseconds 200
                        $back = Find-ById $window 'vocabulary-back'
                    } while (-not $back -and [DateTime]::UtcNow -lt $deadline)
                    if (-not $back) { throw 'The 背单词 row did not open its sub-view' }
                    if (Find-ById (Find-ById $window 'settings-content') 'lexicon-tab-manage') { throw 'The 背单词 sub-view still shows the lexicon tabs' }
                    # 和 macOS 一样，设置里只管理词书，复习交给背单词面板：读到进度后有「开始复习」（读不出进度时只有「重试」），任何时候都没有复习卡片。
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        Start-Sleep -Milliseconds 200
                        $start = Find-ById $window 'vocabulary-start'
                        $retry = Find-ByName (Find-ById $window 'settings-content') '重试'
                    } while (-not $start -and -not $retry -and [DateTime]::UtcNow -lt $deadline)
                    if (-not $start -and -not $retry) { throw 'The 背单词 sub-view has no 开始复习 button' }
                    if (Find-ById $window 'vocabulary-card') { throw 'The 背单词 sub-view still deals review cards in settings' }
                    # 和 macOS 的 vocabulary 子页一样没有页首引导，正文从「‹ 词库」开始。
                    if (Find-ById (Find-ById $window 'settings-content') 'settings-page-lexicon') { throw 'The 背单词 sub-view still shows the 词库 page intro' }
                    Start-Sleep -Milliseconds 500
                    Save-Window $window "$theme-$width-lexicon-vocabulary.png"
                    # 读到进度后整页重画过，第一次找到的返回按钮已经不在树里，用之前重新找。
                    $back = Find-ById $window 'vocabulary-back'
                    if (-not $back) { throw 'The 背单词 sub-view lost its way back after loading' }
                    $back.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    Select-PageTab $window 'lexicon-tab-manage' '导入与导出'
                    Select-PageTab $window 'lexicon-tab-cloud' '打开云词库'
                    Save-Window $window "$theme-$width-lexicon-cloud.png"
                    Select-PageTab $window 'lexicon-tab-lookup' '词条或编码'
                }
                if ($page -eq 'voice') {
                    # 「服务配置 ›」在原处换成子视图：带返回按钮的「服务配置」标题和「文本润色」等组，标签条不在；返回回到「识别与输入」。
                    (Find-ById $window 'voice-service').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        Start-Sleep -Milliseconds 200
                        $back = Find-ById $window 'voice-service-back'
                        $content = Find-ById $window 'settings-content'
                        $polish = $content -and (Find-ByName $content '文本润色')
                    } while ((-not $back -or -not $polish) -and [DateTime]::UtcNow -lt $deadline)
                    if (-not $back -or -not $polish) { throw 'The 服务配置 row did not open its view' }
                    if (Find-ById $window 'voice-tab-input') { throw 'Voice tabs are still shown inside the 服务配置 view' }
                    Start-Sleep -Milliseconds 350
                    Save-Window $window "$theme-$width-voice-service.png"
                    $back.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    Select-PageTab $window 'voice-tab-input' '结果提交策略'
                    # 「录音行为」标签：录音行为和录音设备两组。
                    Select-PageTab $window 'voice-tab-recording' '录音设备'
                    Assert-PageIntro $window 'voice' '调整录音提示音与录音期间的静音行为。'
                    if (Find-ById (Find-ById $window 'settings-content') 'voice-service') { throw 'The 服务配置 row rendered inside the recording tab' }
                    Save-Window $window "$theme-$width-voice-recording.png"
                    Select-PageTab $window 'voice-tab-input' '结果提交策略'
                }
                if ($page -eq 'about') {
                    # 反馈并进关于后仍须能生成报告；不能只删除旧导航的验收。它在「帮助与反馈」标签里，前面是进帮助视图的「帮助」一行。
                    Select-PageTab $window 'about-tab-feedback' '反馈描述'
                    $content = Find-ById $window 'settings-content'
                    foreach ($name in @('反馈类型', '反馈描述', '复制报告', '查看 Issues', '复制群号', '打开群组')) {
                        if (-not (Find-ByName $content $name)) { throw "About page is missing merged feedback content: $name" }
                    }
                    if (Find-ById $window 'feedback') { throw 'Merged feedback page is still in the sidebar' }
                    Save-Window $window "$theme-$width-about-feedback.png"
                    # 「帮助」一行在原处换成帮助视图（macOS 的「帮助」子页），这时没有标签条；顶上的「关于」回到标签页。
                    (Find-ById $window 'about-help-entry').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        Start-Sleep -Milliseconds 200
                        $content = Find-ById $window 'settings-content'
                        $guide = if ($content) { Find-ByName $content '开始输入' } else { $null }
                    } while ((-not $guide -or -not (Find-ById $window 'about-help')) -and [DateTime]::UtcNow -lt $deadline)
                    if (-not $guide -or -not (Find-ById $window 'about-help')) { throw 'Help entry did not open the help view' }
                    if (Find-ById $window 'about-tab-version') { throw 'Help view still shows the about tabs' }
                    Start-Sleep -Milliseconds 350
                    Save-Window $window "$theme-$width-about-help.png"
                    (Find-ById $window 'about-help-back').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        Start-Sleep -Milliseconds 200
                        $entry = Find-ById $window 'about-help-entry'
                    } while (-not $entry -and [DateTime]::UtcNow -lt $deadline)
                    if (-not $entry) { throw 'Help view did not return to the about tabs' }
                    # 「系统信息」逐项列出，最后一行一键复制；不点复制，免得动验收机器的剪贴板。
                    Select-PageTab $window 'about-tab-system' '复制系统信息'
                    $content = Find-ById $window 'settings-content'
                    foreach ($name in @('应用版本', '系统', '处理器架构')) {
                        if (-not (Find-ByName $content $name)) { throw "About system tab is missing: $name" }
                    }
                    Save-Window $window "$theme-$width-about-system.png"
                    # 「许可与隐私」的顺序照 macOS，末尾是「发送哪些内容」折叠区；不碰统计开关。
                    Select-PageTab $window 'about-tab-privacy' '许可与版权'
                    $content = Find-ById $window 'settings-content'
                    foreach ($name in @('开源许可协议', '第三方组件许可', '隐私政策', '匿名使用统计', '发送哪些内容')) {
                        if (-not (Find-ByName $content $name)) { throw "About privacy tab is missing: $name" }
                    }
                    Save-Window $window "$theme-$width-about-privacy.png"
                    # 回到打开时的标签，下一轮打开「关于」时品牌卡上的 settings-page-about 还在。
                    Select-PageTab $window 'about-tab-version' '当前版本'
                }
                if ($page -eq 'osk') {
                    # 「手写」标签只有「手写识别板」一组：打开面板的一行和手写板示意。
                    Select-PageTab $window 'osk-tab-handwriting' '打开手写识别板'
                    Assert-PageIntro $window 'osk' '设置手写识别与输入方式。'
                    if (-not (Find-ById $window 'handwriting-preview-stroke')) { throw 'Handwriting preview is missing' }
                    if (Find-ByName (Find-ById $window 'settings-content') '打开屏幕键盘') { throw 'Keyboard groups rendered inside the handwriting tab' }
                    Save-Window $window "$theme-$width-osk-handwriting.png"
                    Select-PageTab $window 'osk-tab-keyboard' '打开屏幕键盘'
                }
                if ($page -eq 'ai') {
                    # 两类配置共用一个导航；切到连接页截图，再回到辅助页，不能只覆盖默认标签。
                    foreach ($tab in @('connection', 'assistance')) {
                        $control = Find-ById $window "ai-settings-$tab"
                        if (-not $control) { throw "AI tab is missing: $tab" }
                        $control.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
                        Start-Sleep -Milliseconds 500
                        $content = Find-ById $window 'settings-content'
                        $expected = if ($tab -eq 'connection') { '连接 AI 助手' } else { '启用 AI 辅助' }
                        if (-not (Find-ByName $content $expected)) { throw "AI tab content is missing: $expected" }
                        Save-Window $window "$theme-$width-ai-$tab.png"
                        if ($tab -eq 'connection') {
                            # 合成状态目录里没有运行时选项，照 macOS 的 McpConnectSection 只说尚未完成初始化，不画助手标签；有运行时选项时的样子在主题循环之后另开一次窗口核对。
                            if (-not (Find-ByName $content '输入法尚未完成初始化，完成设置向导后即可连接。')) { throw 'MCP connect without runtime options does not show the setup notice' }
                            if (Find-ById $content 'mcp-tab-claude_code') { throw 'MCP connect shows assistant tabs without runtime options' }
                        }
                    }
                    # 照 macOS 的服务组：接口地址和「服务模型」常驻，自定义提示词单独一个默认收起的折叠区，没有「更多选项」。
                    $content = Find-ById $window 'settings-content'
                    if (-not (Find-ById $content 'ai-provider')) { throw 'AI provider select is missing' }
                    foreach ($name in @('AI 接口地址', '服务模型')) {
                        if (-not (Find-ByName $content $name)) { throw "AI service group is missing $name outside a fold" }
                    }
                    $prompt = Find-ById $content 'ai-prompt-fold'
                    if (-not $prompt -or $prompt.Current.ItemStatus -ne '已收起') { throw 'AI custom prompt is not a collapsed fold of its own' }
                    if (Find-ByName $content '更多选项') { throw 'AI service group still folds rows into 更多选项' }
                }
                if ($page -eq 'candidate') {
                    # 「布局」已随整页截过，其余三个标签各截一张，「游戏」组在每个标签下都在；最后回到「布局」，下一个宽度照样从它开始。
                    foreach ($tab in @(@('font', '字号'), @('style', '圆角大小'), @('preedit', '行内预编辑'))) {
                        Select-PageTab $window "candidate-tab-$($tab[0])" $tab[1]
                        if (-not (Find-ByName (Find-ById $window 'settings-content') '在游戏中显示候选窗口')) { throw "Candidate $($tab[0]) tab lost the 游戏 group" }
                        if ($tab[0] -eq 'style') {
                            # 合成偏好没设圆角，照 macOS「跟随皮肤」按钮是按下的，说明写「跟随皮肤；设置后覆盖皮肤自带的圆角」。共享页的圆角和不透明度滑块只有读屏用的 valueText，滑块旁不画数值：圆角滑块后面紧跟的不能是数值文字，也不能有写着「100%」的不透明度数值。
                            $styleContent = Find-ById $window 'settings-content'
                            $followName = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '跟随皮肤')
                            $toggleable = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::IsTogglePatternAvailableProperty, $true)
                            $followCondition = New-Object System.Windows.Automation.AndCondition($followName, $toggleable)
                            $follow = $styleContent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $followCondition)
                            if (-not $follow -or $follow.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -ne [System.Windows.Automation.ToggleState]::On) { throw 'Candidate 跟随皮肤 is not pressed while the radius follows the skin' }
                            $radiusName = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '圆角大小')
                            $sliderType = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Slider)
                            $radiusCondition = New-Object System.Windows.Automation.AndCondition($radiusName, $sliderType)
                            $radius = $styleContent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $radiusCondition)
                            $radiusValue = if ($radius) { [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetNextSibling($radius) } else { $null }
                            if ($radiusValue -and $radiusValue.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text) { throw "Candidate 圆角大小 draws a visible value next to the slider: $($radiusValue.Current.Name)" }
                            if (-not (Find-ByName $styleContent '跟随皮肤；设置后覆盖皮肤自带的圆角')) { throw 'Candidate 圆角大小 description does not read 跟随皮肤 while the radius follows the skin' }
                            $percentName = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '100%')
                            $textType = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
                            if ($styleContent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.AndCondition($percentName, $textType)))) { throw 'Candidate 不透明度 draws a visible value next to the slider' }
                        }
                        Start-Sleep -Milliseconds 350
                        Save-Window $window "$theme-$width-candidate-$($tab[0]).png"
                    }
                    Select-PageTab $window 'candidate-tab-layout' '候选项排列方式'
                }
            }
        }
        Assert-SentenceModelSettings $window $theme
        Open-Page $window 'dev'
        Assert-CommandLineCandidates $window
        # 通过真实单选控件切换方案，再读合成偏好确认保存成功，避免只验证静态页面。方案卡片在「输入方案」标签里。
        Open-Page $window 'typing'
        Select-PageTab $window 'input-tab-scheme' '中文方案'
        Start-Sleep -Milliseconds 700
        foreach ($scheme in @('shuangpin', 'wubi', 'quanpin')) {
            $radio = Find-ById $window "scheme-$scheme"
            if (-not $radio -or -not $radio.Current.IsEnabled) { throw "Scheme option is unavailable: $scheme" }
            $radio.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 200
                $saved = (Read-SettingsSnapshotText (Join-Path $state 'preferences.json')) | ConvertFrom-Json
            } while ($saved.preferences.scheme -ne $scheme -and [DateTime]::UtcNow -lt $deadline)
            if ($saved.preferences.scheme -ne $scheme) { throw "Scheme was not saved: $scheme" }
            $content = Find-ById $window 'settings-content'
            $pattern = $content.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
            if ($pattern.Current.VerticallyScrollable) { $pattern.SetScrollPercent(-1, 0) }
            Start-Sleep -Milliseconds 350
            Assert-SelectedAccent (Find-ById $window "scheme-$scheme")
            if ($pattern.Current.VerticallyScrollable) { $pattern.SetScrollPercent(-1, 55) }
            Start-Sleep -Milliseconds 500
            Save-Window $window "$theme-scheme-$scheme.png"
            Write-Host "$theme $scheme selected and saved"
            if ($scheme -eq 'wubi') { Assert-WubiCandidateSelection $window }
            if ($scheme -ne 'quanpin') {
                Open-Page $window 'toolbar'
                $expected = if ($scheme -eq 'shuangpin') { '双' } else { '五' }
                $deadline = [DateTime]::UtcNow.AddSeconds(10)
                do {
                    Start-Sleep -Milliseconds 200
                    $badge = Find-ByName (Find-ById $window 'settings-content') $expected
                } while (-not $badge -and [DateTime]::UtcNow -lt $deadline)
                if (-not $badge -or $badge.Current.IsOffscreen) { throw "Toolbar preview does not reflect scheme: $scheme" }
                Open-Page $window 'typing'
            }
            Assert-PinyinAutocorrect $window $scheme
        }
        Assert-CanvasPreview $window $theme
        # 侧栏搜索照 macOS：有查询时页面列表整个换成结果列表，名字匹配的页面排在最前，设置行的第二行（读屏帮助文本）是「页 › 标签 › 分组」；查不到时写「没有匹配的设置」；Esc 清空查询，页面列表回来。
        $search = Find-ById $window 'settings-search'
        $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
        $input = $search.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if (-not $input -and $search.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) { $input = $search }
        if (-not $input) { throw 'Settings search input is missing' }
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('剪贴板')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $results = Find-ById $window 'settings-search-results'
            $first = if ($results) { Find-ById $results 'settings-search-result-0' } else { $null }
            $row = if ($results) { Find-ByName $results '剪贴板历史' } else { $null }
        } while ((-not $first -or -not $row) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $first -or $first.Current.Name -ne '剪贴板' -or $first.Current.HelpText) { throw 'Search results do not start with the matching page' }
        if (-not $row -or $row.Current.HelpText -ne '剪贴板 › 设置 › 剪贴板') { throw "Search result path is wrong: $($row.Current.HelpText)" }
        $listed = Find-ById $window 'typing'
        if ($listed -and -not $listed.Current.IsOffscreen) { throw 'The page list is still shown while searching' }
        Save-Window $window "$theme-search-results.png"
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('没有这个设置项')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $results = Find-ById $window 'settings-search-results'
            $empty = if ($results) { Find-ByName $results '没有匹配的设置' } else { $null }
        } while (-not $empty -and [DateTime]::UtcNow -lt $deadline)
        if (-not $empty) { throw 'Search without matches does not say 没有匹配的设置' }
        $input.SetFocus()
        [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $item = Find-ById $window 'typing'
        } while ((-not $item -or $item.Current.IsOffscreen) -and [DateTime]::UtcNow -lt $deadline)
        $results = Find-ById $window 'settings-search-results'
        if (-not $item -or $item.Current.IsOffscreen -or ($results -and -not $results.Current.IsOffscreen)) { throw 'Escape did not clear the search and bring the page list back' }
        # 搜索落到页内标签里的行时先切到那个标签：「打开手写识别板」在「键盘与手写」页的「手写」标签里，从「键盘」标签搜它。
        Open-Page $window 'osk'
        Select-PageTab $window 'osk-tab-keyboard' '打开屏幕键盘'
        $search = Find-ById $window 'settings-search'
        $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
        $input = $search.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if (-not $input -and $search.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) { $input = $search }
        if (-not $input) { throw 'Settings search input is missing' }
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('打开手写识别板')
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $tab = Find-ById $window 'osk-tab-handwriting'
            $target = Find-ByName (Find-ById $window 'settings-content') '打开手写识别板'
        } while ((-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $target) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $target) { throw 'Search did not switch to the tab holding its target' }
        Assert-PageIntro $window 'osk' '设置手写识别与输入方式。'
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('')
        Select-PageTab $window 'osk-tab-keyboard' '打开屏幕键盘'
        # 输入页同样：「@ 地名」在「高级选项」标签「逐个模式」的「更多选项」里，从「输入方案」标签搜它要先切标签再展开折叠区。
        Open-Page $window 'typing'
        Select-PageTab $window 'input-tab-scheme' '中文方案'
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('@ 地名')
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $tab = Find-ById $window 'input-tab-advanced'
            $target = Find-ByName (Find-ById $window 'settings-content') '@ 地名'
        } while ((-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $target) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $target) { throw 'Search did not open the input tab and fold holding its target' }
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('')
        Select-PageTab $window 'input-tab-scheme' '中文方案'
        # 搜索落到折叠区里的行时要展开那个折叠区：「候选悬停色」收在「主题」页默认收起的「自定义配色」里，收起时它不在 UIA 树里。
        Open-Page $window 'themes'
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $fold = Find-ByName (Find-ById $window 'settings-content') '自定义配色'
        } while (-not $fold -and [DateTime]::UtcNow -lt $deadline)
        if (-not $fold) { throw 'Missing 自定义配色 fold on the themes page' }
        if ($fold.Current.ItemStatus -ne '已收起' -or (Find-ByName (Find-ById $window 'settings-content') '候选悬停色')) { throw '自定义配色 is not collapsed before the search' }
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('候选悬停色')
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $content = Find-ById $window 'settings-content'
            $fold = if ($content) { Find-ByName $content '自定义配色' } else { $null }
            $hover = if ($content) { Find-ByName $content '候选悬停色' } else { $null }
        } while ((-not $fold -or $fold.Current.ItemStatus -ne '已展开' -or -not $hover) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $fold -or $fold.Current.ItemStatus -ne '已展开' -or -not $hover) { throw 'Search did not open the 自定义配色 fold holding its target' }
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('')
        # 搜索必须让目标可见：候选窗口页的行在哪个标签就切到哪个标签；主字体在预设下本来收起，搜索落到它时还要画出来。
        Open-Page $window 'candidate'
        Select-PageTab $window 'candidate-tab-layout' '候选项排列方式'
        $search = Find-ById $window 'settings-search'
        if (-not $search) { throw 'Settings search is missing' }
        $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
        $input = $search.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
        if (-not $input -and $search.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) { $input = $search }
        if (-not $input) { throw 'Settings search input is missing' }
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('主字体')
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $tab = Find-ById $window 'candidate-tab-font'
            $font = Find-ByName (Find-ById $window 'settings-content') '主字体'
        } while ((-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $font) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $font) { throw 'Font search did not switch to 字体与大小 and show its target' }
        Start-Sleep -Milliseconds 350
        Save-Window $window "$theme-search-candidate-font.png"
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('行内预编辑')
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $tab = Find-ById $window 'candidate-tab-preedit'
            $inline = Find-ByName (Find-ById $window 'settings-content') '行内预编辑'
        } while ((-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $inline) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $tab -or $tab.Current.ItemStatus -ne '已选择' -or -not $inline) { throw 'Search did not switch to the 预编辑 tab holding its target' }
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('')
        Select-PageTab $window 'candidate-tab-layout' '候选项排列方式'
        # 与 macOS 一样，示例与预览明暗在离开再回到本页后保留，但不写入真实设置。
        $preferencesBeforePreview = (Read-SettingsSnapshotText (Join-Path $state 'preferences.json'))
        $sample = Find-ById $window 'candidate-preview-sample'
        if (-not $sample) { throw 'Candidate preview sample input is missing' }
        $sample.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('杉树🌲 Alpha')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $first = Find-ById $window 'candidate-preview-word-0'
            $third = Find-ById $window 'candidate-preview-word-2'
        } while ((-not $first -or $first.Current.Name -ne '杉树🌲' -or -not $third -or $third.Current.Name -ne '杉树🌲') -and [DateTime]::UtcNow -lt $deadline)
        if (-not $first -or $first.Current.Name -ne '杉树🌲' -or -not $third -or $third.Current.Name -ne '杉树🌲') { throw 'Candidate preview did not cycle the custom sample' }
        $appearance = if ($theme -eq 'light') { '预览深色' } else { '预览浅色' }
        Select-ComboChoice $window 'candidate-preview-appearance' $appearance
        Open-Page $window 'typing'
        Open-Page $window 'candidate'
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $sample = Find-ById $window 'candidate-preview-sample'
        } while (-not $sample -and [DateTime]::UtcNow -lt $deadline)
        if (-not $sample -or $sample.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne '杉树🌲 Alpha') { throw 'Leaving the candidate page lost the preview sample' }
        $combo = Find-ById $window 'candidate-preview-appearance'
        $selected = @($combo.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection())
        if ($selected.Count -ne 1 -or $selected[0].Current.Name -ne $appearance) { throw 'Leaving the candidate page lost the preview appearance' }
        Start-Sleep -Milliseconds 350
        Save-Window $window "$theme-candidate-custom-preview.png"
        $sample.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('   ')
        Select-ComboChoice $window 'candidate-preview-appearance' '跟随设置'
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $first = Find-ById $window 'candidate-preview-word-0'
        } while ((-not $first -or $first.Current.Name -ne '你们') -and [DateTime]::UtcNow -lt $deadline)
        if (-not $first -or $first.Current.Name -ne '你们') { throw 'Whitespace did not restore the default preview' }
        if ((Read-SettingsSnapshotText (Join-Path $state 'preferences.json')) -ne $preferencesBeforePreview) { throw 'Preview-only controls changed saved preferences' }
        # 真实开关必须同时改变持久化和预览宽度，再切回初始值，避免静态画面假通过。预览下面照 macOS 不再写尺寸，画出来的 DIP 尺寸在「预览」小字的帮助文本里。
        Open-Page $window 'toolbar'
        $size = Find-ById $window 'toolbar-preview-label'
        if (-not $size) { throw 'Toolbar preview label is missing' }
        $original = $size.Current.HelpText
        if (-not $original) { throw 'Toolbar preview size is missing' }
        foreach ($enabled in @($true, $false)) {
            $voice = Find-CheckBox (Find-ById $window 'settings-content') '语音输入'
            if (-not $voice) { throw 'Toolbar voice component is missing' }
            $voice.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 200
                $saved = (Read-SettingsSnapshotText (Join-Path $state 'preferences.json')) | ConvertFrom-Json
                $size = Find-ById $window 'toolbar-preview-label'
                $changed = $size -and ($size.Current.HelpText -ne $original)
            } while (($saved.preferences.floating_toolbar.voice -ne $enabled -or $changed -ne $enabled) -and [DateTime]::UtcNow -lt $deadline)
            if ($saved.preferences.floating_toolbar.voice -ne $enabled -or $changed -ne $enabled) { throw 'Toolbar component did not update preferences and preview together' }
            Start-Sleep -Milliseconds 350
            # 按钮照 macOS 排成纵向一列后，「语音输入」在 800 × 780 的窗口里落在视口下面，而勾选它写入偏好后整页重画、回到原来的滚动位置，滚进视口也留不住。同一组的复选框共用一个样式，所以核对一直在视口里、默认勾选的「中英文切换」的强调色。
            if ($enabled) { Assert-SelectedAccent (Find-CheckBox (Find-ById $window 'settings-content') '中英文切换') }
            Save-Window $window "$theme-toolbar-voice-$enabled.png"
        }
        # 固定操作仍要写入真实历史存储，只操作上方构造的合成记录。此时窗口是 800 宽，行窄，固定在「更多操作」菜单里；宽行直接点排出来的「固定」。
        Open-Page $window 'clip'
        Select-PageTab $window 'clip-tab-history' '历史记录'
        $pin = Find-ById $window 'clipboard-entry-0-pin'
        if (-not $pin) {
            $more = Find-ById $window 'clipboard-entry-0-actions'
            if (-not $more) { throw 'Clipboard actions are missing' }
            $more.SetFocus()
            $more.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 200
                $pin = Find-ById $window 'clipboard-entry-pin'
            } while (-not $pin -and [DateTime]::UtcNow -lt $deadline)
            if (-not $pin) { throw 'Clipboard action menu did not open' }
        }
        Save-Window $window "$theme-clipboard-actions.png"
        $pin.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $savedEntries = @((Read-SettingsSnapshotText (Join-Path $state 'clipboard_history.json')) | ConvertFrom-Json)
        } while (-not $savedEntries[0].pinned -and [DateTime]::UtcNow -lt $deadline)
        if (-not $savedEntries[0].pinned) { throw 'Clipboard pin action was not saved' }
        # 写盘完成不等于 UI 回调已呈现；确认固定标记与可用操作后再验收最终画面。
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        $textCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
        do {
            Start-Sleep -Milliseconds 200
            $more = Find-ById $window 'clipboard-entry-0-actions'
            if (-not $more) { $more = Find-ById $window 'clipboard-entry-0-pin' }
            $pinnedVisible = $false
            $texts = (Find-ById $window 'settings-content').FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)
            foreach ($text in $texts) {
                if ($text.Current.Name.StartsWith('已固定 · ') -and -not $text.Current.IsOffscreen) { $pinnedVisible = $true; break }
            }
        } while ((-not $pinnedVisible -or -not $more -or -not $more.Current.IsEnabled) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $pinnedVisible -or -not $more -or -not $more.Current.IsEnabled) { throw 'Clipboard pin result did not finish rendering' }
        Start-Sleep -Milliseconds 200
        Save-Window $window "$theme-clipboard-pinned.png"
        Select-PageTab $window 'clip-tab-clipboard' '历史记录上限'
        Assert-CloudClipboardRelaySettings $window
        Assert-CloudClipboardWebDavSettings $window
        Assert-DictionarySyncSettings
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(10000)) { throw 'Settings window failed to close' }
        if ($process.ExitCode -ne 0) { throw "Settings exited with $($process.ExitCode)" }
        $process = $null
    }
    # 连接助手要有运行时选项才画助手标签。另开一次窗口，经 MSIME_CLIENT_HOST_OPTIONS 交给它一份运行时选项，直接打开「连接助手」（mcp 路由）：照 macOS 的 McpConnectSection，打开时在 Claude Code，有安装和移除命令与两个权限开关，「其他」是 MCP 配置，复制按钮点后显示「已复制」、1.6 秒后就地换回原来的字。连接助手只用这份文件的路径和版本（没有 edition 键就是完整版，服务器名 msime），所以内容是空对象；文件不叫 runtime-options.json，不会被当成 Server 的配置改掉偏好目录。词库等页拿空对象发不出有效请求，这次只看连接助手。
    $hostOptions = Join-Path $state 'mcp-host-options.json'
    [IO.File]::WriteAllText($hostOptions, '{}')
    $env:MSIME_CLIENT_HOST_OPTIONS = $hostOptions
    try { $window = Start-SettingsWindow 'settings:mcp' } finally { $env:MSIME_CLIENT_HOST_OPTIONS = $oldHostOptions }
    # 800 宽时六个助手标签放不下，整排在横向滚动里，截图看得到。
    if (-not [SettingsWindowProbe]::SetWindowPos($process.MainWindowHandle, [IntPtr]::Zero, 0, 0, 800, 780, 0x0040)) { throw 'Window resize failed' }
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 200
        $content = Find-ById $window 'settings-content'
        $tab = if ($content) { Find-ById $content 'mcp-tab-claude_code' } else { $null }
    } while (-not $tab -and [DateTime]::UtcNow -lt $deadline)
    if (-not $tab) { throw 'MCP connect with runtime options does not show the assistant tabs' }
    if ($tab.Current.ItemStatus -ne '已选择') { throw 'MCP connect does not open on Claude Code' }
    foreach ($name in @('Claude Code 安装命令', 'Claude Code 移除命令', '允许修改设置', '允许读取词库')) {
        if (-not (Find-ByName $content $name)) { throw "MCP connect is missing $name" }
    }
    Start-Sleep -Milliseconds 350
    Save-Window $window 'ai-connection-claude-code.png'
    # 复制（会改写系统剪贴板）后只换这个按钮的字，不重画页面：按钮按弱引用记着，计时结束时取回来就地复原。等到「已复制」出现，再等它换回「复制命令」。
    $copy = Find-ById $content 'mcp-copy-claude_code'
    if (-not $copy -or $copy.Current.Name -ne '复制命令') { throw 'MCP connect is missing the Claude Code copy button' }
    $copy.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(1.5)
    do {
        $copy = Find-ById $window 'mcp-copy-claude_code'
        if ($copy -and $copy.Current.Name -eq '已复制') { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $copy -or $copy.Current.Name -ne '已复制') { throw 'MCP copy button did not show 已复制' }
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    do {
        Start-Sleep -Milliseconds 200
        $copy = Find-ById $window 'mcp-copy-claude_code'
    } while ((-not $copy -or $copy.Current.Name -ne '复制命令') -and [DateTime]::UtcNow -lt $deadline)
    if (-not $copy -or $copy.Current.Name -ne '复制命令') { throw 'MCP copy button did not return to 复制命令' }
    Select-PageTab $window 'mcp-tab-json' 'MCP 配置'
    Start-Sleep -Milliseconds 350
    Save-Window $window 'ai-connection-json.png'
    $process.CloseMainWindow() | Out-Null
    if (-not $process.WaitForExit(10000)) { throw 'Settings window failed to close' }
    if ($process.ExitCode -ne 0) { throw "Settings exited with $($process.ExitCode)" }
    $process = $null
} catch {
    # 失败时保留合成窗口，便于区分产品布局问题与 UIA 时序问题。
    if ($process -and -not $process.HasExited) {
        try {
            Save-Window ([System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)) 'failure.png'
        } catch { Write-Warning 'Could not capture the failing synthetic window' }
    }
    throw
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    [SettingsWindowProbe]::SetDllDirectory($null) | Out-Null
    $env:MSIME_CLIENT_STATE_DIR = $oldState
    Remove-Item -LiteralPath $state -Recurse -Force -ErrorAction SilentlyContinue
}

exit 0
