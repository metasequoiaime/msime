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
    # The helpcode tables, one flat directory; only its *.txt tables are staged, and its notices go into THIRD_PARTY_NOTICES.txt through Collect-Notices.ps1.
    [string]$HelpCodeDirectory = 'resources/helpcodes',
    [string]$ServerReleaseDirectory = '',
    # Native WinUI 3 settings binary; relative overrides are resolved against RepoRoot.
    [string]$DesktopExecutable = 'target/windows-full/x64/bin/msime-client-settings.exe',
    # Optional shared Tauri panel shell. The normal consolidated build stages it beside the Server.
    [string]$DesktopPreviewExecutable = '',
    # Exact files from resources/desktop-dictionary.lock.json; full packages only.
    [string]$DesktopResourcesDirectory = 'target/desktop-resources',
    [string]$Tsf32ReleaseDirectory = '',
    [string]$Tsf64ReleaseDirectory = '',
    # Build-Client.ps1 的 Arm64X TIP 和它原生那一半导入的 ARM64 宿主 DLL；安装器只在 Windows on Arm 上用它们代替 64 位 TIP。
    [string]$TsfArm64ReleaseDirectory = '',
    # THIRD_PARTY_NOTICES.txt used to sit next to the tip's sources. In the consolidated repository
    # the notice covers the whole product and lives at the root, one level above windows/, so where
    # to read it is no longer answered by where the tip is.
    [string]$NoticesDirectory = '.',
    # The on-device speech runtime from scripts/fetch_voice_runtime.py --platform windows-x64; relative paths are resolved against RepoRoot. Used when the Server output does not already carry it.
    [string]$VoiceRuntimeDirectory = 'target/voice-runtime/windows-x64',
    # 产品版本（shared/contracts/editions.json 里有 Windows 段的 id）。决定从哪个构建目录取文件（full 是 target/windows-full，其他版本是 target/windows-<id>）、host DLL 的名字、按哪份资源锁校验词库、带哪些语言词库，以及 Server 目录里版本声明写哪个版本。缺省是 full。
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full',
    [switch]$Light
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($TargetVersion -notmatch '^[0-9][0-9A-Za-z.+-]*$') {
    throw "Invalid installer version: $TargetVersion"
}

$editionTable = Get-Content -LiteralPath (Join-Path $RepoRoot 'shared/contracts/editions.json') -Raw | ConvertFrom-Json
$editionEntry = @($editionTable.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
if ($editionEntry.Count -ne 1) {
    throw "版本 $Edition 在 shared/contracts/editions.json 里没有 Windows 标识"
}
$hostDllName = [string]$editionEntry[0].platforms.windows.host_dll
# full 的资源锁是原文件本身，其他版本的是 scripts/editions.py gen-locks 生成的子集（与 client-core 的 Edition::resource_lock 一致）。
$resourceLock = if ($Edition -eq 'full') {
    Join-Path $RepoRoot 'resources/desktop-dictionary.lock.json'
} else {
    Join-Path $RepoRoot "resources/editions/$Edition.lock.json"
}
$languageDictionaryNames = @($editionEntry[0].language_dictionaries)
# 落定重排模型（settled-model）和手写模型（Zinnia handwriting-zh_CN.model）不进安装包：设置应用按 resources/settled-model.lock.json 和 resources/handwriting-model.lock.json 把它们下载到 DataDir\resource-packs（手写模型连同它的 LGPL-2.1 许可证一起下载）。
# 非英文离线释义（offline-glosses/zh-<语言>.db）按中文候选查释义，只给提供中文方案的版本（版本表 features.offline_glosses，scripts/test-editions.py 检查它等于版本是否提供中文方案）。日文、越南文和藏文版不装。
$editionOfflineGlosses = [bool]$editionEntry[0].features.offline_glosses
$editionBuild = "target/windows-$Edition"
if (-not $PSBoundParameters.ContainsKey('DesktopExecutable')) {
    $DesktopExecutable = "$editionBuild/x64/bin/msime-client-settings.exe"
}

function Test-PackageTestArtifact {
    param([Parameter(Mandatory)][string]$BaseName)
    # Client CMake tests use windows-*; the other patterns cover the remaining test executables.
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
$clientNativeBin = Join-Path $RepoRoot "$editionBuild\x64\bin"
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
$mcpRelease = Join-Path $serverRelease 'msime-mcp.exe'
if (-not $Tsf32ReleaseDirectory -and (Test-Path -LiteralPath (Join-Path $RepoRoot "$editionBuild/x86/bin") -PathType Container)) {
    $Tsf32ReleaseDirectory = "$editionBuild/x86/bin"
}
if (-not $Tsf64ReleaseDirectory -and (Test-Path -LiteralPath $clientNativeBin -PathType Container)) {
    $Tsf64ReleaseDirectory = "$editionBuild/x64/bin"
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
if (-not $TsfArm64ReleaseDirectory) { $TsfArm64ReleaseDirectory = "$editionBuild/arm64/bin" }
$tsfArm64Directory = Join-Path $RepoRoot $TsfArm64ReleaseDirectory
$tsfArm64Release = Join-Path $tsfArm64Directory 'MetasequoiaImeTsf.dll'
$tsfArm64Pdb = Join-Path $tsfArm64Directory 'MetasequoiaImeTsf.pdb'
$arm64HostDllName = [IO.Path]::GetFileNameWithoutExtension($hostDllName) + '_arm64.dll'
$tsfArm64Host = Join-Path $tsfArm64Directory $arm64HostDllName
$tsf32Host = Join-Path (Split-Path -Parent $tsf32Release) $hostDllName
$tsf64Host = Join-Path (Split-Path -Parent $tsf64Release) $hostDllName
$factoryConfig = Join-Path $PSScriptRoot 'config.default.toml'
$iconSource = Join-Path $PSScriptRoot 'assets\icons'
$audioSource = Join-Path $PSScriptRoot 'assets\audios'
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
$englishDb = Join-Path $resourceSource 'msime-english.db'

Assert-PathExists -LiteralPath $RepoRoot -Description '源码仓库根目录'
if (-not (Test-Path -LiteralPath $desktopSource -PathType Leaf)) {
    throw "缺少 WinUI 3 设置窗口，请先构建或通过 -DesktopExecutable 指定：$desktopSource"
}
if ($DesktopPreviewExecutable -and -not (Test-Path -LiteralPath $previewSource -PathType Leaf)) {
    throw "缺少 Tauri 面板外壳，请先构建或通过 -DesktopPreviewExecutable 指定：$previewSource"
}
Assert-PathExists -LiteralPath $serverRelease -Description 'Server Release 输出目录'
Assert-PathExists -LiteralPath (Join-Path $serverRelease 'MetasequoiaImeWatchdog.exe') -Description 'Watchdog Release EXE'
Assert-PathExists -LiteralPath $mcpRelease -Description 'MCP 服务程序 Release EXE'
Assert-PathExists -LiteralPath $tsf32Release -Description '32 位 TSF Release DLL'
Assert-PathExists -LiteralPath $tsf64Release -Description '64 位 TSF Release DLL'
Assert-PathExists -LiteralPath $tsf32Pdb -Description '32 位 TSF Release PDB'
Assert-PathExists -LiteralPath $tsf64Pdb -Description '64 位 TSF Release PDB'
Assert-PathExists -LiteralPath $tsfArm64Release -Description 'Arm64X TSF Release DLL'
Assert-PathExists -LiteralPath $tsfArm64Pdb -Description 'Arm64X TSF Release PDB'
Assert-PathExists -LiteralPath $tsfArm64Host -Description "Arm64X TSF 的 $arm64HostDllName"
foreach ($hostDll in @($tsf32Host, $tsf64Host)) {
    if (-not (Test-Path -LiteralPath $hostDll -PathType Leaf)) {
        throw "缺少对应架构 TSF 的 $hostDllName"
    }
}
# TIP 的运行时 DLL。x86 输出目录里只有 32 位 TIP 的构建产物、它的宿主 DLL 和 Copy-RuntimeDependencies.ps1 放进去的 vcpkg DLL，所以其中其他 DLL 都是 TIP 的依赖。x64 输出目录与 Server、自包含的 WinUI 设置程序和语音运行时共用，整个复制会在 64 位 TIP 旁边多放一份 Windows App SDK 和 onnxruntime/sherpa；两个前缀装的是同一份 vcpkg 清单，所以 64 位 TIP 取 32 位 TIP 旁边那些名字，每一个都必须在它旁边存在。
# 别的版本的宿主 DLL 永远不是本版本 TIP 的依赖。
$editionHostDllNames = @($editionTable.editions | Where-Object { $null -ne $_.platforms.windows } | ForEach-Object { [string]$_.platforms.windows.host_dll })
$tsf32Dependencies = @(
    Get-ChildItem -LiteralPath (Split-Path -Parent $tsf32Release) -File -Filter '*.dll' |
        Where-Object { $_.Name -notin (@('MetasequoiaImeTsf.dll') + $editionHostDllNames) } |
        ForEach-Object FullName
)
$tsf64Dependencies = @(
    foreach ($dependency in $tsf32Dependencies) {
        $candidate = Join-Path (Split-Path -Parent $tsf64Release) (Split-Path -Leaf $dependency)
        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            throw "64 位 TSF 旁缺少运行时依赖 $(Split-Path -Leaf $dependency)（32 位 TSF 旁有它）：$candidate"
        }
        $candidate
    }
)
# 安装器从 tsf_dll\64 那一份把 x64 宿主 DLL 和这些依赖装进 Server 目录，包里只存一份；Server 输出里同名却内容不同的文件会被它悄悄替换，所以在替换任何旧的暂存内容之前就在这里拒绝。
foreach ($shared in @($tsf64Host) + $tsf64Dependencies) {
    $serverCopy = Join-Path $serverRelease (Split-Path -Leaf $shared)
    if ((Test-Path -LiteralPath $serverCopy -PathType Leaf) -and
        (Get-FileHash -LiteralPath $serverCopy).Hash -ne (Get-FileHash -LiteralPath $shared).Hash) {
        throw "Server 输出里的 $(Split-Path -Leaf $shared) 与 64 位 TSF 旁的同名文件不同：$serverCopy"
    }
}
# The self-contained Windows App SDK copies its own runtime executables beside the WinUI settings app, and Microsoft ships them without symbols; they are packaged, but no PDB is expected for them.
$windowsAppSdkExecutables = @('RestartAgent')
$serverExecutables = @(
    Get-ChildItem -LiteralPath $serverRelease -Recurse -File -Filter '*.exe' |
        Where-Object {
            -not (Test-PackageTestArtifact -BaseName $_.BaseName) -and
                $windowsAppSdkExecutables -notcontains $_.BaseName
        }
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
    # Check pinned bytes before staging reads them.
    $desktopResources = @(& (Join-Path $PSScriptRoot 'Get-VerifiedDesktopResources.ps1') `
        -SourceDirectory $resourceSource `
        -ManifestPath $resourceLock)
    Assert-PathExists -LiteralPath $factoryConfig -Description '出厂配置 default_config\config.default.toml'
    Assert-PathExists -LiteralPath $helpcodeSource -Description '辅助码目录'
    if (-not (Get-ChildItem -LiteralPath $helpcodeSource -File -Filter '*.txt')) {
        throw "辅助码目录中没有码表：$helpcodeSource"
    }
    Assert-PathExists -LiteralPath $englishDb -Description '英文词库数据库 msime-english.db'
    python -c @"
import sqlite3, sys
cols = list(sqlite3.connect(sys.argv[1]).execute('PRAGMA table_info(english_words)'))
names = {row[1] for row in cols}
pk = [row[1] for row in cols if row[5] > 0]
if 'weight' not in names or pk != ['word', 'display']:
    raise SystemExit('msime-english.db schema is stale; rebuild with weight and PRIMARY KEY(word, display)')
"@ $englishDb
    if ($LASTEXITCODE -ne 0) {
        throw "英文词库数据库 schema 检查失败：$englishDb"
    }
    # Validate source content before any existing package staging is removed.
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
    Write-Host '轻量模式：跳过词库、辅助码和出厂配置，刷新 TSF、Server 与 Tauri。'
    New-Item -ItemType Directory -Path $targetAppData -Force | Out-Null
}
else {
    Reset-Directory -LiteralPath $targetAppData
    $defaultConfigPath = Join-Path $targetAppData 'config.default.toml'
    # 出厂配置来自本仓库的 default_config，不依赖本机是否已安装输入法。安装脚本用 onlyifdoesntexist 生成用户 config.toml，升级不会覆盖已有方案/主题。
    Set-Content -LiteralPath $defaultConfigPath -Value $defaultConfig -Encoding utf8NoBOM -NoNewline

    $targetHelpcodes = Join-Path $targetAppData 'helpcodes'
    Reset-Directory -LiteralPath $targetHelpcodes
    Get-ChildItem -LiteralPath $helpcodeSource -File -Filter '*.txt' |
        Copy-Item -Destination $targetHelpcodes -Force
}

$targetAudios = Join-Path $targetAppData 'audios'
Copy-DirectoryContents -Source $audioSource -Destination $targetAudios
# The built-in sound packs, synthesized by scripts/generate_sound_packs.py. The Server names DataDir\sound-packs to client-core as the built-in pack root; installed packs live under DataDir\plugins, which is user state.
$targetSoundPacks = Join-Path $targetAppData 'sound-packs'
Reset-Directory -LiteralPath $targetSoundPacks
Copy-DirectoryContents -Source (Join-Path $RepoRoot 'resources/sound-packs') -Destination $targetSoundPacks

# Server Release 输出整体复制，但测试程序及其 PDB 绝不能进入安装包。其他 PDB 照常暂存在对应 EXE 旁边，供发布流程打成单独的符号包；msime_setup.iss 不把 PDB 和 .ilk 装到用户机器上。
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
# Inno recursively installs server_exe under Program Files. Keep these verified read-only sources separate from app_data and per-user writable state.
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
        -ManifestPath $resourceLock
}
# 非英文的候选释义（scripts/build_offline_glosses.py），每种目标语言一个 zh-<lang>.db，装在 resources 旁边而不是里面：校验过的资源目录必须与词库锁完全一致，Engine 也到这个同级目录找它们。可选；没有时候选释义只有英文。
$glossesSource = Join-Path $RepoRoot 'target/offline-glosses'
$glossesTarget = Join-Path $targetServer 'offline-glosses'
if (Test-Path -LiteralPath $glossesTarget) {
    Remove-Item -LiteralPath $glossesTarget -Recurse -Force
}
if (-not $editionOfflineGlosses) {
    Write-Host "版本 $Edition 不提供中文方案，不装非英文离线释义"
} elseif (-not $Light) {
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
# 粤拼、注音和笔画词库（scripts/fetch_language_dictionaries.py 下载到 target/language-dictionaries，版本由 resources/language-dictionaries.lock.json 固定），和译文一样装在 resources 旁边：host-api 在那里找到 language-dictionaries 并写进运行时配置。可选；缺少词库时对应方案显示为不可用并退回上次的中文方案，越南文和藏文不需要数据。每个词库只随它的授权文本一起分发，授权文本必须跟着数据走。设置 MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1 时，没有带上 resources/language-dictionaries.lock.json 固定的每一个词库的包会失败。
$languagesSource = Join-Path $RepoRoot 'target/language-dictionaries'
$languagesTarget = Join-Path $targetServer 'language-dictionaries'
if (Test-Path -LiteralPath $languagesTarget) {
    Remove-Item -LiteralPath $languagesTarget -Recurse -Force
}
if (-not $Light) {
    $stagedLanguages = @()
    foreach ($pair in @(@('msime-cantonese.db', 'msime-rime_cantonese_LICENSE.txt'), @('msime-zhuyin.db', 'msime-libchewing_data_LICENSE.txt'), @('msime-stroke.db', 'msime-rime_stroke_LICENSE.txt'))) {
        # 只带本版本的方案用得到的语言词库（版本表 language_dictionaries）。
        if ($languageDictionaryNames -notcontains $pair[0]) { continue }
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
        Write-Host "未找到语言词库（$languagesSource），粤拼、注音和笔画保持不可用"
    }
    # 发版要求的是本版本要带的（版本表 language_dictionaries）、resources/language-dictionaries.lock.json 又固定了的每一份词库，而不是写死的清单：还没发布的词库存在时照常装入，但不会让发版失败；发布它的那次锁更新会让它变成必需。
    if ($env:MSIME_REQUIRE_LANGUAGE_DICTIONARIES -eq '1') {
        $languagesLock = Join-Path $RepoRoot 'resources/language-dictionaries.lock.json'
        if (-not (Test-Path -LiteralPath $languagesLock -PathType Leaf)) {
            throw "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1，但 $languagesLock 不存在"
        }
        $pinnedLanguages = @((Get-Content -LiteralPath $languagesLock -Raw -Encoding UTF8 | ConvertFrom-Json).artifacts | ForEach-Object { $_.name } | Where-Object { $_ -like '*.db' })
        if ($pinnedLanguages.Count -eq 0) {
            throw "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1，但 $languagesLock 没有固定任何词库"
        }
        foreach ($pinned in $pinnedLanguages) {
            if ($languageDictionaryNames -notcontains $pinned) { continue }
            if ($stagedLanguages -notcontains $pinned) {
                throw "MSIME_REQUIRE_LANGUAGE_DICTIONARIES=1，但锁文件固定的 $pinned 没有从 $languagesSource 装入"
            }
        }
    }
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
# CI 里 Server 输出目录同时也是 x64 TIP 的构建目录。TIP 和它的符号暂存在 tsf_dll\64 下，只从版本目录加载；宿主 DLL 和 TIP 的运行时 DLL 也从同一份 tsf_dll\64 进入 Server 目录（msime_setup.iss），所以它们都不重复暂存。Build-Client.ps1 也把宿主 DLL 的 PDB 留在这里；Collect-Symbols.ps1 直接从构建输出取它打进符号包，所以它完全不暂存。
foreach ($name in @('MetasequoiaImeTsf.dll', 'MetasequoiaImeTsf.pdb', $hostDllName, 'msime_host_api.pdb') + @($tsf64Dependencies | ForEach-Object { Split-Path -Leaf $_ })) {
    $staged = Join-Path $targetServer $name
    if (Test-Path -LiteralPath $staged -PathType Leaf) { Remove-Item -LiteralPath $staged -Force }
}

# 版本声明（Edition::PACKAGE_MARKER_FILE）：MSIME.exe 和 msime-mcp.exe 从自己所在的 Server 目录读它，决定连哪个版本的 Server、用哪个状态目录。只有管理员能写 Program Files，普通进程改不了它。每个版本（包括 full）都写：full 在 Windows 上也有自己的一组名字（版本表 platforms.windows），不再是引入版本之前的那组。
$editionMarker = Join-Path $targetServer 'edition.json'
[IO.File]::WriteAllText($editionMarker, "{`"edition`": `"$Edition`"}`n", [Text.UTF8Encoding]::new($false))

Reset-Directory -LiteralPath $targetTsf
$targetTsf32 = Join-Path $targetTsf '32'
$targetTsf64 = Join-Path $targetTsf '64'
$targetTsfArm64 = Join-Path $targetTsf 'arm64'
New-Item -ItemType Directory -Path $targetTsf32, $targetTsf64, $targetTsfArm64 -Force | Out-Null
Copy-Item -LiteralPath $tsf32Release -Destination $targetTsf32 -Force
Copy-Item -LiteralPath $tsf32Pdb -Destination $targetTsf32 -Force
Copy-Item -LiteralPath $tsf64Release -Destination $targetTsf64 -Force
Copy-Item -LiteralPath $tsf64Pdb -Destination $targetTsf64 -Force
Copy-Item -LiteralPath $tsf32Host -Destination $targetTsf32 -Force
Copy-Item -LiteralPath $tsf64Host -Destination $targetTsf64 -Force
# Arm64X TIP 的 ARM64EC 那一半导入 x64 宿主，它由 tsf_dll\64 那一份装进同一个版本目录；这里只放 TIP 和 ARM64 宿主。两者都静态链接 C 运行时，没有别的运行时 DLL。
Copy-Item -LiteralPath $tsfArm64Release, $tsfArm64Pdb, $tsfArm64Host -Destination $targetTsfArm64 -Force
# Build-Client 把检查过架构的发布依赖收集到 TIP 旁边；只复制上面列出的那些，绝不复制共用 x64 目录里的其余文件。
foreach ($dependency in $tsf32Dependencies) { Copy-Item -LiteralPath $dependency -Destination $targetTsf32 -Force }
foreach ($dependency in $tsf64Dependencies) { Copy-Item -LiteralPath $dependency -Destination $targetTsf64 -Force }
Copy-Item -LiteralPath $appIcon -Destination (Join-Path $PSScriptRoot 'MetasequoiaIME.ico') -Force
# rime-ice is GPL-3.0 and requires attribution, and its content forms the bulk of msime-pinyin.db, so the
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
