# 仅操作 settings_ui.ps1 创建的合成窗口和合成码表，不读取用户词库或联网下载。
function Assert-SchemeResources($window, [string]$fixture) {
    function Wait-SchemeResource([string]$stateName, [bool]$download) {
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        do {
            Start-Sleep -Milliseconds 150
            # Border 不保证有 UIA 节点，直接读取有可访问性名称的状态文字和独立动作。
            $status = Find-ById $window 'resource-pack-wubi-state'
            $stateMatches = $status -and $status.Current.Name -eq $stateName
            $button = Find-ById $window 'sentence-model-download-wubi'
        } while ((-not $stateMatches -or [bool]$button -ne $download) -and [DateTime]::UtcNow -lt $deadline)
        if (-not $stateMatches -or [bool]$button -ne $download) { throw "Wubi resource state did not become $stateName (actual=$($status.Current.Name), download=$([bool]$button))" }
        if ($download -and -not (Find-ById $window 'sentence-model-import-wubi')) { throw 'Missing Wubi offline import action' }
    }

    Wait-SchemeResource '随安装包提供。' $false
    Save-Window $window 'scheme-wubi-bundled.png'
    $backup = $fixture + '.synthetic-backup'
    Move-Item -LiteralPath $fixture -Destination $backup
    try {
        # 重新进页必须重新查询文件状态，不能让已内置的旧结果覆盖瘦包状态。
        Open-Page $window 'expression'
        Open-Page $window 'typing'
        Select-PageTab $window 'input-tab-scheme' '中文方案'
        Wait-SchemeResource '词库尚未安装，下载后切换一次输入框生效。' $true
        Save-Window $window 'scheme-wubi-missing.png'
    } finally {
        Move-Item -LiteralPath $backup -Destination $fixture
        Open-Page $window 'expression'
        Open-Page $window 'typing'
        Select-PageTab $window 'input-tab-scheme' '中文方案'
    }
    Wait-SchemeResource '随安装包提供。' $false
    Write-Host 'Wubi bundled/missing/restored states and install actions passed in the native settings window'
}
