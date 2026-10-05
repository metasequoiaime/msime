$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('msime-package-' + [Guid]::NewGuid())
function Write-Fixture([string]$Relative, [string]$Text = 'fixture') {
    $path = Join-Path $fixture $Relative
    New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
    [IO.File]::WriteAllText($path, $Text)
}
try {
    $installer = Join-Path $fixture 'installer'
    New-Item -ItemType Directory -Force -Path $installer | Out-Null
    Copy-Item (Join-Path $PSScriptRoot '../Prepare-PackageFiles.ps1') $installer
    Copy-Item (Join-Path $PSScriptRoot '../Get-VerifiedDesktopResources.ps1') $installer
    Copy-Item (Join-Path $PSScriptRoot '../msime_setup.iss') $installer
    Copy-Item (Join-Path $PSScriptRoot '../config.default.toml') $installer
    Copy-Item (Join-Path $PSScriptRoot '../assets') $installer -Recurse
    # Prepare-PackageFiles.ps1 从版本表取本次打包的版本，fixture 用仓库里的那一份。
    New-Item -ItemType Directory -Force -Path (Join-Path $fixture 'shared/contracts') | Out-Null
    Copy-Item (Join-Path $PSScriptRoot '../../../../shared/contracts/editions.json') (Join-Path $fixture 'shared/contracts/editions.json')
    # 中文版本即使没有拉取模型也会读 settled-model 锁文件。
    New-Item -ItemType Directory -Force -Path (Join-Path $fixture 'resources') | Out-Null
    Copy-Item (Join-Path $PSScriptRoot '../../../../resources/settled-model.lock.json') (Join-Path $fixture 'resources/settled-model.lock.json')
    foreach ($file in @(
        'server/build-release/bin/Release/MetasequoiaImeServer.exe',
        'server/build-release/bin/Release/MetasequoiaImeServer.pdb',
        'server/build-release/bin/Release/MetasequoiaImeWatchdog.exe',
        'server/build-release/bin/Release/MetasequoiaImeWatchdog.pdb',
        'server/build-release/bin/Release/msime-mcp.exe',
        'server/build-release/bin/Release/msime-mcp.pdb',
        'server/build-release/bin/Release/MetasequoiaImeServerTests.exe',
        'server/build-release/bin/Release/MetasequoiaImeServerTests.pdb',
        'server/build-release/bin/Release/test_webview_contract.exe',
        'server/build-release/bin/Release/test_webview_contract.pdb',
        'server/build-release/bin/Release/windows-first-run.exe',
        'server/build-release/bin/Release/nested/windows-server-launch.exe',
        'server/build-release/bin/Release/nested/windows-server-launch.pdb',
        'server/build-release/bin/Release/msime-client-prepare.exe',
        'server/build-release/bin/Release/msime-client-prepare.pdb',
        'server/build-release/bin/Release/msime-client-settings.exe',
        'server/build-release/bin/Release/msime-client-settings.pdb',
        'server/build-release/bin/Release/MSIME.exe',
        'server/build-release/bin/Release/MSIME.pdb',
        'server/build-release/bin/Release/RestartAgent.exe',
        'windows/build32-release/Release/MetasequoiaImeTsf.dll',
        'windows/build32-release/Release/MetasequoiaImeTsf.pdb',
        'windows/build64-release/Release/MetasequoiaImeTsf.dll',
        'windows/build64-release/Release/MetasequoiaImeTsf.pdb',
        'THIRD_PARTY_NOTICES.txt',
        'LICENSE',
        'target/release/msime-desktop.exe',
        'target/handwriting-model/handwriting-zh_CN.model',
        'target/handwriting-model/HandwritingModel-LICENSE.txt',
        'target/language-dictionaries/msime-zhuyin.db',
        'target/language-dictionaries/msime-libchewing_data_LICENSE.txt',
        'target/language-dictionaries/msime-stroke.db',
        'target/language-dictionaries/msime-rime_stroke_LICENSE.txt',
        'resources/helpcodes/helpcode.txt',
        'resources/helpcodes/NOTICE.md',
        'resources/sound-packs/default/plugin.toml',
        'resources/sound-packs/default/key.wav',
        'target/offline-glosses/zh-fr.db',
        'target/offline-glosses/offline-glosses-NOTICE.txt'
    )) { Write-Fixture $file }
    Write-Fixture 'windows/build32-release/Release/msime_host_api.dll' 'synthetic x86 host'
    Write-Fixture 'windows/build64-release/Release/msime_host_api.dll' 'synthetic x64 host'
    Write-Fixture 'windows/build32-release/Release/synthetic-runtime.dll' 'synthetic x86 dependency'
    Write-Fixture 'windows/build64-release/Release/synthetic-runtime.dll' 'synthetic x64 dependency'
    $english = Join-Path $fixture 'target/desktop-resources/msime-english.db'
    New-Item -ItemType Directory -Force (Split-Path -Parent $english) | Out-Null
    python -c "import sqlite3,sys; sqlite3.connect(sys.argv[1]).execute('CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER,PRIMARY KEY(word,display))')" $english
    if ($LASTEXITCODE -ne 0) { throw 'Failed to create packaging fixture' }
    $artifacts = @(
        foreach ($name in @('msime-pinyin.db', 'msime-english.db', 'msime-scowl_Copyright.txt', 'msime-others.db',
                            'msime-japanese.dat', 'msime-mozc_dictionary_oss_README.txt', 'msime-mozc_LICENSE.txt',
                            'msime-dictionary-manifest.json')) {
            if ($name -ne 'msime-english.db') { Write-Fixture "target/desktop-resources/$name" "synthetic pinned $name" }
            $path = Join-Path $fixture "target/desktop-resources/$name"
            @{ name = $name; size = (Get-Item $path).Length; sha256 = (Get-FileHash $path).Hash.ToLowerInvariant() }
        }
    )
    Write-Fixture 'resources/desktop-dictionary.lock.json' (@{
        source_commit = ('a' * 40); artifacts = $artifacts
    } | ConvertTo-Json -Depth 5)
    Write-Fixture 'target/desktop-resources/unlisted-private-file.txt' 'synthetic excluded data'
    Write-Fixture 'server/build-release/bin/Release/resources/stale.txt' 'synthetic stale bundle'
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -TargetVersion '2026.9.1'
    foreach ($artifact in $artifacts) {
        $path = Join-Path $installer "server_exe/resources/$($artifact.name)"
        if ((Get-FileHash $path).Hash -ne $artifact.sha256) { throw 'Packaged resource hash mismatch' }
    }
    if (Test-Path (Join-Path $installer 'server_exe/resources/unlisted-private-file.txt')) {
        throw 'Packaged an unlisted resource'
    }
    if (Test-Path (Join-Path $installer 'server_exe/resources/stale.txt')) {
        throw 'Packaged unverified native build resources'
    }
    $pinned = Join-Path $fixture 'target/desktop-resources/msime-pinyin.db'
    $originalPinned = [IO.File]::ReadAllText($pinned)
    foreach ($bad in @('short', ('x' * $originalPinned.Length))) {
        [IO.File]::WriteAllText($pinned, $bad)
        $rejected = $false
        try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $true }
        if (-not $rejected) { throw 'Invalid pinned resource accepted' }
        if ([IO.File]::ReadAllText((Join-Path $installer 'server_exe/resources/msime-pinyin.db')) -ne $originalPinned) {
            throw 'Failed resource preflight damaged previous staging'
        }
    }
    [IO.File]::WriteAllText($pinned, $originalPinned)
    foreach ($file in @('tsf_dll/32/MetasequoiaImeTsf.dll', 'tsf_dll/32/MetasequoiaImeTsf.pdb',
                         'tsf_dll/64/MetasequoiaImeTsf.dll', 'tsf_dll/64/MetasequoiaImeTsf.pdb',
                         'server_exe/MetasequoiaImeServer.pdb',
                         'server_exe/MetasequoiaImeWatchdog.exe',
                         'server_exe/MetasequoiaImeWatchdog.pdb',
                         'server_exe/msime-mcp.exe',
                         'server_exe/msime-mcp.pdb',
                         'server_exe/msime-client-settings.exe',
                         'server_exe/msime-client-settings.pdb',
                         'server_exe/MSIME.exe',
                         'server_exe/MSIME.pdb',
                         'server_exe/msime-client-prepare.exe',
                         'server_exe/msime-client-prepare.pdb',
                         'server_exe/RestartAgent.exe',
                         'server_exe/handwriting/handwriting-zh_CN.model',
                         'server_exe/handwriting/HandwritingModel-LICENSE.txt',
                         'server_exe/offline-glosses/zh-fr.db',
                         'server_exe/offline-glosses/offline-glosses-NOTICE.txt',
                         'app_data/helpcodes/helpcode.txt',
                         'app_data/sound-packs/default/plugin.toml', 'app_data/sound-packs/default/key.wav',
                         'THIRD_PARTY_NOTICES.txt', 'LICENSE.txt')) {
        if (-not (Test-Path (Join-Path $installer $file))) { throw "Missing packaged file: $file" }
    }
    if (Test-Path (Join-Path $installer 'app_data/helpcodes/NOTICE.md')) { throw 'Staged a helpcode notice as a table' }
    # The Zhuyin and Stroke dictionaries travel beside resources with their licences; the absent Cantonese one leaves that scheme unavailable, and a dictionary without its licence is refused.
    foreach ($name in @('msime-zhuyin.db', 'msime-libchewing_data_LICENSE.txt', 'msime-stroke.db', 'msime-rime_stroke_LICENSE.txt')) {
        if (-not (Test-Path (Join-Path $installer "server_exe/language-dictionaries/$name"))) { throw "Missing language dictionary file: $name" }
    }
    if (Test-Path (Join-Path $installer 'server_exe/language-dictionaries/msime-cantonese.db')) { throw 'Packaged a Cantonese dictionary that was not provided' }
    Write-Fixture 'target/language-dictionaries/msime-cantonese.db'
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $_.Exception.Message -match 'rime_cantonese_LICENSE' }
    if (-not $rejected) { throw 'A language dictionary without its licence was accepted' }
    Remove-Item (Join-Path $fixture 'target/language-dictionaries/msime-cantonese.db') -Force
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture
    foreach ($testFile in @(
        'server_exe/MetasequoiaImeServerTests.exe',
        'server_exe/MetasequoiaImeServerTests.pdb',
        'server_exe/test_webview_contract.exe',
        'server_exe/test_webview_contract.pdb',
        'server_exe/windows-first-run.exe',
        'server_exe/nested/windows-server-launch.exe',
        'server_exe/nested/windows-server-launch.pdb'
    )) {
        if (Test-Path (Join-Path $installer $testFile)) { throw "Packaged a test file: $testFile" }
    }
    $serverPdbFixture = Join-Path $fixture 'server/build-release/bin/Release/MetasequoiaImeServer.pdb'
    Remove-Item $serverPdbFixture -Force
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $_.Exception.Message -match 'PDB' }
    if (-not $rejected) { throw 'Missing production PDB was accepted' }
    [IO.File]::WriteAllText($serverPdbFixture, 'fixture')
    # 发布时 Server 输出目录同时作为 x64 TIP 目录传入，所以自包含的 Windows App SDK 和语音运行时就在 64 位 TIP 旁边。tsf_dll\64 只取 TIP、它的宿主 DLL 和 32 位 TIP 也有的那些依赖；Server 暂存目录去掉 TIP 以及 msime_setup.iss 从 tsf_dll\64 装进 Server 目录的那些文件。
    $sharedOutput = 'server/build-release/bin/Release'
    $sharedTipFiles = @('MetasequoiaImeTsf.dll', 'MetasequoiaImeTsf.pdb', 'msime_host_api.dll', 'synthetic-runtime.dll')
    foreach ($name in $sharedTipFiles) {
        Copy-Item (Join-Path $fixture "windows/build64-release/Release/$name") (Join-Path $fixture $sharedOutput)
    }
    $serverOnlyFiles = @('Microsoft.UI.Xaml.dll', 'Microsoft.WindowsAppRuntime.dll', 'sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll')
    foreach ($name in $serverOnlyFiles) { Write-Fixture "$sharedOutput/$name" "server-only $name" }
    # Build-Client.ps1 把宿主 DLL 的 PDB 留在这个目录里供发布的符号包使用；它从不暂存。
    Write-Fixture "$sharedOutput/msime_host_api.pdb" 'synthetic x64 host symbols'
    $rootNotices = Join-Path $fixture 'THIRD_PARTY_NOTICES.txt'
    [IO.File]::WriteAllText($rootNotices, 'fixture sherpa-onnx ONNX Runtime')
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Tsf64ReleaseDirectory $sharedOutput
    $tsf64Staged = @(Get-ChildItem -LiteralPath (Join-Path $installer 'tsf_dll/64') -File | ForEach-Object Name | Sort-Object)
    if (($tsf64Staged -join ',') -ne (($sharedTipFiles | Sort-Object) -join ',')) {
        throw "tsf_dll/64 is not limited to the TIP, its host DLL and its dependencies: $($tsf64Staged -join ', ')"
    }
    foreach ($pattern in @('Microsoft.*', 'onnxruntime*', 'sherpa*')) {
        if (@(Get-ChildItem -LiteralPath (Join-Path $installer 'tsf_dll/64') -File -Filter $pattern).Count -ne 0) {
            throw "tsf_dll/64 carries Server-only files: $pattern"
        }
    }
    foreach ($name in $sharedTipFiles) {
        if (Test-Path (Join-Path $installer "server_exe/$name")) { throw "server_exe duplicates tsf_dll/64: $name" }
    }
    if (Test-Path (Join-Path $installer 'server_exe/msime_host_api.pdb')) { throw 'server_exe stages the host DLL PDB' }
    foreach ($name in $serverOnlyFiles) {
        if (-not (Test-Path (Join-Path $installer "server_exe/$name"))) { throw "server_exe lost a Server file: $name" }
    }
    # Server 输出里某个共用文件与 64 位 TIP 旁边的那份不同时，安装会用后者替换它，所以在暂存内容改动之前就拒绝。
    [IO.File]::WriteAllText((Join-Path $fixture "$sharedOutput/synthetic-runtime.dll"), 'different x64 dependency')
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $_.Exception.Message -match 'synthetic-runtime\.dll' }
    if (-not $rejected) { throw 'Conflicting Server and TSF copies of a shared DLL were accepted' }
    if (-not (Test-Path (Join-Path $installer 'server_exe/Microsoft.UI.Xaml.dll'))) { throw 'Shared DLL conflict damaged previous staging' }
    foreach ($name in $sharedTipFiles + $serverOnlyFiles + @('msime_host_api.pdb')) { Remove-Item -LiteralPath (Join-Path $fixture "$sharedOutput/$name") }
    [IO.File]::WriteAllText($rootNotices, 'fixture')
    # 32 位 TIP 旁边的每个 DLL 都必须在 64 位 TIP 旁边有对应的 x64 版本。
    $x64Dependency = Join-Path $fixture 'windows/build64-release/Release/synthetic-runtime.dll'
    Remove-Item -LiteralPath $x64Dependency
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $_.Exception.Message -match 'synthetic-runtime\.dll' }
    if (-not $rejected) { throw 'Missing x64 TIP dependency was accepted' }
    [IO.File]::WriteAllText($x64Dependency, 'synthetic x64 dependency')
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture
    $database = Join-Path $installer 'app_data/previous-staging.txt'
    [IO.File]::WriteAllText($database, 'preserved user data')
    foreach ($arch in @('32', '64')) {
        $expected = if ($arch -eq '32') { 'synthetic x86 host' } else { 'synthetic x64 host' }
        $packagedHost = Join-Path $installer "tsf_dll/$arch/msime_host_api.dll"
        $expectedDependency = if ($arch -eq '32') { 'synthetic x86 dependency' } else { 'synthetic x64 dependency' }
        if ([IO.File]::ReadAllText((Join-Path $installer "tsf_dll/$arch/synthetic-runtime.dll")) -ne $expectedDependency) {
            throw 'Full package lost matching runtime dependency'
        }
        if ([IO.File]::ReadAllText($packagedHost) -ne $expected) { throw 'TSF Host DLL architecture mapping mismatch' }
        $sourceHost = Join-Path $fixture "windows/build$arch-release/Release/msime_host_api.dll"
        Remove-Item -LiteralPath $sourceHost
        foreach ($lightMode in @($false, $true)) {
            $rejected = $false
            try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light:$lightMode }
            catch { $rejected = $_.Exception.Message -match 'msime_host_api.dll' }
            if (-not $rejected) { throw 'Missing TSF Host DLL accepted' }
            if ([IO.File]::ReadAllText($database) -ne 'preserved user data' -or
                [IO.File]::ReadAllText($packagedHost) -ne $expected) { throw 'Missing Host DLL damaged previous staging' }
        }
        [IO.File]::WriteAllText($sourceHost, $expected)
    }
    $watchdog = Join-Path $fixture 'server/build-release/bin/Release/MetasequoiaImeWatchdog.exe'
    Remove-Item -LiteralPath $watchdog
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light } catch { $rejected = $_.Exception.Message -match 'Watchdog' }
    if (-not $rejected) { throw 'Missing watchdog was accepted' }
    if ([IO.File]::ReadAllText($database) -ne 'preserved user data') { throw 'Missing watchdog damaged previous staging' }
    [IO.File]::WriteAllText($watchdog, 'fixture')
    $desktop = Join-Path $fixture 'target/release/msime-desktop.exe'
    $nativeDesktop = Join-Path $fixture 'server/build-release/bin/Release/msime-client-settings.exe'
    Remove-Item -LiteralPath $nativeDesktop
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light } catch { $rejected = $_.Exception.Message -match 'WinUI' }
    if (-not $rejected) { throw 'Missing WinUI settings shell was accepted' }
    if ([IO.File]::ReadAllText($database) -ne 'preserved user data') { throw 'Missing shell damaged previous staging' }
    [IO.File]::WriteAllText($nativeDesktop, 'fixture')
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -TsfDirectory windows -ServerDirectory server -NoticesDirectory . -Light
    if ([IO.File]::ReadAllText($database) -ne 'preserved user data') { throw 'Light package replaced dictionary data' }
    if (-not (Test-Path (Join-Path $installer 'server_exe/msime-client-settings.exe'))) { throw 'Light package lost WinUI settings shell' }
    foreach ($arch in @('32', '64')) {
        $expected = if ($arch -eq '32') { 'synthetic x86 host' } else { 'synthetic x64 host' }
        if ([IO.File]::ReadAllText((Join-Path $installer "tsf_dll/$arch/msime_host_api.dll")) -ne $expected) {
            throw 'Light package lost matching TSF Host DLL'
        }
        $expectedDependency = if ($arch -eq '32') { 'synthetic x86 dependency' } else { 'synthetic x64 dependency' }
        if ([IO.File]::ReadAllText((Join-Path $installer "tsf_dll/$arch/synthetic-runtime.dll")) -ne $expectedDependency) {
            throw 'Light package lost matching runtime dependency'
        }
    }
    if (-not (Test-Path (Join-Path $installer 'server_exe/msime-client-prepare.exe'))) { throw 'Light package lost preparation tool' }
    foreach ($testFile in @('windows-first-run.exe', 'nested/windows-server-launch.exe', 'nested/windows-server-launch.pdb')) {
        if (Test-Path (Join-Path $installer "server_exe/$testFile")) { throw 'Light package contains a Client test artifact' }
    }
    if (Test-Path (Join-Path $installer 'server_exe/resources')) { throw 'Light package unexpectedly carries dictionaries' }
    Write-Fixture 'custom build/shell.exe' 'synthetic alternate shell'
    Write-Fixture 'server/build-release/bin/Release/msime-client-settings.exe' 'synthetic native shell'
    Write-Fixture 'server/build-release/bin/Release/msime-client-settings.pdb' 'synthetic native symbols'
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light
    if ([IO.File]::ReadAllText((Join-Path $installer 'server_exe/msime-client-settings.exe')) -ne 'synthetic native shell') {
        throw 'Native WinUI output did not take precedence over old Cargo output'
    }
    Remove-Item -LiteralPath $desktop
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light
    if ([IO.File]::ReadAllText((Join-Path $installer 'server_exe/msime-client-settings.pdb')) -ne 'synthetic native symbols') {
        throw 'Native WinUI shell symbols were not packaged'
    }
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -DesktopExecutable 'target/release/msime-desktop.exe' }
    catch { $rejected = $_.Exception.Message -match 'WinUI' }
    if (-not $rejected) { throw 'Missing explicit shell silently fell back to native output' }
    [IO.File]::WriteAllText($desktop, 'fixture')
    foreach ($shellPath in @('custom build/shell.exe', (Join-Path $fixture 'custom build/shell.exe'))) {
        & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -DesktopExecutable $shellPath
        if ([IO.File]::ReadAllText((Join-Path $installer 'server_exe/msime-client-settings.exe')) -ne 'synthetic alternate shell') {
            throw 'Explicit settings shell path was not packaged'
        }
        if ([IO.File]::ReadAllText($database) -ne 'preserved user data') { throw 'Shell override replaced dictionary data' }
        if (Test-Path (Join-Path $installer 'server_exe/msime-client-settings.pdb')) {
            throw 'Explicit shell inherited unrelated native symbols'
        }
    }
    foreach ($name in @('handwriting-zh_CN.model', 'HandwritingModel-LICENSE.txt')) {
        if (-not (Test-Path (Join-Path $installer "server_exe/handwriting/$name"))) {
            throw "Light package lost handwriting resource: $name"
        }
    }
    $notice = Join-Path $fixture 'target/handwriting-model/HandwritingModel-LICENSE.txt'
    Remove-Item -LiteralPath $notice
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $true }
    if (-not $rejected) { throw 'Missing handwriting license was accepted' }
    if ([IO.File]::ReadAllText($database) -ne 'preserved user data') { throw 'Missing license damaged previous staging' }
    [IO.File]::WriteAllText($notice, 'fixture')
    $factory = Join-Path $installer 'config.default.toml'
    $originalFactory = [IO.File]::ReadAllText($factory)
    foreach ($invalid in @(
        ('schema = "invalid"' + "`n" + 'theme_mode = "system"'),
        ('schema = "quanpin"' + "`n" + 'theme_mode = "invalid"'),
        ('schema = "quanpin"' + "`n" + 'theme_mode = "system"' + "`n" + 'diagnostic_log = true')
    )) {
        [IO.File]::WriteAllText($factory, $invalid)
        $rejected = $false
        try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture } catch { $rejected = $true }
        if (-not $rejected) { throw 'Invalid factory configuration was accepted' }
        if (-not (Test-Path $database) -or [IO.File]::ReadAllText($database) -ne 'preserved user data') { throw 'Invalid factory configuration damaged previous staging' }
    }
    [IO.File]::WriteAllText($factory, $originalFactory)
    foreach ($mapping in @(@('32', 'x86'), @('64', 'x64'))) {
        $native = Join-Path $fixture "target/windows-full/$($mapping[1])/bin"
        New-Item -ItemType Directory -Force $native | Out-Null
        foreach ($name in @('MetasequoiaImeTsf.dll', 'MetasequoiaImeTsf.pdb')) {
            Copy-Item (Join-Path $fixture "windows/build$($mapping[0])-release/Release/$name") $native
        }
        [IO.File]::WriteAllText((Join-Path $native 'msime_host_api.dll'), "native $($mapping[1]) host")
    }
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory 'server/build-release/bin/Release'
    foreach ($mapping in @(@('32', 'x86'), @('64', 'x64'))) {
        if ([IO.File]::ReadAllText((Join-Path $installer "tsf_dll/$($mapping[0])/msime_host_api.dll")) -ne "native $($mapping[1]) host") {
            throw 'Native build directory default was not selected'
        }
    }
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light `
        -ServerReleaseDirectory 'server/build-release/bin/Release' `
        -Tsf32ReleaseDirectory 'windows/build32-release/Release' -Tsf64ReleaseDirectory 'windows/build64-release/Release'
    foreach ($mapping in @(@('32', 'x86'), @('64', 'x64'))) {
        if ([IO.File]::ReadAllText((Join-Path $installer "tsf_dll/$($mapping[0])/msime_host_api.dll")) -ne "synthetic $($mapping[1]) host") {
            throw 'Explicit TSF directory override was ignored'
        }
    }
    Write-Fixture 'target/windows-notices/THIRD_PARTY_NOTICES.txt' 'synthetic collected notices'
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory 'server/build-release/bin/Release'
    if ([IO.File]::ReadAllText((Join-Path $installer 'THIRD_PARTY_NOTICES.txt')) -ne 'synthetic collected notices') {
        throw 'Collected notice default not selected'
    }
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory 'server/build-release/bin/Release' -NoticesDirectory .
    if ([IO.File]::ReadAllText((Join-Path $installer 'THIRD_PARTY_NOTICES.txt')) -ne 'fixture') {
        throw 'Explicit notice directory override ignored'
    }
    # The on-device speech runtime rides beside the Server: all three libraries, or none.
    $voiceRuntimeLibraries = @('sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll')
    $serverOutput = 'server/build-release/bin/Release'
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput
    foreach ($library in $voiceRuntimeLibraries) {
        if (Test-Path (Join-Path $installer "server_exe/$library")) { throw "Packaged a voice runtime that was never built: $library" }
    }
    foreach ($library in $voiceRuntimeLibraries) {
        Write-Fixture "target/voice-runtime/windows-x64/$library" "fetched $library"
    }
    Write-Fixture 'target/voice-runtime/windows-x64/.archive/runtime.tar.bz2' 'synthetic cached archive'
    # The runtime's licenses must be in the notices it ships with: the collection above predates them and is refused, leaving the previous staging alone.
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput }
    catch { $rejected = $_.Exception.Message -match 'sherpa-onnx, ONNX Runtime' }
    if (-not $rejected) { throw 'Voice runtime packaged without its license notices' }
    if (Test-Path (Join-Path $installer 'server_exe/sherpa-onnx-c-api.dll')) { throw 'Refused notices still staged the voice runtime' }
    if ([IO.File]::ReadAllText((Join-Path $installer 'THIRD_PARTY_NOTICES.txt')) -ne 'synthetic collected notices') {
        throw 'Refused notices replaced the staged notices'
    }
    Write-Fixture 'target/windows-notices/THIRD_PARTY_NOTICES.txt' 'synthetic ONNX Runtime notice only'
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput }
    catch { $rejected = $_.Exception.Message -match 'sherpa-onnx' -and $_.Exception.Message -notmatch 'ONNX Runtime）' }
    if (-not $rejected) { throw 'Voice runtime packaged without the sherpa-onnx license' }
    $voiceNotices = "synthetic collected notices`n===== sherpa-onnx 1.13.8 (sherpa-onnx-c-api.dll), Apache License 2.0 =====`n===== ONNX Runtime (onnxruntime.dll, onnxruntime_providers_shared.dll), MIT License ====="
    Write-Fixture 'target/windows-notices/THIRD_PARTY_NOTICES.txt' $voiceNotices
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput
    if ([IO.File]::ReadAllText((Join-Path $installer 'THIRD_PARTY_NOTICES.txt')) -ne $voiceNotices) {
        throw 'Voice runtime notices not staged'
    }
    foreach ($library in $voiceRuntimeLibraries) {
        if ([IO.File]::ReadAllText((Join-Path $installer "server_exe/$library")) -ne "fetched $library") {
            throw "Fetched voice runtime not packaged beside the Server: $library"
        }
    }
    if (Test-Path (Join-Path $installer 'server_exe/.archive')) { throw 'Packaged the voice runtime download cache' }
    foreach ($library in $voiceRuntimeLibraries) {
        Write-Fixture "custom runtime/$library" "custom $library"
    }
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput `
        -VoiceRuntimeDirectory (Join-Path $fixture 'custom runtime')
    foreach ($library in $voiceRuntimeLibraries) {
        if ([IO.File]::ReadAllText((Join-Path $installer "server_exe/$library")) -ne "custom $library") {
            throw "Explicit voice runtime directory ignored: $library"
        }
    }
    # Build-Client.ps1 stages the runtime into the Server output, which then wins over the fetch directory.
    foreach ($library in $voiceRuntimeLibraries) {
        Write-Fixture "$serverOutput/$library" "built $library"
    }
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput
    foreach ($library in $voiceRuntimeLibraries) {
        if ([IO.File]::ReadAllText((Join-Path $installer "server_exe/$library")) -ne "built $library") {
            throw "Server output voice runtime not packaged: $library"
        }
    }
    foreach ($partial in @($serverOutput, 'target/voice-runtime/windows-x64')) {
        if ($partial -eq 'target/voice-runtime/windows-x64') {
            # What the first pass left of the Server output copy, so the fetch directory is consulted.
            Remove-Item -LiteralPath (Join-Path $fixture "$serverOutput/sherpa-onnx-c-api.dll"), (Join-Path $fixture "$serverOutput/onnxruntime_providers_shared.dll")
        }
        Remove-Item -LiteralPath (Join-Path $fixture "$partial/onnxruntime.dll")
        $before = [IO.File]::ReadAllText((Join-Path $installer 'server_exe/sherpa-onnx-c-api.dll'))
        $rejected = $false
        try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Light -ServerReleaseDirectory $serverOutput }
        catch { $rejected = $_.Exception.Message -match 'onnxruntime\.dll' -and $_.Exception.Message -notmatch 'sherpa-onnx-c-api' }
        if (-not $rejected) { throw "Partial voice runtime accepted: $partial" }
        if ([IO.File]::ReadAllText((Join-Path $installer 'server_exe/sherpa-onnx-c-api.dll')) -ne $before -or
            [IO.File]::ReadAllText($database) -ne 'preserved user data') {
            throw 'Partial voice runtime damaged previous staging'
        }
    }
    # 版本：五笔版按自己的资源锁只带它的词库、不带语言词库，host DLL 用版本表里的名字，Server 目录里放版本声明；再打一次 full，声明就不在了。
    foreach ($partial in @($serverOutput, 'target/voice-runtime/windows-x64')) {
        foreach ($library in $voiceRuntimeLibraries) {
            Remove-Item -LiteralPath (Join-Path $fixture "$partial/$library") -ErrorAction SilentlyContinue
        }
    }
    Write-Fixture 'windows/build32-release/Release/msime_host_api_wubi.dll' 'synthetic x86 wubi host'
    Write-Fixture 'windows/build64-release/Release/msime_host_api_wubi.dll' 'synthetic x64 wubi host'
    $wubiArtifacts = @($artifacts | Where-Object { $_.name -in @('msime-pinyin.db', 'msime-wubi.db', 'msime-english.db', 'msime-scowl_Copyright.txt', 'msime-others.db', 'msime-dictionary-manifest.json') })
    Write-Fixture 'resources/editions/wubi.lock.json' (@{
        source_commit = ('a' * 40); artifacts = $wubiArtifacts
    } | ConvertTo-Json -Depth 5)
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Edition wubi
    $declared = Get-Content -LiteralPath (Join-Path $installer 'server_exe/edition.json') -Raw | ConvertFrom-Json
    if ($declared.edition -ne 'wubi') { throw 'Edition package declaration missing or wrong' }
    $staged = @(Get-ChildItem -LiteralPath (Join-Path $installer 'server_exe/resources') -File | ForEach-Object Name | Sort-Object)
    if (($staged -join ',') -ne ((@($wubiArtifacts | ForEach-Object { $_.name }) | Sort-Object) -join ',')) {
        throw "Edition resources do not follow its lock: $($staged -join ', ')"
    }
    if (Test-Path (Join-Path $installer 'server_exe/language-dictionaries')) { throw 'Edition without Zhuyin packaged the Zhuyin dictionary' }
    foreach ($arch in @('32', '64')) {
        if (-not (Test-Path (Join-Path $installer "tsf_dll/$arch/msime_host_api_wubi.dll")) -or
            (Test-Path (Join-Path $installer "tsf_dll/$arch/msime_host_api.dll"))) {
            throw "Edition host DLL not packaged under its own name ($arch)"
        }
    }
    # 五笔版提供中文方案，手写模型和非英文离线释义照常装。
    foreach ($file in @('server_exe/handwriting/handwriting-zh_CN.model', 'server_exe/offline-glosses/zh-fr.db')) {
        if (-not (Test-Path (Join-Path $installer $file))) { throw "Chinese edition lost $file" }
    }
    # 越南文版没有中文方案（版本表 features.handwriting 和 features.offline_glosses 为 false）：手写模型和非英文离线释义都不装，即使构建目录里有它们。
    Write-Fixture 'windows/build32-release/Release/msime_host_api_vietnamese.dll' 'synthetic x86 vietnamese host'
    Write-Fixture 'windows/build64-release/Release/msime_host_api_vietnamese.dll' 'synthetic x64 vietnamese host'
    $vietnameseArtifacts = @($artifacts | Where-Object { $_.name -in @('msime-english.db', 'msime-scowl_Copyright.txt', 'msime-others.db', 'msime-dictionary-manifest.json') })
    Write-Fixture 'resources/editions/vietnamese.lock.json' (@{
        source_commit = ('a' * 40); artifacts = $vietnameseArtifacts
    } | ConvertTo-Json -Depth 5)
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Edition vietnamese
    foreach ($absent in @('server_exe/handwriting', 'server_exe/offline-glosses', 'server_exe/language-dictionaries')) {
        if (Test-Path (Join-Path $installer $absent)) { throw "Edition without a Chinese scheme packaged $absent" }
    }
    $rejected = $false
    try { & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -Edition klingon }
    catch { $rejected = $_.Exception.Message -match 'klingon' }
    if (-not $rejected) { throw 'Unknown edition accepted' }
    & (Join-Path $installer 'Prepare-PackageFiles.ps1') -RepoRoot $fixture -ServerReleaseDirectory $serverOutput
    if (Test-Path (Join-Path $installer 'server_exe/edition.json')) { throw 'Full package carries an edition declaration' }
    Write-Host 'Full/light package contracts, provenance, exclusions and failure staging and the per-edition packages passed'
} finally {
    if (Test-Path $fixture) { Remove-Item $fixture -Recurse -Force }
}
