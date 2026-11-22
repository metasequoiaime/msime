# Exercise real entry scripts with synthetic stages: never build/sign/install.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-local-entry-' + [Guid]::NewGuid())
$original = (Get-Location).Path
try {
    $installer = Join-Path $root 'platforms/windows/installer'
    # 桌面资源目录存在时入口不去下载它；这里只给一个空目录，暂存阶段是合成的。
    New-Item -ItemType Directory -Force $installer, (Join-Path $root 'notices'), (Join-Path $root 'target/desktop-resources') | Out-Null
    [IO.File]::WriteAllText((Join-Path $root 'notices/THIRD_PARTY_NOTICES.txt'), 'synthetic notices')
    foreach ($entry in @('test.ps1', 'test-light.ps1', 'Invoke-LocalInstall.ps1')) {
        Copy-Item (Join-Path $PSScriptRoot "../$entry") $installer
    }
    $stages = @('Build-Client.ps1', 'Prepare-PackageFiles.ps1', 'Sign-PackageBinaries-Local.ps1',
        'Compile-Installer.ps1', 'Sign-Installer-Local.ps1', 'Install.ps1')
    foreach ($stage in $stages) {
        $path = if ($stage -eq 'Build-Client.ps1') { Join-Path $root 'platforms/windows/Build-Client.ps1' } else { Join-Path $installer $stage }
        $body = @'
param($RepoRoot,$X64Dependencies,$X86Dependencies,$Generator,$TargetVersion,$NoticesDirectory,
      $DesktopResourcesDirectory,$ServerReleaseDirectory,$Tsf32ReleaseDirectory,$Tsf64ReleaseDirectory,$DesktopExecutable,$RustOutputs,[switch]$Light,[switch]$ServerUiAccess)
$global:InstallerProbeCalls.Add(@{ Stage = '@STAGE@'; Parameters = $PSBoundParameters })
if ($global:InstallerProbeCalls.Count -eq $global:InstallerProbeFailAt) {
    if ($global:InstallerProbeExit) { exit 23 }
    throw 'synthetic stage failure'
}
'@
        [IO.File]::WriteAllText($path, $body.Replace('@STAGE@', $stage))
    }
    $global:InstallerProbeCalls = [Collections.Generic.List[object]]::new()
    foreach ($entry in @('test.ps1', 'test-light.ps1')) {
        $global:InstallerProbeFailAt = 0
        $global:InstallerProbeExit = $false
        $global:InstallerProbeCalls.Clear()
        $parameters = @{ X64Dependencies = 'synthetic x64'; X86Dependencies = 'synthetic x86'; NoticesDirectory = 'notices'; TargetVersion = '2026.9.1' }
        & (Join-Path $installer $entry) @parameters
        if (($global:InstallerProbeCalls.Stage -join ',') -ne ($stages -join ',')) { throw 'Installer stage ordering mismatch' }
        $prepare = $global:InstallerProbeCalls[1].Parameters
        if ($global:InstallerProbeCalls[0].Parameters.TargetVersion -ne '2026.9.1') {
            throw 'Release version did not reach Client build'
        }
        if (-not $global:InstallerProbeCalls[0].Parameters.ServerUiAccess) { throw 'Trusted local signing build lost UIAccess' }
        if ($prepare.RepoRoot -ne $root -or $prepare.ServerReleaseDirectory -ne 'target/windows-full/x64/bin' -or
            $prepare.Tsf32ReleaseDirectory -ne 'target/windows-full/x86/bin' -or $prepare.TargetVersion -ne '2026.9.1') {
            throw 'Client build/staging arguments mismatch'
        }
        foreach ($index in @(1, 3, 4, 5)) {
            if ([bool]$global:InstallerProbeCalls[$index].Parameters.Light -ne ($entry -eq 'test-light.ps1')) { throw 'Light mode not propagated' }
        }
        foreach ($exitFailure in @($false, $true)) {
            for ($failure = 1; $failure -le 6; $failure++) {
                $global:InstallerProbeCalls.Clear()
                $global:InstallerProbeFailAt = $failure
                $global:InstallerProbeExit = $exitFailure
                $rejected = $false
                try { & (Join-Path $installer $entry) @parameters } catch { $rejected = $true }
                if (-not $rejected -or $global:InstallerProbeCalls.Count -ne $failure) { throw 'Installer continued after failure' }
                if ((Get-Location).Path -ne $original) { throw 'Installer leaked working directory' }
            }
        }
    }
    $global:InstallerProbeFailAt = 0
    $global:InstallerProbeExit = $false

    # 不传依赖前缀时取环境变量；-NoInstall 停在签安装包；-RustOutputs 以绝对路径交给 Build-Client；生成器总有值。
    $savedX64 = $env:MSIME_WINDOWS_X64_DEPS
    $savedX86 = $env:MSIME_WINDOWS_X86_DEPS
    try {
        $env:MSIME_WINDOWS_X64_DEPS = 'env x64'
        $env:MSIME_WINDOWS_X86_DEPS = 'env x86'
        foreach ($entry in @('test.ps1', 'test-light.ps1')) {
            $global:InstallerProbeCalls.Clear()
            & (Join-Path $installer $entry) -NoticesDirectory notices -TargetVersion 2026.9.1 -NoInstall -RustOutputs 'target/windows-rust'
            if (($global:InstallerProbeCalls.Stage -join ',') -ne (($stages | Select-Object -First 5) -join ',')) { throw 'NoInstall still launched the installer' }
            $build = $global:InstallerProbeCalls[0].Parameters
            if ($build.X64Dependencies -ne 'env x64' -or $build.X86Dependencies -ne 'env x86') { throw 'Dependency prefixes did not default from the environment' }
            if ($build.RustOutputs -ne (Join-Path $root 'target/windows-rust')) { throw 'RustOutputs did not reach Client build' }
            if (-not $build.Generator) { throw 'No CMake generator was resolved' }
        }
        # 入口在进程环境为空时还会读用户和系统环境变量；开发机上设过这两个变量时，缺依赖这一条测不了，只在干净的机器（CI）上核对。
        $env:MSIME_WINDOWS_X86_DEPS = $null
        $persisted = [Environment]::GetEnvironmentVariable('MSIME_WINDOWS_X86_DEPS', 'User') -or
            [Environment]::GetEnvironmentVariable('MSIME_WINDOWS_X86_DEPS', 'Machine')
        if ($persisted) {
            Write-Output 'Skipped the missing-prefix check: MSIME_WINDOWS_X86_DEPS is persisted on this machine'
        } else {
            $global:InstallerProbeCalls.Clear()
            $rejected = $false
            try { & (Join-Path $installer 'test.ps1') -NoticesDirectory notices -TargetVersion 2026.9.1 } catch { $rejected = $true }
            if (-not $rejected -or $global:InstallerProbeCalls.Count -ne 0) { throw 'Missing dependency prefix reached build/sign/install' }
        }
        # 进程环境为空、只有持久化的值时也能取到（setx 之后没重开的终端就是这种情况）。
        if ($persisted) {
            $global:InstallerProbeCalls.Clear()
            & (Join-Path $installer 'test-light.ps1') -NoticesDirectory notices -TargetVersion 2026.9.1 -NoInstall
            if ($global:InstallerProbeCalls[0].Parameters.X86Dependencies -ne [Environment]::GetEnvironmentVariable('MSIME_WINDOWS_X86_DEPS', 'User')) {
                throw 'Persisted dependency prefix was not read'
            }
        }
    } finally {
        $env:MSIME_WINDOWS_X64_DEPS = $savedX64
        $env:MSIME_WINDOWS_X86_DEPS = $savedX86
    }

    Remove-Item -LiteralPath (Join-Path $root 'notices/THIRD_PARTY_NOTICES.txt')
    $global:InstallerProbeCalls.Clear()
    $rejected = $false
    try { & (Join-Path $installer 'test.ps1') @parameters } catch { $rejected = $true }
    if (-not $rejected -or $global:InstallerProbeCalls.Count -ne 0) { throw 'Missing notices reached build/sign/install' }
    Write-Output 'Full/light installer entry ordering, arguments and all exception/exit failures passed'
} finally {
    Remove-Variable InstallerProbeCalls, InstallerProbeFailAt, InstallerProbeExit -Scope Global -ErrorAction SilentlyContinue
    if (Test-Path $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
