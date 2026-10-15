# 用独立合成设置运行真实 WinUI 宿主，检查全部页面并保存浅深色与窄窗口截图。
param(
    [Parameter(Mandatory = $true)][string]$Program,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
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
}
'@
$programPath = (Resolve-Path -LiteralPath $Program).Path
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
$state = Join-Path ([IO.Path]::GetTempPath()) ('msime-settings-ui-' + [Guid]::NewGuid())
$oldState = $env:MSIME_CLIENT_STATE_DIR
$pages = @('typing', 'expression', 'shortcuts', 'lexicon', 'themes', 'candidate', 'toolbar', 'osk', 'voice', 'hand', 'clip', 'stats', 'plugins', 'ai', 'account', 'dev', 'feedback', 'about')
$process = $null
function Find-ById($parent, [string]$id) {
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $id)
    return $parent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}
function Find-ByName($parent, [string]$name) {
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
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
function Open-Page($window, [string]$page) {
    $item = Find-ById $window $page
    if (-not $item) { throw "Missing native page: $page" }
    $item.GetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern).ScrollIntoView()
    $item = Find-ById $window $page
    $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 200
        $content = Find-ById $window 'settings-content'
        $title = if ($content) { Find-ById $content "settings-page-$page" } else { $null }
    } while (-not $title -and [DateTime]::UtcNow -lt $deadline)
    if (-not $title) { throw "Page did not change to $page" }
    if (Find-ById $content 'settings-load-error') { throw 'Synthetic preferences were rejected by the real host' }
}
function Select-CandidateTab($window, [string]$tab) {
    # 页面重绘和弹出层关闭后，UIA 树可能稍晚恢复；等待目标而非固定延时。
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $button = Find-ById $window "candidate-tab-$tab"
        if ($button -and $button.Current.IsEnabled) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $button -or -not $button.Current.IsEnabled) { throw "Missing or disabled candidate tab: $tab" }
    $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $button = Find-ById $window "candidate-tab-$tab"
    } while ((-not $button -or $button.Current.ItemStatus -ne '已选择') -and [DateTime]::UtcNow -lt $deadline)
    if (-not $button -or $button.Current.ItemStatus -ne '已选择') { throw "Candidate tab did not change: $tab" }
    $label = @{layout = '候选项排列方式'; font = '主字体'; style = '不透明度'; preedit = '行内预编辑'}[$tab]
    if (-not (Find-ByName (Find-ById $window 'settings-content') $label)) { throw "Candidate tab content is missing: $tab" }
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
    if (-not [SettingsWindowProbe]::SetDllDirectory([IO.Path]::GetDirectoryName($programPath))) { throw 'Settings DLL path could not be set' }
    $defaults = [SettingsWindowProbe]::DefaultPreferences() | ConvertFrom-Json
    if (-not $defaults.ok) { throw 'Default preferences could not be loaded' }
    foreach ($theme in @('light', 'dark')) {
        $preferences = $defaults.value
        $preferences.settings_theme = $theme
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
        $process = Start-Process -FilePath $programPath -ArgumentList '--route=settings:input' -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds(60)
        do {
            $process.Refresh()
            if ($process.HasExited) { throw "Settings exited before opening: $($process.ExitCode)" }
            if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 200
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Settings window did not open in 60 seconds' }
        $window = [System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
        # 检查窗口真正交给系统的拖动尺寸边界，不能只检查 XAML 内部的最小宽度。
        $limits = [SettingsWindowProbe]::TrackLimits($process.MainWindowHandle)
        if ($limits.MinTrackSize.X -lt 800 -or $limits.MinTrackSize.Y -lt 560) { throw 'Native window minimum size was not applied' }
        foreach ($width in @(1000, 800)) {
            if (-not [SettingsWindowProbe]::SetWindowPos($process.MainWindowHandle, [IntPtr]::Zero, 0, 0, $width, 760, 0x0040)) { throw 'Window resize failed' }
            foreach ($page in $pages) {
                if ($process.HasExited) { throw "Settings exited on $page" }
                # 复用导航等待，既验证正文身份，也处理侧栏虚拟化。
                Open-Page $window $page
                $content = Find-ById $window 'settings-content'
                if ($content.Current.BoundingRectangle.Width -lt 480) { throw "No usable content column on $page" }
                # 再等一轮呈现，避免 UIA 树已经更新而屏幕仍是上一页的画面。
                Start-Sleep -Milliseconds 500
                if ($page -eq 'clip') {
                    $deadline = [DateTime]::UtcNow.AddSeconds(10)
                    do {
                        $preview = Find-ById $window 'clipboard-entry-0-preview'
                        if ($preview) { break }
                        Start-Sleep -Milliseconds 200
                    } while ([DateTime]::UtcNow -lt $deadline)
                    if (-not $preview) { throw 'Synthetic clipboard history was not rendered' }
                    $more = Find-ById $window 'clipboard-entry-0-actions'
                    if (-not $preview -or -not $more) { throw 'Clipboard preview or actions are missing' }
                    $textBounds = $preview.Current.BoundingRectangle
                    $actionBounds = $more.Current.BoundingRectangle
                    if ($textBounds.Width -lt 180 -or $textBounds.Right -gt $actionBounds.Left) { throw 'Clipboard actions crowded out the text preview' }
                    if ($actionBounds.Right -gt $content.Current.BoundingRectangle.Right) { throw 'Clipboard actions overflow the content column' }
                }
                Save-Window $window "$theme-$width-$page.png"
                Write-Host "$theme $width $page rendered"
                if ($page -eq 'candidate') {
                    foreach ($tab in @('layout', 'font', 'style', 'preedit')) {
                        Select-CandidateTab $window $tab
                        Start-Sleep -Milliseconds 350
                        Save-Window $window "$theme-$width-candidate-tab-$tab.png"
                    }
                    Select-CandidateTab $window 'layout'
                }
            }
        }
        # 通过真实单选控件切换方案，再读合成偏好确认保存成功，避免只验证静态页面。
        Open-Page $window 'typing'
        Start-Sleep -Milliseconds 700
        foreach ($scheme in @('shuangpin', 'wubi', 'quanpin')) {
            $radio = Find-ById $window "scheme-$scheme"
            if (-not $radio -or -not $radio.Current.IsEnabled) { throw "Scheme option is unavailable: $scheme" }
            $radio.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 200
                $saved = Get-Content -LiteralPath (Join-Path $state 'preferences.json') -Raw | ConvertFrom-Json
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
        }
        # 搜索必须打开所属页内标签，不能只打开候选窗口后留下不可见的目标。
        Open-Page $window 'candidate'
        Select-CandidateTab $window 'layout'
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
            $font = Find-ById $window 'candidate-tab-font'
        } while ((-not $font -or $font.Current.ItemStatus -ne '已选择') -and [DateTime]::UtcNow -lt $deadline)
        if (-not $font -or $font.Current.ItemStatus -ne '已选择') { throw 'Font search did not select the font tab' }
        if (-not (Find-ByName (Find-ById $window 'settings-content') '主字体')) { throw 'Font search target is missing' }
        Start-Sleep -Milliseconds 350
        Save-Window $window "$theme-search-candidate-font.png"
        # 与 macOS 一样，示例与预览明暗在切标签后保留，但不写入真实设置。
        $preferencesBeforePreview = [IO.File]::ReadAllText((Join-Path $state 'preferences.json'))
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
        Select-CandidateTab $window 'layout'
        $sample = Find-ById $window 'candidate-preview-sample'
        if ($sample.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value -ne '杉树🌲 Alpha') { throw 'Candidate tab switch lost the preview sample' }
        $combo = Find-ById $window 'candidate-preview-appearance'
        $selected = @($combo.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection())
        if ($selected.Count -ne 1 -or $selected[0].Current.Name -ne $appearance) { throw 'Candidate tab switch lost the preview appearance' }
        Start-Sleep -Milliseconds 350
        Save-Window $window "$theme-candidate-custom-preview.png"
        $sample.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('   ')
        Select-ComboChoice $window 'candidate-preview-appearance' '跟随设置'
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $first = Find-ById $window 'candidate-preview-word-0'
        } while ((-not $first -or $first.Current.Name -ne '候选项') -and [DateTime]::UtcNow -lt $deadline)
        if (-not $first -or $first.Current.Name -ne '候选项') { throw 'Whitespace did not restore the default preview' }
        if ([IO.File]::ReadAllText((Join-Path $state 'preferences.json')) -ne $preferencesBeforePreview) { throw 'Preview-only controls changed saved preferences' }
        # 真实开关必须同时改变持久化和预览宽度，再切回初始值，避免静态画面假通过。
        Open-Page $window 'toolbar'
        $size = Find-ById $window 'toolbar-preview-size'
        if (-not $size) { throw 'Toolbar preview size is missing' }
        $original = $size.Current.Name
        foreach ($enabled in @($true, $false)) {
            $voice = Find-CheckBox (Find-ById $window 'settings-content') '语音输入'
            if (-not $voice) { throw 'Toolbar voice component is missing' }
            $voice.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            do {
                Start-Sleep -Milliseconds 200
                $saved = Get-Content -LiteralPath (Join-Path $state 'preferences.json') -Raw | ConvertFrom-Json
                $size = Find-ById $window 'toolbar-preview-size'
                $changed = $size -and ($size.Current.Name -ne $original)
            } while (($saved.preferences.floating_toolbar.voice -ne $enabled -or $changed -ne $enabled) -and [DateTime]::UtcNow -lt $deadline)
            if ($saved.preferences.floating_toolbar.voice -ne $enabled -or $changed -ne $enabled) { throw 'Toolbar component did not update preferences and preview together' }
            Start-Sleep -Milliseconds 350
            if ($enabled) { Assert-SelectedAccent (Find-CheckBox (Find-ById $window 'settings-content') '语音输入') }
            Save-Window $window "$theme-toolbar-voice-$enabled.png"
        }
        # 菜单中的固定操作仍要写入真实历史存储，只操作上方构造的合成记录。
        Open-Page $window 'clip'
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
        Save-Window $window "$theme-clipboard-actions.png"
        $pin.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $savedEntries = @(Get-Content -LiteralPath (Join-Path $state 'clipboard_history.json') -Raw | ConvertFrom-Json)
        } while (-not $savedEntries[0].pinned -and [DateTime]::UtcNow -lt $deadline)
        if (-not $savedEntries[0].pinned) { throw 'Clipboard pin action was not saved' }
        # 写盘完成不等于 UI 回调已呈现；确认固定标记与可用操作后再验收最终画面。
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        $textCondition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)
        do {
            Start-Sleep -Milliseconds 200
            $more = Find-ById $window 'clipboard-entry-0-actions'
            $pinnedVisible = $false
            $texts = (Find-ById $window 'settings-content').FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)
            foreach ($text in $texts) {
                if ($text.Current.Name.StartsWith('已固定 · ') -and -not $text.Current.IsOffscreen) { $pinnedVisible = $true; break }
            }
        } while ((-not $pinnedVisible -or -not $more -or -not $more.Current.IsEnabled) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $pinnedVisible -or -not $more -or -not $more.Current.IsEnabled) { throw 'Clipboard pin result did not finish rendering' }
        Start-Sleep -Milliseconds 200
        Save-Window $window "$theme-clipboard-pinned.png"
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(10000)) { throw 'Settings window failed to close' }
        if ($process.ExitCode -ne 0) { throw "Settings exited with $($process.ExitCode)" }
        $process = $null
    }
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
