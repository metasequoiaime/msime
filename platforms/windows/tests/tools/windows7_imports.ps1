[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# 模拟 dumpbin 的文本边界，验证固定序号可解析，未知序号与新系统静态接口仍会被拒绝。
$audit = Join-Path $PSScriptRoot '../../Test-Windows7CoreImports.ps1'
function Check-Import {
    param([string]$Library, [string]$Import, [string]$Expected = '', [switch]$Delayed)
    $importLines = @(
        $(if ($Delayed) { 'Section contains the following delay load imports:' } else { 'Section contains the following imports:' }),
        ('    ' + $Library), $Import, '  Summary', '        1000 _RDATA'
    )
    Set-Item function:dumpbin -Value {
        $global:LASTEXITCODE = 0
        return $importLines
    }.GetNewClosure()
    if ($Expected -eq '') {
        $rejected = $false
        try { & $audit -LiteralPath 'synthetic.dll' | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw "Unexpectedly accepted: $Library $Import" }
    } else {
        $result = & $audit -LiteralPath 'synthetic.dll'
        if ($result.Imports.Count -ne 1 -or $result.Imports[0].Name -cne $Expected) {
            throw "Unexpected import mapping: $Library $Import"
        }
    }
}
Check-Import 'WS2_32.dll' '                             Ordinal   115' 'WSAStartup'
Check-Import 'ws2_32.dll' '                             Ordinal   116' 'WSACleanup'
Check-Import 'WS2_32.dll' '                             Ordinal   117'
Check-Import 'unverified.dll' '                             Ordinal   115'
Check-Import 'WS2_32.dll' '                             Ordinal   115' -Delayed
Check-Import 'kernel32.dll' '                            A6 GetDpiForWindow'
Check-Import 'dxgi.dll' '                             5 CreateDXGIFactory2'
Check-Import 'user32.dll' '                            A6 LogicalToPhysicalPointForPerMonitorDPI'
Check-Import 'uiautomationcore.dll' '                            A6 UiaDisconnectProvider'
Check-Import 'winhttp.dll' '                            A6 WinHttpWebSocketReceive'
Check-Import 'kernel32.dll' '                            A6 GetTickCount64' 'GetTickCount64'
forEach ($entry in @(@(2,'SysAllocString'),@(4,'SysAllocStringLen'),@(6,'SysFreeString'),@(7,'SysStringLen'),@(8,'VariantInit'),@(9,'VariantClear'),@(16,'SafeArrayDestroy'),@(26,'SafeArrayPutElement'),@(411,'SafeArrayCreateVector'))) {
    Check-Import 'oleaut32.dll' ('                             Ordinal   ' + $entry[0]) $entry[1]
}
Check-Import 'oleaut32.dll' '                             Ordinal   999'
Check-Import 'oleaut32.dll' '                             Ordinal   6' -Delayed
foreach ($entry in @(@(410,'SetWindowSubclass'),@(412,'RemoveWindowSubclass'),@(413,'DefSubclassProc'))) {
    Check-Import 'comctl32.dll' ('                             Ordinal   ' + $entry[0]) $entry[1]
}
Check-Import 'comctl32.dll' '                             Ordinal   411'
Check-Import 'comctl32.dll' '                             Ordinal   410' -Delayed
Check-Import 'd2d1.dll' '                             Ordinal   1' 'D2D1CreateFactory'
Check-Import 'shlwapi.dll' '                             Ordinal   12' 'SHCreateMemStream'
Check-Import 'd2d1.dll' '                             Ordinal   7'
Check-Import 'shlwapi.dll' '                             Ordinal   999'
Check-Import 'onnxruntime.dll' '                             Ordinal   1'
Write-Output 'Windows 7 导入检查：固定 Winsock 序号、未知序号与系统接口边界通过。'

# 纯 CRT 转发只接受相邻 ucrtbase 的真实具名导出，不通过文件名或零导入跳过依赖核验。
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('msime-crt-audit-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    $runtime = Join-Path $fixture 'ucrtbase.dll'
    [IO.File]::WriteAllText($runtime, 'synthetic')
    function Check-Forward {
        param([string[]]$Entry, [bool]$Accepted, [int]$Count = 1, [string]$File = 'api-ms-win-crt-runtime-l1-1-0.dll')
        $exportLines = @("         $Count number of functions") + $Entry
        Set-Item function:dumpbin -Value {
            $global:LASTEXITCODE = 0
            if ($args -contains '/imports') { return @('File Type: DLL') }
            if ([IO.Path]::GetFileName($args[-1]) -in 'ucrtbase.dll','kernel32.dll') { return @('          1    0 00001000 _Exit') }
            return $exportLines
        }.GetNewClosure()
        $passed = $false
        try {
            $result = & $audit -LiteralPath $File -DependencyDirectory $fixture -RequiredExports @('_Exit')
            $passed = $true
            if ($Accepted -and ($result.Imports.Count -ne 1 -or $result.Imports[0].Library -cne $(if ($File -like 'api-ms-win-core-*') { 'kernel32.dll' } else { 'ucrtbase.dll' }) -or $result.Imports[0].Name -cne '_Exit')) { throw 'Unexpected CRT forwarding mapping' }
        } catch { if ($Accepted) { throw } }
        if ($passed -ne $Accepted) { throw "Unexpected CRT forwarding acceptance: $Entry" }
    }
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase._Exit)' $true
    Check-Forward @('          1    0          _Exit (forwarded to ucrtbase._Exit)', '          2    1          unused (forwarded to ucrtbase.missing)') $true 2
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase._exit)' $false
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase.missing)' $false
    Check-Forward '          1    0          _Exit (forwarded to kernelbase._Exit)' $false
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase.#1)' $false
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase._Exit)' $false 2
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase._Exit)' $false 1 'unknown.dll'
    Check-Forward '          1    0          _Exit (forwarded to kernel32._Exit)' $true 1 'api-ms-win-core-file-l1-1-0.dll'
    Check-Forward '          1    0          _Exit (forwarded to kernelbase._Exit)' $false 1 'api-ms-win-core-file-l1-1-0.dll'
    Check-Forward '          1    0          _Exit (forwarded to kernel32.missing)' $false 1 'api-ms-win-core-file-l1-1-0.dll'
    Remove-Item -LiteralPath $runtime
    Check-Forward '          1    0          _Exit (forwarded to ucrtbase._Exit)' $false
} finally {
    Remove-Item -LiteralPath $fixture -Recurse -Force
}
Write-Output 'Windows 7 CRT forwarding: exact adjacent exports, unknown targets, ordinal targets, partial parsing and missing runtime passed.'
