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
$pages = @('typing', 'lexicon', 'shortcuts', 'expression', 'voice', 'osk', 'themes', 'candidate', 'toolbar', 'clip', 'ai', 'plugins', 'stats', 'account', 'dev', 'about')
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
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        Start-Sleep -Milliseconds 200
        $content = Find-ById $window 'settings-content'
        $title = if ($content) { Find-ById $content "settings-page-$page" } else { $null }
    } while (-not $title -and [DateTime]::UtcNow -lt $deadline)
    if (-not $title) { throw "Page did not change to $page" }
    if (Find-ById $content 'settings-load-error') { throw 'Synthetic preferences were rejected by the real host' }
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
    $beforeEdit = [IO.File]::ReadAllText($path)
    # 交换键位的中间状态会重复；只有按保存时才能成对进入共享偏好库。
    $secondValue.SetValue("'")
    $thirdValue.SetValue(';')
    if ([IO.File]::ReadAllText($path) -ne $beforeEdit) { throw 'Editing Wubi shortcuts saved an intermediate pair' }
    $save = Wait-WubiControl $window 'wubi-save-candidate-keys'
    $save.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $saved = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
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
        $saved = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        $keys = $saved.preferences.wubi_candidate_selection
    } while ($keys.enabled -and [DateTime]::UtcNow -lt $deadline)
    if ($keys.enabled -or $keys.second_key -ne "'" -or $keys.third_key -ne ';') { throw 'Disabling Wubi shortcuts lost custom keys' }
    Write-Host 'Wubi shortcut toggle, atomic swap, reload and retained keys passed'
}
function Set-CandidateMore($window, [bool]$expanded) {
    # 候选窗口页是一列，进阶各行收在「更多选项」里。页面重绘后 UIA 树可能稍晚恢复；等待目标而非固定延时。
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $fold = Find-ById $window 'candidate-more'
        if ($fold -and $fold.Current.IsEnabled) { break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $fold) { throw 'Missing candidate 更多选项 fold' }
    $pattern = $fold.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
    $wanted = if ($expanded) { [System.Windows.Automation.ExpandCollapseState]::Expanded } else { [System.Windows.Automation.ExpandCollapseState]::Collapsed }
    if ($pattern.Current.ExpandCollapseState -ne $wanted) {
        if ($expanded) { $pattern.Expand() } else { $pattern.Collapse() }
    }
    # 主流程的行一直在；折叠区里的行只在展开时出现在 UIA 树里。
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 200
        $content = Find-ById $window 'settings-content'
        $inside = Find-ByName $content '行内预编辑'
    } while (([bool]$inside -ne $expanded) -and [DateTime]::UtcNow -lt $deadline)
    if ([bool]$inside -ne $expanded) { throw "Candidate 更多选项 did not change to expanded=$expanded" }
    if (-not (Find-ByName $content '候选项排列方式')) { throw 'Candidate main rows are missing' }
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
    $beforeEdit = [IO.File]::ReadAllText($path)
    $scroll = (Find-ById $window 'settings-content').GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
    if ($scroll.Current.VerticallyScrollable) { $scroll.SetScrollPercent(-1, 100) }
    Select-ComboChoice $window 'cloud-clipboard-source' '自建中继'
    $url = Wait-RelayControl $window 'cloud-clipboard-relay-url'
    $token = Wait-RelayControl $window 'cloud-clipboard-relay-token'
    if (-not $token.Current.IsPassword) { throw 'Relay token is not masked' }
    $url.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('http://relay.example.test')
    $token.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('synthetic-relay-token')
    if ([IO.File]::ReadAllText($path) -ne $beforeEdit) { throw 'Relay draft was persisted before saving' }
    (Wait-RelayControl $window 'cloud-clipboard-save-source').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    Wait-RelayControl $window 'settings-notice' | Out-Null
    if ([IO.File]::ReadAllText($path) -ne $beforeEdit) { throw 'Invalid HTTP relay passed Rust preference validation' }
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
        $saved = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
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
        $saved = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        $relay = Read-RelayPreference $saved
    } while ($relay -and [DateTime]::UtcNow -lt $deadline)
    if ($relay) { throw 'Switching to account retained relay credentials' }
    Write-Host 'Cloud clipboard relay masking, draft, Rust validation, atomic save, reload and account switch passed'
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
                if ($page -eq 'osk') {
                    # 完整版提供手写：「手写输入」并进了这一页；「恢复默认」的说明只提这一页看得到的几项，不再提工具栏按钮。
                    foreach ($name in @('打开手写识别板', '高度、间距和顶部语音入口回到默认')) {
                        if (-not (Find-ByName $content $name)) { throw "Screen keyboard page is missing: $name" }
                    }
                }
                if ($page -eq 'about') {
                    # 反馈并进关于后仍须能生成报告；不能只删除旧导航的验收。
                    foreach ($name in @('反馈类型', '反馈描述', '复制报告', '查看 Issues')) {
                        if (-not (Find-ByName $content $name)) { throw "About page is missing merged feedback content: $name" }
                    }
                    if (Find-ById $window 'feedback') { throw 'Merged feedback page is still in the sidebar' }
                }
                if ($page -eq 'toolbar') {
                    # 缩放和图标尺寸合成一个「工具栏大小」。
                    if (-not (Find-ByName $content '工具栏大小')) { throw 'Toolbar size select is missing' }
                    foreach ($name in @('工具栏缩放', '图标尺寸')) {
                        if (Find-ByName $content $name) { throw "Toolbar page still offers $name apart" }
                    }
                }
                Save-Window $window "$theme-$width-$page.png"
                Write-Host "$theme $width $page rendered"
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
                    }
                }
                if ($page -eq 'candidate') {
                    # 整页截过一张收起的，再展开「更多选项」截一张，然后收起，下一个宽度照样从收起开始。
                    Set-CandidateMore $window $true
                    Start-Sleep -Milliseconds 350
                    Save-Window $window "$theme-$width-candidate-more.png"
                    Set-CandidateMore $window $false
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
        }
        # 搜索必须让目标可见：主字体在预设下本来收起，搜索落到它时要画出来；折叠区里的行要把「更多选项」展开。
        Open-Page $window 'candidate'
        Set-CandidateMore $window $false
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
            $font = Find-ByName (Find-ById $window 'settings-content') '主字体'
        } while (-not $font -and [DateTime]::UtcNow -lt $deadline)
        if (-not $font) { throw 'Font search target is missing' }
        Start-Sleep -Milliseconds 350
        Save-Window $window "$theme-search-candidate-font.png"
        $input.SetFocus()
        $input.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue('行内预编辑')
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            Start-Sleep -Milliseconds 200
            $inline = Find-ByName (Find-ById $window 'settings-content') '行内预编辑'
        } while (-not $inline -and [DateTime]::UtcNow -lt $deadline)
        if (-not $inline) { throw 'Search did not open the 更多选项 fold holding its target' }
        Set-CandidateMore $window $false
        # 与 macOS 一样，示例与预览明暗在离开再回到本页后保留，但不写入真实设置。
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
        Assert-CloudClipboardRelaySettings $window
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
