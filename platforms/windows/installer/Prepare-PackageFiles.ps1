[CmdletBinding()]
param(
    [string]$TargetVersion = '0.0.1',
    # This script lives in platforms/windows/installer; resolve the repository
    # root rather than treating platforms/windows as the repository.
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))),
    # Component paths are relative to RepoRoot and default to the consolidated layout.
    # Historical or custom layouts remain available through explicit overrides.
    [string]$TsfDirectory = 'windows',
    [string]$ServerDirectory = 'server',
    # Deprecated compatibility argument; UI assets are embedded in Tauri now.
    [string]$UiHtmlDirectory = 'ui-html',
    # The helpcode tables, one flat directory; only its *.txt tables are staged, and its notices go into THIRD_PARTY_NOTICES.txt through Collect-Notices.ps1.
    [string]$HelpCodeDirectory = 'resources/helpcodes',
    # The zinnia handwriting model with its licence, relative to RepoRoot, as scripts/fetch_handwriting_model.py downloads them against resources/handwriting-model.lock.json. Without the model the package installs without offline handwriting.
    [string]$HandwritingDirectory = 'target/handwriting-model',
    # Deprecated: both resource layouts now use DesktopResourcesDirectory.
    [string]$DictionaryDirectory = 'MetasequoiaImeDict',
    [string]$ServerReleaseDirectory = '',
    # Native WinUI 3 settings binary; relative overrides are resolved against RepoRoot.
    [string]$DesktopExecutable = 'target/windows-full/x64/bin/msime-client-settings.exe',
    # Optional shared Tauri panel shell. The normal consolidated build stages it beside the Server.
    [string]$DesktopPreviewExecutable = '',
    # Exact files from resources/desktop-dictionary.lock.json; full packages only.
    [string]$DesktopResourcesDirectory = 'target/desktop-resources',
    [string]$Tsf32ReleaseDirectory = '',
    [string]$Tsf64ReleaseDirectory = '',
    # THIRD_PARTY_NOTICES.txt used to sit next to the tip's sources. In the consolidated repository
    # the notice covers the whole product and lives at the root, one level above windows/, so where
    # to read it is no longer answered by where the tip is.
    [string]$NoticesDirectory = '.',
    # The on-device speech runtime from scripts/fetch_voice_runtime.py --platform windows-x64; relative paths are resolved against RepoRoot. Used when the Server output does not already carry it.
    [string]$VoiceRuntimeDirectory = 'target/voice-runtime/windows-x64',
    [switch]$Light
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($TargetVersion -notmatch '^[0-9][0-9A-Za-z.+-]*$') {
    throw "Invalid installer version: $TargetVersion"
}

function Test-PackageTestArtifact {
    param([Parameter(Mandatory)][string]$BaseName)
    # Client CMake tests use windows-*, alongside the legacy test conventions.
    # Production entry points use MetasequoiaIme* or msime-client-* names.
    return $BaseName -like '*Tests' -or $BaseName -like 'test_*' -or $BaseName -like 'windows-*'
}

function Assert-PathExists {
    param([Parameter(Mandatory)][string]$LiteralPath, [Parameter(Mandatory)][string]$Description)
    if (-not (Test-Path -LiteralPath $LiteralPath)) {
        throw "$Description 不存在：$LiteralPath"
    }
}

function Copy-DirectoryContents {
    param([Parameter(Mandatory)][string]$Source, [Parameter(Mandatory)][string]$Destination)
    Assert-PathExists -LiteralPath $Source -Description '源目录'
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    Get-ChildItem -LiteralPath $Source -Force | Copy-Item -Destination $Destination -Recurse -Force
}

function Reset-Directory {
    param([Parameter(Mandatory)][string]$LiteralPath)
    if (Test-Path -LiteralPath $LiteralPath) {
        Remove-Item -LiteralPath $LiteralPath -Recurse -Force
    }
    New-Item -ItemType Directory -Path $LiteralPath -Force | Out-Null
}

$serverRelease = Join-Path $RepoRoot (Join-Path $ServerDirectory 'build-release\bin\Release')
if ($ServerReleaseDirectory) { $serverRelease = Join-Path $RepoRoot $ServerReleaseDirectory }
$clientNativeBin = Join-Path $RepoRoot 'target\windows-full\x64\bin'
if (-not $ServerReleaseDirectory -and (Test-Path -LiteralPath $clientNativeBin -PathType Container)) {
    $serverRelease = $clientNativeBin
}
$stagedDesktop = Join-Path $serverRelease 'msime-client-settings.exe'
$desktopSource = if (-not $PSBoundParameters.ContainsKey('DesktopExecutable') -and
    (Test-Path -LiteralPath $stagedDesktop -PathType Leaf)) {
    $stagedDesktop
} elseif ([IO.Path]::IsPathRooted($DesktopExecutable)) {
    $DesktopExecutable
} else {
    Join-Path $RepoRoot $DesktopExecutable
}
$previewSource = $null
if ($DesktopPreviewExecutable) {
    $previewSource = if ([IO.Path]::IsPathRooted($DesktopPreviewExecutable)) {
        $DesktopPreviewExecutable
    } else {
        Join-Path $RepoRoot $DesktopPreviewExecutable
    }
} else {
    $stagedPreview = Join-Path $serverRelease 'MSIME.exe'
    if (Test-Path -LiteralPath $stagedPreview -PathType Leaf) { $previewSource = $stagedPreview }
}
$dictionaryReplayRelease = Join-Path $serverRelease 'MetasequoiaImeDictionaryReplay.exe'
$mcpRelease = Join-Path $serverRelease 'msime-mcp.exe'
if (-not $Tsf32ReleaseDirectory -and (Test-Path -LiteralPath (Join-Path $RepoRoot 'target/windows-full/x86/bin') -PathType Container)) {
    $Tsf32ReleaseDirectory = 'target/windows-full/x86/bin'
}
if (-not $Tsf64ReleaseDirectory -and (Test-Path -LiteralPath $clientNativeBin -PathType Container)) {
    $Tsf64ReleaseDirectory = 'target/windows-full/x64/bin'
}
$tsf32Release = Join-Path $RepoRoot (Join-Path $TsfDirectory 'build32-release\Release\MetasequoiaImeTsf.dll')
$tsf64Release = Join-Path $RepoRoot (Join-Path $TsfDirectory 'build64-release\Release\MetasequoiaImeTsf.dll')
$tsf32Pdb = Join-Path $RepoRoot (Join-Path $TsfDirectory 'build32-release\Release\MetasequoiaImeTsf.pdb')
$tsf64Pdb = Join-Path $RepoRoot (Join-Path $TsfDirectory 'build64-release\Release\MetasequoiaImeTsf.pdb')
if ($Tsf32ReleaseDirectory) {
    $tsf32Release = Join-Path (Join-Path $RepoRoot $Tsf32ReleaseDirectory) 'MetasequoiaImeTsf.dll'
    $tsf32Pdb = Join-Path (Join-Path $RepoRoot $Tsf32ReleaseDirectory) 'MetasequoiaImeTsf.pdb'
}
if ($Tsf64ReleaseDirectory) {
    $tsf64Release = Join-Path (Join-Path $RepoRoot $Tsf64ReleaseDirectory) 'MetasequoiaImeTsf.dll'
    $tsf64Pdb = Join-Path (Join-Path $RepoRoot $Tsf64ReleaseDirectory) 'MetasequoiaImeTsf.pdb'
}
$tsf32Host = Join-Path (Split-Path -Parent $tsf32Release) 'msime_host_api.dll'
$tsf64Host = Join-Path (Split-Path -Parent $tsf64Release) 'msime_host_api.dll'
$factoryConfig = Join-Path $PSScriptRoot 'config.default.toml'
$iconSource = Join-Path $PSScriptRoot 'assets\icons'
$audioSource = Join-Path $PSScriptRoot 'assets\audios'
$pinyinTable = Join-Path $PSScriptRoot 'assets/tables/pinyin.txt'
$helpcodeSource = Join-Path $RepoRoot $HelpCodeDirectory
# 品牌标识。这个目录只放 ServerResources.rc 要编译进 Server 的那一个图标。
# 语言栏与工具栏的状态图标不在这里：它们在 tsf/assets 下，由 MetasequoiaIME.rc 编进 TSF DLL，
# 运行时走 MAKEINTRESOURCE。这里曾经有它们的一份逐字节副本，随包装到用户磁盘、且因为
# uninsneveruninstall 连卸载都不清除，而没有任何代码从磁盘读图标。
$appIcon = Join-Path $iconSource 'msime.ico'
$thirdPartyNotices = Join-Path $RepoRoot (Join-Path $NoticesDirectory 'THIRD_PARTY_NOTICES.txt')
$collectedNotices = Join-Path $RepoRoot 'target/windows-notices/THIRD_PARTY_NOTICES.txt'
if (-not $PSBoundParameters.ContainsKey('NoticesDirectory') -and
    (Test-Path -LiteralPath $collectedNotices -PathType Leaf)) {
    $thirdPartyNotices = $collectedNotices
}
$license = Join-Path $RepoRoot 'LICENSE'
$resourceSource = if ([IO.Path]::IsPathRooted($DesktopResourcesDirectory)) {
    $DesktopResourcesDirectory
} else { Join-Path $RepoRoot $DesktopResourcesDirectory }
$dictionaryDb = Join-Path $resourceSource 'msime.db'
$dictionaryManifest = Join-Path $resourceSource 'dictionary-manifest.json'
$japaneseModel = Join-Path $resourceSource 'dict_japanese.dat'
$japaneseModelLicense = Join-Path $resourceSource 'mozc_dictionary_oss_README.txt'
$englishDb = Join-Path $resourceSource 'english.db'
$othersDb = Join-Path $resourceSource 'others.db'
# 手写模型与其授权声明。Tauri 侧按可执行文件旁的 handwriting\handwriting-zh_CN.model 查找，因此这两个文件与 Server 一起落在 server_exe 下，而不是 app_data。来源由 resources/handwriting-model.lock.json 记录，不再随包附 provenance.json。
$handwritingSource = Join-Path $RepoRoot $HandwritingDirectory
$handwritingModel = Join-Path $handwritingSource 'handwriting-zh_CN.model'
$handwritingLicense = Join-Path $handwritingSource 'HandwritingModel-LICENSE.txt'

Assert-PathExists -LiteralPath $RepoRoot -Description '源码仓库根目录'
if (-not (Test-Path -LiteralPath $desktopSource -PathType Leaf)) {
    throw "缺少 WinUI 3 设置窗口，请先构建或通过 -DesktopExecutable 指定：$desktopSource"
}
if ($DesktopPreviewExecutable -and -not (Test-Path -LiteralPath $previewSource -PathType Leaf)) {
    throw "缺少 Tauri 面板外壳，请先构建或通过 -DesktopPreviewExecutable 指定：$previewSource"
}
Assert-PathExists -LiteralPath $serverRelease -Description 'Server Release 输出目录'
Assert-PathExists -LiteralPath (Join-Path $serverRelease 'MetasequoiaImeWatchdog.exe') -Description 'Watchdog Release EXE'
Assert-PathExists -LiteralPath $dictionaryReplayRelease -Description '用户词库回放程序 Release EXE'
Assert-PathExists -LiteralPath $mcpRelease -Description 'MCP 服务程序 Release EXE'
Assert-PathExists -LiteralPath $tsf32Release -Description '32 位 TSF Release DLL'
Assert-PathExists -LiteralPath $tsf64Release -Description '64 位 TSF Release DLL'
Assert-PathExists -LiteralPath $tsf32Pdb -Description '32 位 TSF Release PDB'
Assert-PathExists -LiteralPath $tsf64Pdb -Description '64 位 TSF Release PDB'
foreach ($hostDll in @($tsf32Host, $tsf64Host)) {
    if (-not (Test-Path -LiteralPath $hostDll -PathType Leaf)) {
        throw '缺少对应架构 TSF 的 msime_host_api.dll'
    }
}
$serverExecutables = @(
    Get-ChildItem -LiteralPath $serverRelease -Recurse -File -Filter '*.exe' |
        Where-Object { -not (Test-PackageTestArtifact -BaseName $_.BaseName) }
)
$missingServerPdb = @(
    $serverExecutables |
        Where-Object {
            -not (Test-Path -LiteralPath (Join-Path $_.DirectoryName "$($_.BaseName).pdb"))
        } |
        ForEach-Object { Join-Path $_.DirectoryName "$($_.BaseName).pdb" }
)
if ($missingServerPdb.Count -gt 0) {
    throw "Server Release 缺少同名 PDB：$($missingServerPdb -join ', ')"
}
Assert-PathExists -LiteralPath $appIcon -Description '应用图标'
Assert-PathExists -LiteralPath $thirdPartyNotices -Description '第三方声明 THIRD_PARTY_NOTICES.txt'
Assert-PathExists -LiteralPath $license -Description '许可证 LICENSE'

$desktopResources = @()
if (-not $Light) {
    # Check pinned bytes before either legacy or shared-runtime staging reads
    # them. Both layouts must be built from the same source generation.
    $desktopResources = @(& (Join-Path $PSScriptRoot 'Get-VerifiedDesktopResources.ps1') `
        -SourceDirectory $resourceSource `
        -ManifestPath (Join-Path $RepoRoot 'resources/desktop-dictionary.lock.json'))
    Assert-PathExists -LiteralPath $factoryConfig -Description '出厂配置 default_config\config.default.toml'
    Assert-PathExists -LiteralPath $pinyinTable -Description '完整拼音音节表 pinyin.txt'
    Assert-PathExists -LiteralPath $helpcodeSource -Description '辅助码目录'
    if (-not (Get-ChildItem -LiteralPath $helpcodeSource -File -Filter '*.txt')) {
        throw "辅助码目录中没有码表：$helpcodeSource"
    }
    Assert-PathExists -LiteralPath $dictionaryDb -Description '词库数据库 msime.db'
    Assert-PathExists -LiteralPath $japaneseModel -Description '日语整句模型 dict_japanese.dat'
    Assert-PathExists -LiteralPath $japaneseModelLicense -Description 'Mozc 日语词典授权声明'
    Assert-PathExists -LiteralPath $englishDb -Description '英文词库数据库 english.db'
    python -c @"
import sqlite3, sys
cols = list(sqlite3.connect(sys.argv[1]).execute('PRAGMA table_info(english_words)'))
names = {row[1] for row in cols}
pk = [row[1] for row in cols if row[5] > 0]
if 'weight' not in names or pk != ['word', 'display']:
    raise SystemExit('english.db schema is stale; rebuild with weight and PRIMARY KEY(word, display)')
"@ $englishDb
    if ($LASTEXITCODE -ne 0) {
        throw "英文词库数据库 schema 检查失败：$englishDb"
    }
    Assert-PathExists -LiteralPath $othersDb -Description '杂项数据库 others.db'
    # Validate source content before any existing package staging is removed.
    if (-not (Get-Content -LiteralPath $pinyinTable | Where-Object { $_.Trim() -eq 'xing' })) {
        throw "完整拼音音节表缺少 xing：$pinyinTable"
    }
    $defaultConfig = Get-Content -LiteralPath $factoryConfig -Raw
    if ($defaultConfig -notmatch '(?m)^schema\s*=\s*"quanpin"\s*$') {
        throw '出厂配置的 input.schema 必须是 quanpin。'
    }
    if ($defaultConfig -notmatch '(?m)^theme_mode\s*=\s*"system"\s*$') {
        throw '出厂配置的 appearance.theme_mode 必须是 system。'
    }
    if ($defaultConfig -match '(?m)^diagnostic_log\s*=\s*true\s*$') {
        throw '出厂配置不应默认打开 diagnostic_log。'
    }
    $defaultConfig = $defaultConfig.TrimEnd("`r", "`n") + "`r`n"
}

$hasHandwritingModel = Test-Path -LiteralPath $handwritingModel -PathType Leaf
if ($hasHandwritingModel) {
    Assert-PathExists -LiteralPath $handwritingLicense -Description '手写模型随附声明'
}

# On-device speech recognition. The Server loads sherpa-onnx-c-api.dll with LoadLibrary from its own directory, and onnxruntime.dll and its provider bridge resolve beside it, so all three ride in server_exe. Build-Client.ps1 stages them into the Server output; a separately fetched runtime directory is the fallback. The set is all or nothing: a partial one would install a recognizer that fails at first use, so it is refused here, before any previous staging is replaced. With none of them the package installs without local recognition, and a dictation set to the local provider says the component cannot be loaded.
$voiceRuntimeLibraries = @('sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll')
$voiceRuntimeSource = if ([IO.Path]::IsPathRooted($VoiceRuntimeDirectory)) {
    $VoiceRuntimeDirectory
} else { Join-Path $RepoRoot $VoiceRuntimeDirectory }
$voiceRuntimeFrom = $null
foreach ($candidate in @($serverRelease, $voiceRuntimeSource)) {
    $present = @($voiceRuntimeLibraries | Where-Object { Test-Path -LiteralPath (Join-Path $candidate $_) -PathType Leaf })
    if ($present.Count -eq 0) { continue }
    if ($present.Count -ne $voiceRuntimeLibraries.Count) {
        $missing = @($voiceRuntimeLibraries | Where-Object { $_ -notin $present })
        throw "本地语音识别运行时不完整（$candidate），缺少：$($missing -join ', ')"
    }
    $voiceRuntimeFrom = $candidate
    break
}
# The runtime ships under Apache-2.0 (sherpa-onnx) and MIT (ONNX Runtime), both of which require their license to travel with the binaries. Collect-Notices.ps1 writes those sections; a notice file without them (an older collection, a hand-supplied one) would ship the DLLs unlicensed, so it is refused here, before any previous staging is replaced.
if ($null -ne $voiceRuntimeFrom) {
    $noticeText = [IO.File]::ReadAllText($thirdPartyNotices)
    $missingVoiceNotices = @(@('sherpa-onnx', 'ONNX Runtime') | Where-Object { -not $noticeText.Contains($_) })
    if ($missingVoiceNotices.Count -gt 0) {
        throw "第三方声明缺少本地语音识别运行时的许可证（$($missingVoiceNotices -join ', ')），请用 Collect-Notices.ps1 重新生成：$thirdPartyNotices"
    }
}

$targetAppData = Join-Path $PSScriptRoot 'app_data'
$targetServer = Join-Path $PSScriptRoot 'server_exe'
$targetTsf = Join-Path $PSScriptRoot 'tsf_dll'

if ($Light) {
    Write-Host '轻量模式：跳过词库、辅助码、拼音表和出厂配置，刷新 TSF、Server 与 Tauri。'
    New-Item -ItemType Directory -Path $targetAppData -Force | Out-Null
}
else {
    Reset-Directory -LiteralPath $targetAppData
    Copy-Item -LiteralPath $pinyinTable -Destination (Join-Path $targetAppData 'pinyin.txt') -Force
    Copy-Item -LiteralPath $dictionaryDb -Destination (Join-Path $targetAppData 'msime.db') -Force
    if (Test-Path -LiteralPath $dictionaryManifest) {
        Copy-Item -LiteralPath $dictionaryManifest -Destination (Join-Path $targetAppData 'dictionary-manifest.json') -Force
    }
    Copy-Item -LiteralPath $japaneseModel -Destination (Join-Path $targetAppData 'dict_japanese.dat') -Force
    Copy-Item -LiteralPath $japaneseModelLicense -Destination (Join-Path $targetAppData 'MOZC_DICTIONARY_LICENSE.txt') -Force
    Copy-Item -LiteralPath $englishDb -Destination (Join-Path $targetAppData 'english.db') -Force
    Copy-Item -LiteralPath $othersDb -Destination (Join-Path $targetAppData 'others.db') -Force

    $defaultConfigPath = Join-Path $targetAppData 'config.default.toml'
    # 出厂配置来自本仓库的 default_config，不依赖本机是否已安装输入法。
    # 安装脚本用 onlyifdoesntexist 生成用户 config.toml，升级不会覆盖已有方案/主题。
    Set-Content -LiteralPath $defaultConfigPath -Value $defaultConfig -Encoding utf8NoBOM -NoNewline
    foreach ($stagedUserConfig in @('config.toml', 'config.base.toml')) {
        $stagedPath = Join-Path $targetAppData $stagedUserConfig
        if (Test-Path -LiteralPath $stagedPath) {
            Remove-Item -LiteralPath $stagedPath -Force
        }
    }

    $targetHelpcodes = Join-Path $targetAppData 'helpcodes'
    Reset-Directory -LiteralPath $targetHelpcodes
    Get-ChildItem -LiteralPath $helpcodeSource -File -Filter '*.txt' |
        Copy-Item -Destination $targetHelpcodes -Force
}

$targetHtml = Join-Path $targetAppData 'html'
$targetAudios = Join-Path $targetAppData 'audios'
Copy-DirectoryContents -Source $audioSource -Destination $targetAudios
# The built-in sound packs, synthesized by scripts/generate_sound_packs.py. The Server names DataDir\sound-packs to client-core as the built-in pack root; installed packs live under DataDir\plugins, which is user state.
$targetSoundPacks = Join-Path $targetAppData 'sound-packs'
Reset-Directory -LiteralPath $targetSoundPacks
Copy-DirectoryContents -Source (Join-Path $RepoRoot 'resources/sound-packs') -Destination $targetSoundPacks
if (Test-Path -LiteralPath $targetHtml) {
    # Remove obsolete package staging, not the user's installed files.
    Remove-Item -LiteralPath $targetHtml -Recurse -Force
}

# Server Release 输出整体复制，但测试程序及其 PDB 绝不能进入安装包。
# 其他 PDB 保留在对应 EXE 旁边，方便安装后直接进行崩溃分析。
Reset-Directory -LiteralPath $targetServer
Copy-DirectoryContents -Source $serverRelease -Destination $targetServer
# Match ShellSurfaces.h, independent of Cargo/Tauri's build artifact filename.
Copy-Item -LiteralPath $desktopSource -Destination (Join-Path $targetServer 'msime-client-settings.exe') -Force
# An explicit shell override must not inherit a PDB from the native shell that
# was copied with Server output. Only stage symbols beside the chosen source.
$targetDesktopPdb = Join-Path $targetServer 'msime-client-settings.pdb'
if (Test-Path -LiteralPath $targetDesktopPdb) { Remove-Item -LiteralPath $targetDesktopPdb -Force }
$desktopPdbSource = [IO.Path]::ChangeExtension($desktopSource, '.pdb')
if (Test-Path -LiteralPath $desktopPdbSource -PathType Leaf) {
    Copy-Item -LiteralPath $desktopPdbSource -Destination $targetDesktopPdb
}
if ($previewSource) {
    Copy-Item -LiteralPath $previewSource -Destination (Join-Path $targetServer 'MSIME.exe') -Force
    $previewPdbSource = [IO.Path]::ChangeExtension($previewSource, '.pdb')
    if (Test-Path -LiteralPath $previewPdbSource -PathType Leaf) {
        Copy-Item -LiteralPath $previewPdbSource -Destination (Join-Path $targetServer 'MSIME.pdb') -Force
    }
}
# Inno recursively installs server_exe under Program Files. Keep these verified
# read-only sources separate from legacy app_data and per-user writable state.
$targetResources = Join-Path $targetServer 'resources'
if ($Light -and (Test-Path -LiteralPath $targetResources)) {
    # Do not inherit a stale bundle from a reused native build directory.
    Remove-Item -LiteralPath $targetResources -Recurse -Force
}
if (-not $Light) {
    Reset-Directory -LiteralPath $targetResources
    foreach ($resource in $desktopResources) {
        Copy-Item -LiteralPath $resource -Destination $targetResources
    }
    # Recheck the staged bytes too: a changing source must not produce a package
    # that only passed its preflight hash check.
    $null = & (Join-Path $PSScriptRoot 'Get-VerifiedDesktopResources.ps1') `
        -SourceDirectory $targetResources `
        -ManifestPath (Join-Path $RepoRoot 'resources/desktop-dictionary.lock.json')
}
# 落定重排模型，装在资源目录的**同级**而不是里面。
#
# 装在里面会被上面那次复验当场拒绝：它要求那个目录恰好等于词库锁钉死的产物集，
# 而那道校验的职责正是证明已发布的词库完整。prepare_host_configuration 去找的
# 就是这个同级目录，找到才会把路径写进运行时配置。
#
# 可选：25MB 换的是桌面独有的提升（收割集 top-1 0.123 → 0.613），
# 用 scripts/fetch_settled_model.py 取。没有就不装，行为与今天一致。
$settledSource = Join-Path $RepoRoot 'target/settled-model'
# fetch_neural_model.py stages both presets together. Preserve the historical one-artifact path,
# then fall back to the shared directory when it is the only prepared source.
$neuralModelSource = Join-Path $RepoRoot 'target/neural-model'
if (-not (Test-Path -LiteralPath (Join-Path $settledSource 'sentence-model-desktop.safetensors') -PathType Leaf) -and
    (Test-Path -LiteralPath (Join-Path $neuralModelSource 'sentence-model-desktop.safetensors') -PathType Leaf)) {
    $settledSource = $neuralModelSource
}
$settledTarget = Join-Path $targetServer 'settled-model'
if (Test-Path -LiteralPath $settledTarget) {
    Remove-Item -LiteralPath $settledTarget -Recurse -Force
}
if (-not $Light) {
    $settledLock = Join-Path $RepoRoot 'resources/settled-model.lock.json'
    $settledManifest = Get-Content -LiteralPath $settledLock -Raw | ConvertFrom-Json
    $settledFiles = @()
    foreach ($artifact in $settledManifest.artifacts) {
        $candidate = Join-Path $settledSource ([string]$artifact.name)
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) { $settledFiles = @(); break }
        $file = Get-Item -LiteralPath $candidate -Force
        if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            $file.Length -ne $artifact.size -or
            (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash -ne $artifact.sha256) {
            throw "落定模型 $($artifact.name) 与锁文件不符"
        }
        $settledFiles += $file.FullName
    }
    if ($settledFiles.Count -gt 0) {
        New-Item -ItemType Directory -Path $settledTarget -Force | Out-Null
        foreach ($file in $settledFiles) { Copy-Item -LiteralPath $file -Destination $settledTarget -Force }
        Write-Host "落定重排模型已装入：$settledTarget"
    } else {
        Write-Host "未找到落定重排模型（$settledSource），桌面落定重排保持关闭"
    }
}
# Non-English candidate glosses (scripts/build_offline_glosses.py), one zh-<lang>.db per target language, installed beside resources for the same reason as the settled model: the verified directory must equal the dictionary lock exactly, and the Engine looks for them in this sibling. Optional; without them the candidate glosses stay English only.
$glossesSource = Join-Path $RepoRoot 'target/offline-glosses'
$glossesTarget = Join-Path $targetServer 'offline-glosses'
if (Test-Path -LiteralPath $glossesTarget) {
    Remove-Item -LiteralPath $glossesTarget -Recurse -Force
}
if (-not $Light) {
    $glossFiles = @()
    $glossNotice = Join-Path $glossesSource 'offline-glosses-NOTICE.txt'
    if (Test-Path -LiteralPath $glossNotice -PathType Leaf) {
        $glossFiles = @(Get-ChildItem -LiteralPath $glossesSource -Filter 'zh-*.db' -File)
    }
    if ($glossFiles.Count -gt 0) {
        New-Item -ItemType Directory -Path $glossesTarget -Force | Out-Null
        foreach ($file in $glossFiles) { Copy-Item -LiteralPath $file.FullName -Destination $glossesTarget -Force }
        Copy-Item -LiteralPath $glossNotice -Destination $glossesTarget -Force
        Write-Host "Offline glosses staged: $($glossFiles.Count) languages in $glossesTarget"
    } else {
        Write-Host "No offline glosses with their notice in $glossesSource; candidate glosses stay English only"
    }
}
# The Cantonese and Zhuyin dictionaries (scripts/fetch_language_dictionaries.py into target/language-dictionaries, pinned by resources/language-dictionaries.lock.json), installed beside resources like the glosses: host-api finds language-dictionaries there and records it in the runtime options. Optional; without a dictionary its scheme is shown as unavailable and falls back to the last Chinese scheme, and Vietnamese needs no data. Each dictionary ships only with its licence text, which must travel with the data. Set MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 to fail a package that does not carry both.
$languagesSource = Join-Path $RepoRoot 'target/language-dictionaries'
$languagesTarget = Join-Path $targetServer 'language-dictionaries'
if (Test-Path -LiteralPath $languagesTarget) {
    Remove-Item -LiteralPath $languagesTarget -Recurse -Force
}
if (-not $Light) {
    $stagedLanguages = @()
    foreach ($pair in @(@('cantonese.db', 'rime_cantonese_LICENSE.txt'), @('zhuyin.db', 'libchewing_data_LICENSE.txt'))) {
        $database = Join-Path $languagesSource $pair[0]
        $license = Join-Path $languagesSource $pair[1]
        if (-not (Test-Path -LiteralPath $database -PathType Leaf)) { continue }
        if (-not (Test-Path -LiteralPath $license -PathType Leaf)) {
            throw "$database 旁边没有 $($pair[1])，不能在缺少授权声明的情况下打包这份数据"
        }
        New-Item -ItemType Directory -Path $languagesTarget -Force | Out-Null
        Copy-Item -LiteralPath $database -Destination $languagesTarget -Force
        Copy-Item -LiteralPath $license -Destination $languagesTarget -Force
        $stagedLanguages += $pair[0]
    }
    if ($stagedLanguages.Count -gt 0) {
        Write-Host "语言词库已装入（$($stagedLanguages -join ', ')）：$languagesTarget"
    } else {
        Write-Host "未找到语言词库（$languagesSource），粤拼和注音保持不可用"
    }
    if ($env:MSIME_REQUIRE_LANGUAGE_DICTIONARIES -eq '1' -and $stagedLanguages.Count -ne 2) {
        throw "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1，但 $languagesSource 中的 cantonese.db 和 zhuyin.db 没有全部装入"
    }
}
# Both package modes replace Server output. Copy model resources afterwards,
# otherwise Reset-Directory silently removes them from an otherwise valid package.
if ($hasHandwritingModel) {
    $targetHandwriting = Join-Path $targetServer 'handwriting'
    New-Item -ItemType Directory -Path $targetHandwriting -Force | Out-Null
    Copy-Item -LiteralPath $handwritingModel -Destination $targetHandwriting -Force
    Copy-Item -LiteralPath $handwritingLicense -Destination $targetHandwriting -Force
} else {
    Write-Host "未找到手写模型，跳过：$handwritingModel"
}
if ($null -eq $voiceRuntimeFrom) {
    Write-Host "未找到本地语音识别运行时（$voiceRuntimeSource），安装包不含本地语音识别"
} elseif ($voiceRuntimeFrom -ne $serverRelease) {
    # A runtime inside the Server output was already copied with it above.
    foreach ($library in $voiceRuntimeLibraries) {
        Copy-Item -LiteralPath (Join-Path $voiceRuntimeFrom $library) -Destination $targetServer -Force
    }
}
Get-ChildItem -LiteralPath $targetServer -Recurse -File |
    Where-Object {
        $_.Extension -in @('.exe', '.pdb') -and
        (Test-PackageTestArtifact -BaseName $_.BaseName)
    } |
    Remove-Item -Force

Reset-Directory -LiteralPath $targetTsf
$targetTsf32 = Join-Path $targetTsf '32'
$targetTsf64 = Join-Path $targetTsf '64'
New-Item -ItemType Directory -Path $targetTsf32, $targetTsf64 -Force | Out-Null
Copy-Item -LiteralPath $tsf32Release -Destination $targetTsf32 -Force
Copy-Item -LiteralPath $tsf32Pdb -Destination $targetTsf32 -Force
Copy-Item -LiteralPath $tsf64Release -Destination $targetTsf64 -Force
Copy-Item -LiteralPath $tsf64Pdb -Destination $targetTsf64 -Force
Copy-Item -LiteralPath $tsf32Host -Destination $targetTsf32 -Force
Copy-Item -LiteralPath $tsf64Host -Destination $targetTsf64 -Force
foreach ($pair in @(@($tsf32Release, $targetTsf32), @($tsf64Release, $targetTsf64))) {
    # Build-Client collects architecture-checked release dependencies beside TIP.
    Get-ChildItem -LiteralPath (Split-Path -Parent $pair[0]) -File -Filter '*.dll' |
        Where-Object { $_.Name -notin @('MetasequoiaImeTsf.dll', 'msime_host_api.dll') } |
        Copy-Item -Destination $pair[1] -Force
}
Copy-Item -LiteralPath $appIcon -Destination (Join-Path $PSScriptRoot 'MetasequoiaIME.ico') -Force
# rime-ice is GPL-3.0 and requires attribution, and its content forms the bulk of msime.db, so the
# notice has to reach the user's disk rather than only exist in the source repository.
Copy-Item -LiteralPath $thirdPartyNotices -Destination (Join-Path $PSScriptRoot 'THIRD_PARTY_NOTICES.txt') -Force
# GPLv3 sections 4 and 6 require a copy of the licence to reach whoever receives the program, and the
# packaged product includes third-party GPL-3.0 dictionary data. macOS and Linux already install the
# licence text (CMakeLists.txt in MSIME-Apple and MSIME-Linux); Windows is the platform that actually
# ships at volume and was the only one omitting it. THIRD_PARTY_NOTICES.txt does not cover this: it
# points at "the LICENSE file" without carrying the GPL text itself.
Copy-Item -LiteralPath $license -Destination (Join-Path $PSScriptRoot 'LICENSE.txt') -Force

$targetIss = Join-Path $PSScriptRoot 'msime_setup.iss'
Assert-PathExists -LiteralPath $targetIss -Description '安装脚本'
$issContent = Get-Content -LiteralPath $targetIss -Raw
if ($issContent -notmatch '(?m)^#define\s+MyAppVersion\s+"[^"]+"\s*$') {
    throw '未能在安装脚本中找到 MyAppVersion。'
}
$updatedIss = [regex]::Replace(
    $issContent,
    '(?m)^#define\s+MyAppVersion\s+"[^"]+"\s*$',
    "#define MyAppVersion   `"$TargetVersion`""
)
$updatedIss = $updatedIss.TrimEnd("`r", "`n") + "`r`n"
Set-Content -LiteralPath $targetIss -Value $updatedIss -Encoding utf8NoBOM -NoNewline

$serverBinaryCount = @(Get-ChildItem -LiteralPath $targetServer -Recurse -File -Include '*.exe', '*.dll').Count
$tsfBinaryCount = @(Get-ChildItem -LiteralPath $targetTsf -Recurse -File -Include '*.exe', '*.dll').Count
$symbolCount = @(
    Get-ChildItem -LiteralPath $targetServer, $targetTsf -Recurse -File -Filter '*.pdb'
).Count
$modeLabel = if ($Light) { '轻量' } else { '完整' }
Write-Host "安装文件准备完成（$modeLabel）：$PSScriptRoot"
Write-Host "版本：$TargetVersion；Server EXE/DLL：$serverBinaryCount 个；TSF EXE/DLL：$tsfBinaryCount 个；PDB：$symbolCount 个。"
