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
    # Collect-Notices.ps1 reads the notices committed beside the data and code they cover, and the handwriting model's licence where scripts/fetch_handwriting_model.py puts it.
    $repositoryNotices = @('resources/licenses/msime-engine-dictionary-NOTICE.md', 'resources/helpcodes/ENGINE-NOTICE.md',
        'resources/helpcodes/NOTICE.md', 'target/handwriting-model/HandwritingModel-LICENSE.txt',
        'resources/licenses/Zinnia-LICENSE.txt', 'resources/licenses/Administrative-divisions-of-China-WTFPL.txt',
        'resources/licenses/libhangul-hanja-BSD-3-Clause.txt', 'resources/licenses/rime-cantonese-CC-BY-4.0.txt',
        'resources/licenses/libchewing-data-LGPL-2.1.txt', 'resources/licenses/vi-MIT.txt',
        'platforms/windows/third_party/miniaudio/LICENSE',
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
    $handwritingNotice = Join-Path $root 'target/handwriting-model/HandwritingModel-LICENSE.txt'
    foreach ($failure in @('notice', 'license', 'handwriting', 'voice')) {
        # A missing repository notice is refused; the file is restored before the next case.
        if ($failure -eq 'notice') { Remove-Item -LiteralPath $dictionaryNotice }
        if ($failure -eq 'license') {
            [IO.File]::WriteAllText($dictionaryNotice, 'synthetic committed notice resources/licenses/msime-engine-dictionary-NOTICE.md')
            [IO.File]::WriteAllText($license, '')
        }
        # The handwriting model's licence has to be fetched before notices are collected.
        if ($failure -eq 'handwriting') {
            [IO.File]::WriteAllText($license, 'synthetic dependency copyright')
            Remove-Item -LiteralPath $handwritingNotice
        }
        # A package carrying the speech runtime without its license is refused as well.
        if ($failure -eq 'voice') {
            [IO.File]::WriteAllText($handwritingNotice, 'synthetic committed notice target/handwriting-model/HandwritingModel-LICENSE.txt')
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
