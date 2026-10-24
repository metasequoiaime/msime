[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$LiteralPath,
    [string]$DependencyDirectory = '',
    [string[]]$RequiredExports = @()
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# 使用 MSVC 自带解析器同时列出普通与延迟导入，不执行待检查的 DLL。
$lines = @(& dumpbin /nologo /imports $LiteralPath)
if ($LASTEXITCODE -ne 0) { throw "Cannot read PE imports: $LiteralPath" }
$delayed = $false
$library = ''
$imports = [Collections.Generic.List[object]]::new()
$libraries = [Collections.Generic.List[object]]::new()
foreach ($line in $lines) {
    if ($line -match '^\s+Summary\s*$') { break }
    if ($line -match 'Section contains the following delay load imports:') {
        $delayed = $true
        $library = ''
    } elseif ($line -match '^    ([A-Za-z0-9_.-]+\.dll)\s*$') {
        $library = $Matches[1].ToLowerInvariant()
        $libraries.Add([pscustomobject]@{ Name = $library; Delayed = $delayed })
    } elseif ($library -ne '') {
        $pattern = if ($delayed) { '^\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]+\s+([A-Za-z_?][A-Za-z0-9_?@$]*)\s*$' }
                   else { '^\s+[0-9A-Fa-f]+\s+([A-Za-z_?][A-Za-z0-9_?@$]*)\s*$' }
        if ($line -match $pattern) {
            $imports.Add([pscustomobject]@{ Library = $library; Name = $Matches[1]; Delayed = $delayed; Ordinal = $null })
        } elseif ($line -match '^\s+Ordinal\s+([0-9]+)\s*$') {
            $ordinal = [int]$Matches[1]
            # Windows SDK 的 Winsock 导入库保留固定序号；只接受产品实际使用的 Win7 网络接口。
            $winsockOrdinals = @{ 1='accept'; 2='bind'; 3='closesocket'; 4='connect'; 5='getpeername';
                6='getsockname'; 7='getsockopt'; 8='htonl'; 9='htons'; 10='ioctlsocket'; 13='listen';
                15='ntohs'; 16='recv'; 17='recvfrom'; 18='select'; 19='send'; 20='sendto';
                21='setsockopt'; 23='socket'; 57='gethostname'; 111='WSAGetLastError'; 112='WSASetLastError';
                115='WSAStartup'; 116='WSACleanup'; 151='__WSAFDIsSet' }
            if ($library -eq 'ws2_32.dll' -and -not $delayed -and $winsockOrdinals.ContainsKey($ordinal)) {
                $imports.Add([pscustomobject]@{ Library = $library; Name = $winsockOrdinals[$ordinal]; Delayed = $false; Ordinal = $ordinal })
                continue
            }
            # SDK 的 OLE Automation 导入库同样使用固定序号；只映射宿主实际使用的历史 C ABI。
            $automationOrdinals = @{ 2 = 'SysAllocString'; 4 = 'SysAllocStringLen'; 6 = 'SysFreeString';
                7 = 'SysStringLen'; 8 = 'VariantInit'; 9 = 'VariantClear'; 16 = 'SafeArrayDestroy';
                26 = 'SafeArrayPutElement'; 200 = 'GetErrorInfo'; 201 = 'SetErrorInfo'; 202 = 'CreateErrorInfo'; 411 = 'SafeArrayCreateVector' }
            if ($library -eq 'oleaut32.dll' -and -not $delayed -and $automationOrdinals.ContainsKey($ordinal)) {
                $imports.Add([pscustomobject]@{ Library = $library; Name = $automationOrdinals[$ordinal]; Delayed = $false; Ordinal = $ordinal })
                continue
            }
            # 在 Win7 SP1 的 x64 与 x86 上核对过具名导出和序号地址，只接受本产品实际使用的子类化接口。
            $controlsOrdinals = @{ 410 = 'SetWindowSubclass'; 412 = 'RemoveWindowSubclass'; 413 = 'DefSubclassProc' }
            if ($library -eq 'comctl32.dll' -and -not $delayed -and $controlsOrdinals.ContainsKey($ordinal)) {
                $imports.Add([pscustomobject]@{ Library = $library; Name = $controlsOrdinals[$ordinal]; Delayed = $false; Ordinal = $ordinal })
                continue
            }
            $graphicsOrdinal = $library -eq 'd2d1.dll' -and $ordinal -eq 1
            $streamOrdinal = $library -eq 'shlwapi.dll' -and $ordinal -eq 12
            if (-not $delayed -and ($graphicsOrdinal -or $streamOrdinal)) {
                $name = if ($graphicsOrdinal) { 'D2D1CreateFactory' } else { 'SHCreateMemStream' }
                $imports.Add([pscustomobject]@{ Library = $library; Name = $name; Delayed = $false; Ordinal = $ordinal })
                continue
            }
            # sherpa 的上游导入库以序号链接这个 C ABI；必须核对实际相邻 DLL 的导出，不能略过。
            if ($library -ne 'onnxruntime.dll' -or $ordinal -ne 1 -or $delayed -or $DependencyDirectory -eq '') {
                throw "Unverified ordinal import: $library!$ordinal"
            }
            $dependency = Join-Path $DependencyDirectory $library
            if (-not (Test-Path -LiteralPath $dependency -PathType Leaf)) { throw "Missing ordinal dependency: $library" }
            $exports = @(& dumpbin /nologo /exports $dependency)
            if ($LASTEXITCODE -ne 0 -or -not ($exports -match '^\s+1\s+[0-9A-Fa-f]+\s+[0-9A-Fa-f]+\s+OrtGetApiBase\s*$')) {
                throw "Unexpected ONNX ordinal 1 export: $dependency"
            }
            $imports.Add([pscustomobject]@{ Library = $library; Name = 'OrtGetApiBase'; Delayed = $false; Ordinal = $ordinal })
        }
    }
}
if ($imports.Count -eq 0) {
    # API 桥有完整转发表，但旧系统不必提供未被引用的新接口；只沿实际入边解析所需导出。
    $fileName = [IO.Path]::GetFileName($LiteralPath)
    if ($fileName -notmatch '^api-ms-win-(crt|core)-[a-z]+-l[0-9]+-[0-9]+-[0-9]+\.dll$' -or
        $DependencyDirectory -eq '' -or -not [IO.Path]::IsPathRooted($DependencyDirectory)) {
        throw "PE has no recognizable imports: $LiteralPath"
    }
    $crt = $fileName -like 'api-ms-win-crt-*'
    $exports = @(& dumpbin /nologo /exports $LiteralPath)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read API forwarding exports' }
    $functionCount = 0
    $forwarders = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach ($line in $exports) {
        if ($line -match '^\s+([0-9]+) number of functions\s*$') {
            if ($functionCount -ne 0) { throw 'Multiple API export tables' }
            $functionCount = [int]$Matches[1]
        } elseif ($line -match '^\s+[0-9]+\s+[0-9A-Fa-f]+\s+([A-Za-z_?][A-Za-z0-9_?@$]*) \(forwarded to ([A-Za-z0-9_]+)\.([A-Za-z_?][A-Za-z0-9_?@$]*)\)\s*$') {
            $sourceName = $Matches[1]
            $target = $Matches[2].ToLowerInvariant()
            $targetName = $Matches[3]
            if (($crt -and $target -ne 'ucrtbase') -or
                (-not $crt -and $target -notin 'kernel32','user32','advapi32','normaliz','ntdll')) { throw "Unverified API forwarding module: $target" }
            if ($forwarders.ContainsKey($sourceName)) { throw 'Duplicate API forwarding export' }
            $forwarders.Add($sourceName,[pscustomobject]@{ Library = $target; Name = $targetName })
        }
    }
    if ($functionCount -eq 0 -or $forwarders.Count -ne $functionCount) { throw "Unverified API forwarding export table: $LiteralPath ($($forwarders.Count)/$functionCount)" }
    if ($RequiredExports.Count -eq 0) { throw 'API forwarding audit needs the actually referenced exports' }
    $exportCache = @{}
    foreach ($sourceName in @($RequiredExports | Sort-Object -Unique)) {
        if (-not $forwarders.ContainsKey($sourceName)) { throw "Missing API bridge export: $sourceName" }
        $entry = $forwarders[$sourceName]
        if (-not $exportCache.ContainsKey($entry.Library)) {
            $runtime = if ($crt) { Join-Path $DependencyDirectory 'ucrtbase.dll' }
                else { Join-Path ([Environment]::SystemDirectory) "$($entry.Library).dll" }
            if (-not (Test-Path -LiteralPath $runtime -PathType Leaf)) { throw 'Missing forwarding target' }
            $runtimeExports = @(& dumpbin /nologo /exports $runtime)
            if ($LASTEXITCODE -ne 0) { throw 'Cannot read forwarding target exports' }
            $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            foreach ($line in $runtimeExports) {
                if ($line -match '^\s+[0-9]+\s+[0-9A-Fa-f]+\s+(?:[0-9A-Fa-f]+\s+)?([A-Za-z_?][A-Za-z0-9_?@$]*)(?: \(forwarded to [^)]+\))?\s*$') { [void]$names.Add($Matches[1]) }
            }
            $exportCache[$entry.Library] = $names
        }
        if (-not $exportCache[$entry.Library].Contains($entry.Name)) { throw "Missing API forwarding export: $($entry.Library)!$($entry.Name)" }
        $imports.Add([pscustomobject]@{ Library = "$($entry.Library).dll"; Name = $entry.Name; Delayed = $false; Ordinal = $null; ForwardedExport = $sourceName })
    }
}
foreach ($module in $libraries) {
    if (@($imports | Where-Object { $_.Library -eq $module.Name -and $_.Delayed -eq $module.Delayed }).Count -eq 0) {
        throw "PE library has no recognizable imports: $($module.Name)"
    }
}

$allowedDelayed = @{
    'combase.dll' = @('CoTaskMemFree', 'CoCreateFreeThreadedMarshaler')
    'api-ms-win-core-winrt-error-l1-1-0.dll' = @('RoOriginateErrorW')
    'mmdevapi.dll' = @('ActivateAudioInterfaceAsync')
}
$unsupported = @(
    'GetDpiForWindow', 'GetDpiForSystem', 'SetThreadDpiAwarenessContext',
    'SetProcessDpiAwarenessContext', 'GetWindowDpiAwarenessContext', 'GetAwarenessFromDpiAwarenessContext',
    'GetThreadDpiAwarenessContext', 'AreDpiAwarenessContextsEqual',
    'GetSystemTimePreciseAsFileTime', 'WaitOnAddress', 'WakeByAddressSingle', 'WakeByAddressAll',
    'GetTempPath2W', 'GetTempPath2A', 'CreateFile2', 'SetThreadDescription', 'GetThreadDescription',
    'ActivateAudioInterfaceAsync', 'RoOriginateErrorW', 'GetProcessMitigationPolicy',
    'PathCchRemoveFileSpec', 'PathCchRemoveBackslash', 'CreateDXGIFactory2',
    'LogicalToPhysicalPointForPerMonitorDPI', 'PhysicalToLogicalPointForPerMonitorDPI'
    'UiaDisconnectProvider', 'UiaDisconnectAllProviders',
    'WinHttpWebSocketClose', 'WinHttpWebSocketCompleteUpgrade', 'WinHttpWebSocketReceive', 'WinHttpWebSocketSend'
)
foreach ($entry in $imports) {
    if ($entry.Delayed) {
        if (-not $allowedDelayed.ContainsKey($entry.Library) -or $entry.Name -notin $allowedDelayed[$entry.Library]) {
            throw "Win7 hook does not cover delayed import: $($entry.Library)!$($entry.Name)"
        }
    } elseif ($entry.Library -in @('combase.dll', 'dcomp.dll', 'shcore.dll', 'api-ms-win-core-path-l1-1-0.dll') -or
              $entry.Library.StartsWith('api-ms-win-core-winrt-') -or $entry.Name -in $unsupported) {
        throw "Win7 core has an incompatible static import: $($entry.Library)!$($entry.Name)"
    }
}

# 此清单核对已发现的装载障碍；系统全部导出和第三方 DLL 仍需在 Win7 验收。
[pscustomobject]@{
    File = [IO.Path]::GetFileName($LiteralPath)
    Imports = @($imports.ToArray() | Sort-Object Library, Name, Delayed -Unique)
}
