# Test real orchestration with command probes; no compiler, signer or installer runs.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('msime-client-build-' + [Guid]::NewGuid())
$originalLocation = (Get-Location).Path
$originalPrefix = $env:CMAKE_PREFIX_PATH
$originalTarget = $env:CARGO_TARGET_DIR
$originalDebug = $env:CARGO_PROFILE_RELEASE_DEBUG
$originalArm64Flags = $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS
. (Join-Path $PSScriptRoot 'pe_fixture.ps1')
try {
    $desktopSymbols = Join-Path $fixture 'target/x86_64-pc-windows-msvc/release/msime_desktop.pdb'
    New-Item -ItemType Directory -Force (Split-Path -Parent $desktopSymbols) | Out-Null
    [IO.File]::WriteAllText($desktopSymbols, 'synthetic symbols')
    $mcpSymbols = Join-Path $fixture 'target/x86_64-pc-windows-msvc/release/msime_mcp.pdb'
    [IO.File]::WriteAllText($mcpSymbols, 'synthetic symbols')
    $settingsSymbols = Join-Path $fixture 'target/windows-full/x64/bin/msime-client-settings.pdb'
    New-Item -ItemType Directory -Force (Split-Path -Parent $settingsSymbols) | Out-Null
    [IO.File]::WriteAllText($settingsSymbols, 'synthetic WinUI symbols')
    foreach ($arch in @('x86', 'x64')) {
        foreach ($dll in @('MetasequoiaImeTsf.dll', 'msime_host_api.dll')) {
            Write-PEFixture (Join-Path $fixture "target/windows-full/$arch/bin/$dll") $arch dll
        }
    }
    Write-PEFixture (Join-Path $fixture 'target/windows-full/arm64/bin/MetasequoiaImeTsf.dll') arm64x dll
    Write-PEFixture (Join-Path $fixture 'target/windows-full/arm64/bin/msime_host_api_arm64.dll') arm64 dll
    foreach ($exe in @('MetasequoiaImeServer.exe', 'MetasequoiaImeWatchdog.exe', 'msime-client-prepare.exe',
        'msime-mcp.exe', 'msime-client-settings.exe', 'MSIME.exe')) {
        Write-PEFixture (Join-Path $fixture "target/windows-full/x64/bin/$exe") x64 exe
    }
    $voiceRuntimeLibraries = @('sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll')
    foreach ($dll in $voiceRuntimeLibraries) {
        Write-PEFixture (Join-Path $fixture "target/windows-full/x64/bin/$dll") x64 dll
    }
    foreach ($relative in @('Cargo.toml', 'crates/engine/Cargo.toml',
        'platforms/windows/CMakeLists.txt', 'platforms/windows/tsf/CMakeLists.txt',
        'platforms/windows/settings/MSIME.Settings.vcxproj', 'apps/desktop/package.json',
        'scripts/fetch_voice_runtime.py')) {
        $path = Join-Path $fixture $relative
        New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
        [IO.File]::WriteAllText($path, 'synthetic')
    }
    # Build-Client.ps1 按版本表找本次构建的版本，fixture 用仓库里的那一份。
    $editionTable = Join-Path $fixture 'shared/contracts/editions.json'
    New-Item -ItemType Directory -Force (Split-Path $editionTable) | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../../../../shared/contracts/editions.json') -Destination $editionTable
    $x64 = Join-Path $fixture 'deps x64'
    $x86 = Join-Path $fixture 'deps x86'
    New-Item -ItemType Directory $x64, $x86 | Out-Null
    Write-PEFixture (Join-Path $x64 'bin/synthetic-runtime.dll') x64 dll
    Write-PEFixture (Join-Path $x86 'bin/synthetic-runtime.dll') x86 dll
    function global:Invoke-ClientCommandProbe {
        param([string]$Name, [object[]]$Values)
        $global:ClientBuildCalls.Add(@{ Name = $Name; Values = $Values; Prefix = $env:CMAKE_PREFIX_PATH })
        $global:LASTEXITCODE = if ($global:ClientBuildCalls.Count -eq $global:ClientBuildFailAt) { 19 } else { 0 }
    }
    function global:cargo { Invoke-ClientCommandProbe cargo $args }
    function global:cmake { Invoke-ClientCommandProbe cmake $args }
    function global:pnpm { Invoke-ClientCommandProbe pnpm $args }
    function global:python { Invoke-ClientCommandProbe python $args }
    function global:msbuild { Invoke-ClientCommandProbe msbuild $args }
    function global:lib { Invoke-ClientCommandProbe lib $args }
    $entry = Join-Path $PSScriptRoot '../../Build-Client.ps1'
    $global:ClientBuildCalls = [Collections.Generic.List[object]]::new()
    $global:ClientBuildFailAt = 0
    & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 -TargetVersion '2026.9.1'
    $count = $global:ClientBuildCalls.Count
    $desktopCall = @($global:ClientBuildCalls | Where-Object { $_.Name -eq 'pnpm' -and $_.Values -contains 'tauri' })[0]
    $desktopArgs = $desktopCall.Values
    $configIndex = [Array]::IndexOf($desktopArgs, '--config')
    if ($configIndex -lt 0 -or (Get-Content -LiteralPath $desktopArgs[$configIndex + 1] -Raw | ConvertFrom-Json).version -ne '2026.9.1') {
        throw 'Tauri version override missing or incorrect'
    }
    foreach ($arch in @('x86', 'x64')) {
        & (Join-Path $PSScriptRoot '../../Test-PortableExecutable.ps1') `
            -LiteralPath (Join-Path $fixture "target/windows-full/$arch/bin/synthetic-runtime.dll") -Architecture $arch -Kind dll
    }
    if ($count -ne 30) { throw "Unexpected build stage count: $count" }
    # host-api 的 PDB 在每个架构里紧跟着 DLL 复制进 bin：之后的 MCP 与桌面构建共用同一个 target 目录，可能重编 host-api 并以同名覆盖它。
    foreach ($arch in @('x64', 'x86')) {
        $pdbCopies = @($global:ClientBuildCalls | Where-Object { $_.Name -eq 'cmake' -and $_.Values[-1] -eq (Join-Path $fixture "target/windows-full/$arch/bin/msime_host_api.pdb") })
        if ($pdbCopies.Count -ne 1) { throw "Host API PDB copy missing for $arch" }
    }
    if ($global:ClientBuildCalls[4].Values[-1] -ne (Join-Path $fixture 'target/windows-full/x64/bin/msime_host_api.pdb') -or
        $global:ClientBuildCalls[13].Values[-1] -ne (Join-Path $fixture 'target/windows-full/x86/bin/msime_host_api.pdb')) {
        throw 'Host API PDB copy is not right after its DLL'
    }
    if ($global:ClientBuildCalls[18].Values[-1] -ne (Join-Path $fixture 'target/windows-full/x64/bin/MSIME.pdb')) {
        throw 'Desktop PDB did not follow staged executable name'
    }
    # The on-device speech runtime is fetched for x64 only and staged beside the Server.
    $voiceRuntime = Join-Path $fixture 'target/voice-runtime/windows-x64'
    $fetch = $global:ClientBuildCalls[19]
    if ($fetch.Name -ne 'python' -or $fetch.Values[0] -ne (Join-Path $fixture 'scripts/fetch_voice_runtime.py') -or
        [Array]::IndexOf($fetch.Values, 'windows-x64') -ne ([Array]::IndexOf($fetch.Values, '--platform') + 1) -or
        [Array]::IndexOf($fetch.Values, $voiceRuntime) -ne ([Array]::IndexOf($fetch.Values, '--out') + 1)) {
        throw 'Voice runtime fetch mismatch'
    }
    $stage = $global:ClientBuildCalls[20].Values
    if ($global:ClientBuildCalls[20].Name -ne 'cmake' -or $stage[0] -ne '-E' -or $stage[1] -ne 'copy_if_different' -or
        $stage[-1] -ne (Join-Path $fixture 'target/windows-full/x64/bin') -or $stage.Count -ne 6) {
        throw 'Voice runtime staging mismatch'
    }
    foreach ($dll in $voiceRuntimeLibraries) {
        if ($stage -notcontains (Join-Path $voiceRuntime $dll)) { throw "Voice runtime library not staged: $dll" }
    }
    # 安装包不带手写模型和落定重排模型（设置应用按需下载），所以构建不再下载它们。
    if (@($global:ClientBuildCalls | Where-Object { $_.Name -eq 'python' -and ($_.Values -match 'fetch_(handwriting|settled)_model') }).Count -ne 0) {
        throw 'Build fetched a model the installer no longer carries'
    }
    # The Arm64X TIP (21-29): the ARM64 host, its import library under the ARM64 name, the ARM64 then the ARM64EC pass over the TIP sources, and the ARM64 host staged beside the Arm64X DLL.
    $arm64 = Join-Path $fixture 'target/windows-full/arm64'
    $arm64Library = Join-Path $arm64 'msime_host_api_arm64.dll.lib'
    $x64Library = Join-Path $fixture 'target/x86_64-pc-windows-msvc/release/msime_host_api.dll.lib'
    $response = "-DMSIME_TSF_ARM64X_RESPONSE=$(Join-Path $arm64 'msime-tsf-arm64.rsp')"
    $calls = $global:ClientBuildCalls
    if ($calls[21].Name -ne 'cargo' -or $calls[21].Values -notcontains 'aarch64-pc-windows-msvc' -or $calls[21].Values -notcontains 'msime-host-api' -or
        $calls[22].Name -ne 'python' -or $calls[22].Values -notcontains 'host-def' -or $calls[22].Values -notcontains '--arm64' -or
        $calls[23].Name -ne 'lib' -or $calls[23].Values -notcontains "/OUT:$arm64Library" -or $calls[23].Values -notcontains '/MACHINE:ARM64' -or
        $calls[24].Values -notcontains 'ARM64' -or $calls[24].Values -notcontains '-DMSIME_TSF_ARM64X=ARM64' -or
        $calls[24].Values -notcontains "-DMSIME_HOST_LIBRARY=$arm64Library" -or $calls[24].Values -notcontains $response -or
        $calls[25].Values -notcontains (Join-Path $arm64 'arm64') -or $calls[25].Values -notcontains 'msime-tsf' -or
        $calls[26].Values -notcontains 'ARM64EC' -or $calls[26].Values -notcontains '-DMSIME_TSF_ARM64X=ARM64EC' -or
        $calls[26].Values -notcontains "-DMSIME_HOST_LIBRARY=$x64Library" -or $calls[26].Values -notcontains $response -or
        $calls[26].Values -notcontains "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELWITHDEBINFO=$(Join-Path $arm64 'bin')" -or
        $calls[26].Values -notcontains '-DMSIME_WINDOWS_VERSION=2026.9.1' -or
        $calls[27].Values -notcontains (Join-Path $arm64 'arm64ec') -or
        $calls[28].Values[-1] -ne (Join-Path $arm64 'bin/msime_host_api_arm64.dll') -or
        $calls[28].Values -notcontains (Join-Path $fixture 'target/aarch64-pc-windows-msvc/release/msime_host_api.dll') -or
        $calls[29].Values[-1] -ne (Join-Path $arm64 'bin/msime_host_api.pdb')) {
        throw 'Arm64X TIP build mismatch'
    }
    foreach ($index in @(0, 1, 2, 3, 4, 5, 6, 7, 8, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29)) {
        if ($global:ClientBuildCalls[$index].Prefix -ne $x64) { throw 'Incorrect x64 dependency scope' }
    }
    foreach ($index in @(9, 10, 11, 12, 13)) {
        if ($global:ClientBuildCalls[$index].Prefix -ne $x86) { throw 'Incorrect x86 dependency scope' }
    }
    if ($global:ClientBuildCalls[1].Values -notcontains 'x64' -or
        $global:ClientBuildCalls[1].Values -notcontains '-DMSIME_SERVER_UIACCESS=ON' -or
        $global:ClientBuildCalls[10].Values -contains '-DMSIME_SERVER_UIACCESS=ON' -or
        $global:ClientBuildCalls[10].Values -notcontains 'Win32' -or
        $global:ClientBuildCalls[11].Values -notcontains 'msime-tsf' -or
        $global:ClientBuildCalls[5].Values -notcontains 'msime-mcp' -or
        $global:ClientBuildCalls[7].Values[-1] -ne (Join-Path $fixture 'target/windows-full/x64/bin/msime-mcp.pdb') -or
        $global:ClientBuildCalls[8].Name -ne 'msbuild' -or
        $global:ClientBuildCalls[8].Values -notcontains '-restore' -or
        $global:ClientBuildCalls[8].Values -notcontains '/p:TargetName=msime-client-settings' -or
        $global:ClientBuildCalls[16].Values -notcontains '--no-bundle' -or
        $global:ClientBuildCalls[2].Values -notcontains 'RelWithDebInfo') { throw 'Build target mismatch' }
    for ($failure = 1; $failure -le $count; $failure++) {
        $global:ClientBuildCalls.Clear()
        $global:ClientBuildFailAt = $failure
        $rejected = $false
        try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 }
        catch { $rejected = $_.Exception.Message -match 'Client build command failed: .* \(19\)' }
        if (-not $rejected -or $global:ClientBuildCalls.Count -ne $failure) { throw 'Build continued after failure' }
        if ((Get-Location).Path -ne $originalLocation -or $env:CMAKE_PREFIX_PATH -ne $originalPrefix -or
            $env:CARGO_TARGET_DIR -ne $originalTarget -or
            $env:CARGO_PROFILE_RELEASE_DEBUG -ne $originalDebug -or
            $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS -ne $originalArm64Flags) { throw 'Build leaked caller environment' }
    }
    $global:ClientBuildCalls.Clear()
    $global:ClientBuildFailAt = 0
    foreach ($invalid in @('1.2', '01.2.3', '65536.0.0', '1.2.3.4', '1.2.3-beta', 'not-a-version')) {
        $rejected = $false
        try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 -TargetVersion $invalid }
        catch { $rejected = $_.Exception.Message -like 'TargetVersion must*' }
        if (-not $rejected -or $global:ClientBuildCalls.Count -ne 0) { throw 'Invalid version reached build tools' }
    }
    & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86
    # 不传 TargetVersion 时，桌面构建（第 16 条，pnpm tauri build）不带版本覆盖。
    if ($global:ClientBuildCalls[16].Values -contains '--config') { throw 'Development version was overridden' }
    $global:ClientBuildCalls.Clear()
    Remove-Item -LiteralPath $desktopSymbols
    $rejected = $false
    try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 }
    catch { $rejected = $_.Exception.Message -eq 'Expected one Tauri desktop PDB output' }
    if (-not $rejected) { throw 'Missing desktop symbols accepted' }
    [IO.File]::WriteAllText($desktopSymbols, 'synthetic symbols')
    $alternateSymbols = Join-Path (Split-Path -Parent $desktopSymbols) 'msime-desktop.pdb'
    [IO.File]::WriteAllText($alternateSymbols, 'synthetic alternate symbols')
    $rejected = $false
    try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 }
    catch { $rejected = $_.Exception.Message -eq 'Expected one Tauri desktop PDB output' }
    if (-not $rejected) { throw 'Ambiguous desktop symbols accepted' }
    Remove-Item -LiteralPath $alternateSymbols
    # 不是 full 的版本：输出在 target/windows-<id>，host DLL 按版本表改名，并用 lib /DEF 生成同名导入库给原生目标链接；CMake 和设置窗口工程拿到同一个版本。
    foreach ($arch in @('x86', 'x64')) {
        foreach ($dll in @('MetasequoiaImeTsf.dll', 'msime_host_api_wubi.dll')) {
            Write-PEFixture (Join-Path $fixture "target/windows-wubi/$arch/bin/$dll") $arch dll
        }
    }
    Write-PEFixture (Join-Path $fixture 'target/windows-wubi/arm64/bin/MetasequoiaImeTsf.dll') arm64x dll
    Write-PEFixture (Join-Path $fixture 'target/windows-wubi/arm64/bin/msime_host_api_wubi_arm64.dll') arm64 dll
    foreach ($exe in @('MetasequoiaImeServer.exe', 'MetasequoiaImeWatchdog.exe', 'msime-client-prepare.exe',
        'msime-mcp.exe', 'msime-client-settings.exe', 'MSIME.exe')) {
        Write-PEFixture (Join-Path $fixture "target/windows-wubi/x64/bin/$exe") x64 exe
    }
    foreach ($dll in $voiceRuntimeLibraries) {
        Write-PEFixture (Join-Path $fixture "target/windows-wubi/x64/bin/$dll") x64 dll
    }
    [IO.File]::WriteAllText((Join-Path $fixture 'target/windows-wubi/x64/bin/msime-client-settings.pdb'), 'synthetic WinUI symbols')
    $global:ClientBuildCalls.Clear()
    & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 -Edition wubi
    $calls = @($global:ClientBuildCalls)
    $definitions = @($calls | Where-Object { $_.Name -eq 'python' -and $_.Values -contains 'host-def' })
    $libraries = @($calls | Where-Object { $_.Name -eq 'lib' })
    if ($definitions.Count -ne 3 -or $libraries.Count -ne 3) { throw 'Edition host DLL import libraries were not generated for every architecture' }
    # The Arm64X TIP's two halves link the edition's ARM64 and x64 hosts.
    $wubiArm64Library = Join-Path $fixture 'target/windows-wubi/arm64/msime_host_api_wubi_arm64.dll.lib'
    if (@($libraries | Where-Object { $_.Values -contains "/OUT:$wubiArm64Library" -and $_.Values -contains '/MACHINE:ARM64' }).Count -ne 1 -or
        @($calls | Where-Object { $_.Name -eq 'cmake' -and $_.Values -contains '-DMSIME_TSF_ARM64X=ARM64' -and $_.Values -contains "-DMSIME_HOST_LIBRARY=$wubiArm64Library" -and $_.Values -contains '-DMSIME_EDITION=wubi' }).Count -ne 1 -or
        @($calls | Where-Object { $_.Name -eq 'cmake' -and $_.Values -contains '-DMSIME_TSF_ARM64X=ARM64EC' -and $_.Values -contains ('-DMSIME_HOST_LIBRARY=' + (Join-Path $fixture 'target/windows-wubi/x64/msime_host_api_wubi.dll.lib')) }).Count -ne 1 -or
        @($calls | Where-Object { $_.Name -eq 'cmake' -and $_.Values[-1] -eq (Join-Path $fixture 'target/windows-wubi/arm64/bin/msime_host_api_wubi_arm64.dll') }).Count -ne 1) {
        throw 'Edition Arm64X TIP build mismatch'
    }
    foreach ($arch in @('x64', 'x86')) {
        $library = Join-Path $fixture "target/windows-wubi/$arch/msime_host_api_wubi.dll.lib"
        if (@($libraries | Where-Object { $_.Values -contains "/OUT:$library" }).Count -ne 1) { throw "Edition import library missing for $arch" }
        $configure = @($calls | Where-Object { $_.Name -eq 'cmake' -and $_.Values -contains (Join-Path $fixture "target/windows-wubi/$arch") -and $_.Values -contains '-B' })
        if ($configure.Count -ne 1 -or $configure[0].Values -notcontains '-DMSIME_EDITION=wubi' -or
            $configure[0].Values -notcontains "-DMSIME_HOST_LIBRARY=$library") { throw "Edition CMake configuration mismatch for $arch" }
        $copy = @($calls | Where-Object { $_.Name -eq 'cmake' -and $_.Values[-1] -eq (Join-Path $fixture "target/windows-wubi/$arch/bin/msime_host_api_wubi.dll") })
        if ($copy.Count -ne 1) { throw "Edition host DLL was not staged under its own name for $arch" }
    }
    $settings = @($calls | Where-Object { $_.Name -eq 'msbuild' })
    if ($settings.Count -ne 1 -or $settings[0].Values -notcontains '/p:MsimeEdition=wubi' -or
        $settings[0].Values -notcontains ('/p:HostApiLibrary=' + (Join-Path $fixture 'target/windows-wubi/x64/msime_host_api_wubi.dll.lib'))) {
        throw 'Edition settings build mismatch'
    }
    if (@($calls | Where-Object { $_.Name -eq 'cmake' -and $_.Values[-1] -eq (Join-Path $fixture 'target/windows-wubi/x64/bin/MSIME.exe') }).Count -ne 1) {
        throw 'Edition desktop shell was not staged in the edition output'
    }
    $global:ClientBuildCalls.Clear()
    $rejected = $false
    try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 -Edition klingon }
    catch { $rejected = $_.Exception.Message -like 'Edition klingon has no Windows identifiers*' }
    if (-not $rejected -or $global:ClientBuildCalls.Count -ne 0) { throw 'Unknown edition reached build tools' }
    # A plain ARM64 TIP would load in native processes only; emulated x64 ones need the ARM64EC half.
    Write-PEFixture (Join-Path $fixture 'target/windows-full/arm64/bin/MetasequoiaImeTsf.dll') arm64 dll
    $rejected = $false
    try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 }
    catch { $rejected = $_.Exception.Message -eq 'PE Arm64X hybrid metadata mismatch' }
    if (-not $rejected) { throw 'Build accepted an ARM64 TIP without its ARM64EC half' }
    Write-PEFixture (Join-Path $fixture 'target/windows-full/arm64/bin/MetasequoiaImeTsf.dll') arm64x dll
    Write-PEFixture (Join-Path $fixture 'target/windows-full/x64/bin/onnxruntime.dll') x86 dll
    $rejected = $false
    try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 }
    catch { $rejected = $_.Exception.Message -eq 'PE architecture mismatch' }
    if (-not $rejected) { throw 'Build accepted a 32-bit voice runtime beside the 64-bit Server' }
    Write-PEFixture (Join-Path $fixture 'target/windows-full/x64/bin/onnxruntime.dll') x64 dll
    Write-PEFixture (Join-Path $fixture 'target/windows-full/x86/bin/msime_host_api.dll') x64 dll
    $rejected = $false
    try { & $entry -RepoRoot $fixture -X64Dependencies $x64 -X86Dependencies $x86 }
    catch { $rejected = $_.Exception.Message -eq 'PE architecture mismatch' }
    if (-not $rejected) { throw 'Build accepted mixed-architecture output' }
    if ((Get-Location).Path -ne $originalLocation -or $env:CMAKE_PREFIX_PATH -ne $originalPrefix -or
        $env:CARGO_TARGET_DIR -ne $originalTarget -or $env:CARGO_PROFILE_RELEASE_DEBUG -ne $originalDebug -or
        $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS -ne $originalArm64Flags) {
        throw 'PE verification failure leaked caller environment'
    }
    Write-Output 'Client build orchestration: targets, dependency scopes, failure stages and PE gate passed'
} finally {
    Remove-Item Function:/cargo, Function:/cmake, Function:/pnpm, Function:/msbuild, Function:/python, Function:/lib, Function:/Invoke-ClientCommandProbe -ErrorAction SilentlyContinue
    Remove-Variable ClientBuildCalls, ClientBuildFailAt -Scope Global -ErrorAction SilentlyContinue
    if (Test-Path $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
