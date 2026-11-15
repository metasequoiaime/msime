# 由 settings_ui.ps1 在隔离合成设置目录里调用，验证省略字段后的真实控件回读。
function Assert-QuickPhraseSettings($window) {
    function Find-QuickPhraseToggle($window) {
        $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, '快捷短语混输')
        foreach ($node in $window.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)) {
            $pattern = $null
            if ($node.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) { return $node }
        }
        return $null
    }
    function Show-QuickPhraseToggle($window) {
        Select-PageTab $window 'input-tab-candidates' '云候选来源'
        $content = Find-ById $window 'settings-content'
        $more = Find-ByName $content '更多选项'
        if (-not $more) { throw '快捷短语混输缺少更多选项入口' }
        if ($more.Current.ItemStatus -ne '已展开') {
            $more.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        }
        return Wait-PluginEditorElement { Find-QuickPhraseToggle $window } '快捷短语混输开关'
    }
    function Wait-QuickPhraseSaved([bool]$expected) {
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        do {
            $saved = (Read-SettingsSnapshotText (Join-Path $state 'preferences.json')) | ConvertFrom-Json
            $field = $saved.preferences.mixed_input.PSObject.Properties['quick_phrase']
            if (($expected -and -not $field) -or (-not $expected -and $field -and $field.Value -eq $false)) { return }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $deadline)
        throw "快捷短语混输没有按共享序列化保存：$expected"
    }
    $toggle = Show-QuickPhraseToggle $window
    $pattern = $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    if ($pattern.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::Off) { throw '新装快捷短语混输应关闭' }
    $pattern.Toggle()
    Wait-QuickPhraseSaved $true
    $script:process.CloseMainWindow() | Out-Null
    if (-not $script:process.WaitForExit(10000)) { throw '快捷短语保存后窗口未退出' }
    $window = Start-SettingsWindow 'settings:input'
    $toggle = Show-QuickPhraseToggle $window
    $pattern = $toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    if ($pattern.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::On) { throw '省略开启字段后重新启动丢失快捷短语开关状态' }
    $pattern.Toggle()
    Wait-QuickPhraseSaved $false
    Select-PageTab $window 'input-tab-scheme' '中文方案'
    Select-PageTab $window 'input-tab-candidates' '云候选来源'
    $toggle = Show-QuickPhraseToggle $window
    if ($toggle.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Current.ToggleState -ne [System.Windows.Automation.ToggleState]::Off) { throw '关闭快捷短语混输后回读失败' }
    Select-PageTab $window 'input-tab-scheme' '中文方案'
    Write-Host '快捷短语混输的新装关闭、开启后字段省略、重启回读和关闭回读通过'
    return $window
}
