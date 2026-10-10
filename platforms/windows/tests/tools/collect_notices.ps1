$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-notices-' + [Guid]::NewGuid())
try {
    $prefix = Join-Path $root 'deps'
    New-Item -ItemType Directory -Force (Join-Path $prefix 'share/synthetic-lib') | Out-Null
    $license = Join-Path $prefix 'share/synthetic-lib/copyright'
    [IO.File]::WriteAllText($license, 'synthetic dependency copyright')
    $supplement = Join-Path $root 'extra.txt'
    [IO.File]::WriteAllText($supplement, 'synthetic extra notice')
    # Collect-Notices.ps1 读取随数据和代码一起提交的声明。
    $repositoryNotices = @('resources/licenses/msime-engine-dictionary-NOTICE.md', 'resources/helpcodes/ENGINE-NOTICE.md',
        'resources/helpcodes/NOTICE.md', 'resources/helpcodes/NOTICE-wubi86.md',
        'resources/licenses/Zinnia-LICENSE.txt', 'resources/licenses/Administrative-divisions-of-China-WTFPL.txt',
        'resources/licenses/libhangul-hanja-BSD-3-Clause.txt', 'resources/licenses/rime-cantonese-CC-BY-4.0.txt',
        'resources/licenses/libchewing-data-LGPL-2.1.txt', 'resources/licenses/rime-stroke-LGPL-3.0.txt',
        'resources/licenses/vi-MIT.txt', 'resources/licenses/ewts-MIT.txt',
        'platforms/windows/third_party/miniaudio/LICENSE', 'shared/voice/third_party/cppcodec/LICENSE',
        'crates/client-core/data/opencc/LICENSE')
    foreach ($relative in $repositoryNotices) {
        $path = Join-Path $root $relative
        New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null
        [IO.File]::WriteAllText($path, "synthetic committed notice $relative")
    }
    # The on-device speech runtime shipped beside the Server: its version comes from the lock, its license texts from the repository.
    $voiceLock = Join-Path $root 'resources/voice-runtime.lock.json'
    New-Item -ItemType Directory -Force (Split-Path -Parent $voiceLock) | Out-Null
    [IO.File]::WriteAllText($voiceLock, '{ "version": "1.13.8" }')
    $voiceNotices = @('shared/voice/third_party/sherpa-onnx/LICENSE', 'platforms/linux/data/licenses/onnxruntime-MIT.txt',
        'platforms/linux/data/licenses/onnxruntime-ThirdPartyNotices.txt')
    foreach ($relative in $voiceNotices) {
        $path = Join-Path $root $relative
        New-Item -ItemType Directory -Force (Split-Path -Parent $path) | Out-Null
        [IO.File]::WriteAllText($path, "synthetic voice runtime notice $relative")
    }
    # 版本表列出可以收集声明的产品版本。
    $editionTable = Join-Path $root 'shared/contracts/editions.json'
    New-Item -ItemType Directory -Force (Split-Path -Parent $editionTable) | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../../../../shared/contracts/editions.json') -Destination $editionTable
    $entry = Join-Path $PSScriptRoot '../../Collect-Notices.ps1'
    & $entry -RepoRoot $root -DependencyPrefixes @($prefix) -SupplementalNotices @($supplement)
    $output = Join-Path $root 'target/windows-notices/THIRD_PARTY_NOTICES.txt'
    $first = [IO.File]::ReadAllText($output)
    if (-not $first.Contains('synthetic extra notice') -or -not $first.Contains('synthetic dependency copyright') -or
        $first.Contains($root) -or $first.Contains('MSIME-Engine')) { throw 'Notice content/provenance mismatch' }
    foreach ($relative in $repositoryNotices) {
        if (-not $first.Contains("synthetic committed notice $relative") -or -not $first.Contains("($relative) =====")) {
            throw "Repository notice not collected: $relative"
        }
    }
    foreach ($relative in $voiceNotices) {
        if (-not $first.Contains("synthetic voice runtime notice $relative")) { throw "Voice runtime notice not collected: $relative" }
    }
    # Prepare-PackageFiles.ps1 looks for these names before it packages the runtime DLLs.
    if (-not $first.Contains('===== sherpa-onnx 1.13.8 (sherpa-onnx-c-api.dll), Apache License 2.0 (shared/voice/third_party/sherpa-onnx/LICENSE) =====') -or
        -not $first.Contains('===== ONNX Runtime (onnxruntime.dll, onnxruntime_providers_shared.dll), MIT License (platforms/linux/data/licenses/onnxruntime-MIT.txt) =====')) {
        throw 'Voice runtime notice provenance mismatch'
    }
    & $entry -RepoRoot $root -DependencyPrefixes @($prefix) -SupplementalNotices @($supplement)
    if ([IO.File]::ReadAllText($output) -ne $first) { throw 'Notice generation is not deterministic' }
    $dictionaryNotice = Join-Path $root 'resources/licenses/msime-engine-dictionary-NOTICE.md'
    # 安装包不带手写模型（设置应用下载模型时连同许可证一起下载），所以即使 target/handwriting-model 里有下载好的副本，任何一次收集都不列出模型的许可证；zinnia 的许可证保留，对应宿主库编进的识别器。
    if ($first.Contains('HandwritingModel-LICENSE') -or $first.Contains('handwriting-zh_CN.model')) { throw 'Collection lists the handwriting model the installer does not carry' }
    $staleModelNotice = Join-Path $root 'target/handwriting-model/HandwritingModel-LICENSE.txt'
    New-Item -ItemType Directory -Force (Split-Path -Parent $staleModelNotice) | Out-Null
    [IO.File]::WriteAllText($staleModelNotice, 'synthetic fetched model licence')
    & $entry -RepoRoot $root -DependencyPrefixes @($prefix) -SupplementalNotices @($supplement)
    if ([IO.File]::ReadAllText($output) -ne $first) { throw 'A fetched handwriting model licence changed the collection' }
    # 每个版本收集的仓库声明都相同。
    $editionOutput = Join-Path $root 'edition-notices'
    & $entry -RepoRoot $root -DependencyPrefixes @($prefix) -SupplementalNotices @($supplement) -Edition vietnamese -OutputDirectory $editionOutput
    $editionNotices = [IO.File]::ReadAllText((Join-Path $editionOutput 'THIRD_PARTY_NOTICES.txt'))
    foreach ($relative in $repositoryNotices) {
        if (-not $editionNotices.Contains("synthetic committed notice $relative")) { throw "Repository notice not collected for another edition: $relative" }
    }
    # An edition the table does not give a Windows section is refused.
    $rejected = $false
    try { & $entry -RepoRoot $root -DependencyPrefixes @($prefix) -Edition nosuchedition } catch { $rejected = $true }
    if (-not $rejected -or [IO.File]::ReadAllText($output) -ne $first) { throw 'Unknown edition accepted' }
    foreach ($failure in @('notice', 'license', 'voice')) {
        # A missing repository notice is refused; the file is restored before the next case.
        if ($failure -eq 'notice') { Remove-Item -LiteralPath $dictionaryNotice }
        if ($failure -eq 'license') {
            [IO.File]::WriteAllText($dictionaryNotice, 'synthetic committed notice resources/licenses/msime-engine-dictionary-NOTICE.md')
            [IO.File]::WriteAllText($license, '')
        }
        # A package carrying the speech runtime without its license is refused as well.
        if ($failure -eq 'voice') {
            [IO.File]::WriteAllText($license, 'synthetic dependency copyright')
            Remove-Item -LiteralPath (Join-Path $root 'platforms/linux/data/licenses/onnxruntime-MIT.txt')
        }
        $rejected = $false
        try { & $entry -RepoRoot $root -DependencyPrefixes @($prefix) } catch { $rejected = $true }
        if (-not $rejected -or [IO.File]::ReadAllText($output) -ne $first) { throw 'Failed collection damaged previous notices' }
    }
    Write-Output 'Notice provenance, deterministic output and failed-input preservation passed'
} finally {
    if (Test-Path $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
