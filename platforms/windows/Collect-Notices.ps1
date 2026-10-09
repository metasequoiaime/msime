[CmdletBinding()]
param(
    [Parameter(Mandatory)][string[]]$DependencyPrefixes,
    [string[]]$SupplementalNotices = @(),
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$OutputDirectory = '',
    # 声明要放进哪个产品版本的安装包（shared/contracts/editions.json 里有 Windows 段的 id），缺省是 full。
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $RepoRoot 'target/windows-notices' }
if (-not [IO.Path]::IsPathRooted($OutputDirectory)) { throw 'Notice output directory must be absolute' }
$editionTable = Get-Content -LiteralPath (Join-Path $RepoRoot 'shared/contracts/editions.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$editionEntry = @($editionTable.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
if ($editionEntry.Count -ne 1) { throw "Edition $Edition has no Windows section in shared/contracts/editions.json" }
$documents = [Collections.Generic.List[string]]::new()
$documents.Add("MSIME third-party notice collection`nThis collection is not a license-completeness or redistribution-authorization assessment. Nested third-party archives, Rust/frontend and other distribution-specific notices must also be supplied and reviewed.`n")
# 随数据和代码一起提交的声明。安装包不带手写模型：设置应用按需下载模型时一并下载它的 LGPL-2.1 许可证（HandwritingModel-LICENSE.txt，在 resources/handwriting-model.lock.json 里和模型锁在一起），所以模型的许可证随下载走，这里不收集；zinnia 本身的许可证保留，因为宿主库编进了从它移植的识别器。输入引擎是本仓库自己的 Rust crate，适用根目录的 LICENSE，安装包以 LICENSE.txt 带着它，所以没有单独的条目。
foreach ($notice in @(
    @('resources/licenses/msime-engine-dictionary-NOTICE.md', 'Dictionary data (msime-pinyin.db, msime-wubi.db, msime-english.db, msime-others.db, msime-bigram.bin, msime-trigram.bin)'),
    @('resources/helpcodes/ENGINE-NOTICE.md', 'Helpcode tables (lantian, ziranma, shouyou2_0, shouyouplus, xiaohe)'),
    @('resources/helpcodes/NOTICE.md', 'Helpcode table (jiajia)'),
    @('resources/licenses/Zinnia-LICENSE.txt', 'zinnia, whose recognizer the host library ports, BSD License'),
    @('resources/licenses/Administrative-divisions-of-China-WTFPL.txt', 'Chinese administrative divisions compiled into the host library for @ mode, modood/Administrative-divisions-of-China @ c49d495b40ac73eb1a66f6eeae5f8fd10696f035, WTFPL'),
    @('resources/licenses/libhangul-hanja-BSD-3-Clause.txt', 'Korean Hanja table compiled into the host library for Hanja conversion, libhangul data/hanja/hanja.txt @ 717409ce61524bb3d8426060a384822f21354c62, BSD-3-Clause'),
    @('resources/licenses/rime-cantonese-CC-BY-4.0.txt', 'Jyutping syllables and words of the Cantonese scheme in the host library, rime/rime-cantonese @ 259f0e48bba840c3a2e0d117539e96937f3d89bc, CC BY 4.0, with word weights from fcbond/hkcancor @ 39aeadf920e0b5ca93d0ad7792c59e740e7bdd65, CC BY 4.0'),
    @('resources/licenses/libchewing-data-LGPL-2.1.txt', 'Bopomofo syllables and words of the Zhuyin scheme in the host library, chewing/libchewing-data @ c44e81aef24b06f1509f19e1be54c99812d0c43f, LGPL-2.1-or-later, with phrase weights from openvanilla/McBopomofo @ be6564acad6c4d3265c34a2e1a872d80f9db6068 phrase.occ, MIT'),
    @('resources/licenses/rime-stroke-LGPL-3.0.txt', 'Stroke orders of the Stroke scheme in the host library, rime/rime-stroke @ 1e8fff9b9494ddec23b0cbc526bcfd8171a6fd48 (main table from CNS11643, 數位發展部，CNS11643中文標準交換碼全字庫網站，https://www.cns11643.gov.tw), LGPL-3.0'),
    @('resources/licenses/vi-MIT.txt', 'vi crate behind the Vietnamese scheme in the host library, ZeroX-DG/vi-rs 0.8.0, MIT'),
    @('resources/licenses/ewts-MIT.txt', 'ewts crate behind the Tibetan scheme in the host library, emgyrz/ewts-rs 0.1.3, MIT OR Apache-2.0 used under MIT'),
    @('platforms/windows/third_party/miniaudio/LICENSE', 'miniaudio (Server microphone capture and cue sounds)'),
    @('crates/client-core/data/opencc/LICENSE', 'OpenCC dictionaries, BYVoid/OpenCC @ 26753884f1984add422f3b0249ccee8613deaff6'))) {
    $relative = $notice[0]
    $noticePath = Join-Path $RepoRoot $relative
    if (-not (Test-Path -LiteralPath $noticePath -PathType Leaf)) {
        throw "Missing repository notice: $relative"
    }
    $content = Get-Content -LiteralPath $noticePath -Raw -Encoding UTF8
    if (-not $content) { throw "Empty repository notice: $relative" }
    $documents.Add("===== $($notice[1]) ($relative) =====`n$content`n")
}
# The on-device speech runtime Build-Client.ps1 stages beside the Server from resources/voice-runtime.lock.json: sherpa-onnx-c-api.dll (Apache-2.0), and onnxruntime.dll with onnxruntime_providers_shared.dll (MIT, plus the notices of the components ONNX Runtime bundles). The upstream archive carries no license files, so the texts pinned for the Linux package are the ones collected here; the Windows DLLs report the same ONNX Runtime release those texts name. Prepare-PackageFiles.ps1 refuses to package the runtime with a notice file that lacks these sections.
$voiceLockPath = Join-Path $RepoRoot 'resources/voice-runtime.lock.json'
if (-not (Test-Path -LiteralPath $voiceLockPath -PathType Leaf)) { throw 'Missing voice runtime lock: resources/voice-runtime.lock.json' }
$voiceVersion = "$((Get-Content -LiteralPath $voiceLockPath -Raw -Encoding UTF8 | ConvertFrom-Json).version)"
if ($voiceVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'Cannot resolve locked voice runtime version' }
foreach ($voiceNotice in @(
    @('shared/voice/third_party/sherpa-onnx/LICENSE', "sherpa-onnx $voiceVersion (sherpa-onnx-c-api.dll), Apache License 2.0"),
    @('platforms/linux/data/licenses/onnxruntime-MIT.txt', 'ONNX Runtime (onnxruntime.dll, onnxruntime_providers_shared.dll), MIT License'),
    @('platforms/linux/data/licenses/onnxruntime-ThirdPartyNotices.txt', 'ONNX Runtime third-party notices'))) {
    $relative = $voiceNotice[0]
    $noticePath = Join-Path $RepoRoot $relative
    if (-not (Test-Path -LiteralPath $noticePath -PathType Leaf)) { throw "Missing repository notice: $relative" }
    $content = Get-Content -LiteralPath $noticePath -Raw -Encoding UTF8
    if (-not $content) { throw "Empty repository notice: $relative" }
    $documents.Add("===== $($voiceNotice[1]) ($relative) =====`n$content`n")
}
$number = 0
foreach ($prefix in $DependencyPrefixes) {
    $number++
    if (-not [IO.Path]::IsPathRooted($prefix)) { throw 'Dependency prefix must be absolute' }
    $share = Join-Path $prefix 'share'
    if (-not (Test-Path -LiteralPath $share -PathType Container)) { throw 'Missing dependency license directory' }
    $licenses = @(Get-ChildItem -LiteralPath $share -Directory | Sort-Object Name | ForEach-Object {
        $copyright = Join-Path $_.FullName 'copyright'
        if (Test-Path -LiteralPath $copyright -PathType Leaf) { Get-Item -LiteralPath $copyright }
    })
    if ($licenses.Count -eq 0) { throw 'Dependency prefix has no copyright files' }
    foreach ($license in $licenses) {
        if ($license.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked copyright file refused' }
        $content = [IO.File]::ReadAllText($license.FullName)
        if ([string]::IsNullOrWhiteSpace($content)) { throw 'Empty dependency copyright file' }
        $digest = (Get-FileHash -LiteralPath $license.FullName -Algorithm SHA256).Hash
        $documents.Add("===== Dependency prefix $number/share/$($license.Directory.Name)/copyright; SHA256 $digest =====`n$content`n")
    }
}
foreach ($notice in $SupplementalNotices) {
    $content = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $notice).Path)
    if ([string]::IsNullOrWhiteSpace($content)) { throw 'Empty supplemental notice' }
    $digest = (Get-FileHash -LiteralPath $notice -Algorithm SHA256).Hash
    $documents.Add("===== Supplemental $([IO.Path]::GetFileName($notice)); SHA256 $digest =====`n$content`n")
}
# All inputs must succeed before touching an earlier generated notice bundle.
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
[IO.File]::WriteAllText((Join-Path $OutputDirectory 'THIRD_PARTY_NOTICES.txt'),
    ($documents -join "`n"), [Text.UTF8Encoding]::new($false))
Write-Output 'Collected repository and supplied dependency notices; completeness review remains required.'
