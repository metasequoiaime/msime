# SignPath-PackageBinaries.ps1 认定本项目二进制的规则：用合成的暂存包核对 Stage 交给 SignPath 的正好是本项目的 EXE 和 DLL，上游的二进制一个不带；PDB 规则失效、SignPath 少送回文件或送回未签名的文件时 Stage 和 Restore 都失败。不签名、不构建。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Join-Path $PSScriptRoot '../SignPath-PackageBinaries.ps1'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../../..')).Path
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-signpath-payload-' + [Guid]::NewGuid())

function New-File {
    param([string]$Path)
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    [IO.File]::WriteAllText($Path, 'synthetic')
}

function Assert-Throws {
    param([string]$Description, [scriptblock]$Action, [string]$Expected)
    try { & $Action } catch {
        if ($_.Exception.Message -notlike "*$Expected*") { throw "$Description 失败的原因不对：$($_.Exception.Message)" }
        return
    }
    throw "$Description 没有失败"
}

try {
    $package = Join-Path $root 'installer'
    # 本项目的 EXE 都带 PDB；RestartAgent 是 Windows App SDK 自带的，没有 PDB。
    foreach ($exe in @('MetasequoiaImeServer', 'MetasequoiaImeWatchdog', 'msime-mcp', 'msime-client-settings', 'msime-client-prepare', 'MSIME')) {
        New-File (Join-Path $package "server_exe/$exe.exe")
        New-File (Join-Path $package "server_exe/$exe.pdb")
    }
    foreach ($upstream in @('server_exe/RestartAgent.exe', 'server_exe/onnxruntime.dll', 'server_exe/Microsoft.UI.Xaml.dll', 'server_exe/NpuDetect/NPUDetect.dll')) {
        New-File (Join-Path $package $upstream)
    }
    foreach ($arch in @('32', '64')) {
        foreach ($name in @('MetasequoiaImeTsf.dll', 'MetasequoiaImeTsf.pdb', 'msime_host_api_wubi.dll', 'zlib1.dll', 'libcurl.dll', 'fmt.dll')) {
            New-File (Join-Path $package "tsf_dll/$arch/$name")
        }
    }
    foreach ($name in @('MetasequoiaImeTsf.dll', 'msime_host_api_wubi_arm64.dll')) { New-File (Join-Path $package "tsf_dll/arm64/$name") }

    $staged = Join-Path $root 'payload'
    & $script -Mode Stage -Edition wubi -Directory $staged -PackageRoot $package -RepoRoot $repoRoot
    $actual = @(Get-ChildItem -LiteralPath (Join-Path $staged 'wubi') -Recurse -File | ForEach-Object { $_.FullName.Substring((Join-Path $staged 'wubi').Length + 1) -replace '\\', '/' } | Sort-Object)
    $expected = @(
        'server_exe/MetasequoiaImeServer.exe', 'server_exe/MetasequoiaImeWatchdog.exe', 'server_exe/msime-mcp.exe',
        'server_exe/msime-client-settings.exe', 'server_exe/msime-client-prepare.exe', 'server_exe/MSIME.exe',
        'tsf_dll/32/MetasequoiaImeTsf.dll', 'tsf_dll/32/msime_host_api_wubi.dll',
        'tsf_dll/64/MetasequoiaImeTsf.dll', 'tsf_dll/64/msime_host_api_wubi.dll',
        'tsf_dll/arm64/MetasequoiaImeTsf.dll', 'tsf_dll/arm64/msime_host_api_wubi_arm64.dll'
    ) | Sort-Object
    if (($actual -join ',') -cne ($expected -join ',')) {
        throw "Stage 提交的文件不对：`n实际 $($actual -join ', ')`n预期 $($expected -join ', ')"
    }

    # 别的版本的宿主 DLL 不是本版本的二进制，按版本表取名。
    Assert-Throws 'full 版本在五笔的暂存包上 Stage' { & $script -Mode Stage -Edition full -Directory $staged -PackageRoot $package -RepoRoot $repoRoot } 'msime_host_api.dll'

    # SignPath 送回的文件必须带有效签名：合成文件没有签名，Restore 拒绝覆盖。
    Assert-Throws '未签名的文件 Restore' { & $script -Mode Restore -Edition wubi -Directory $staged -PackageRoot $package -RepoRoot $repoRoot } '签名状态'
    # test-signing 只放宽信任链，不放宽「必须带签名」：没有签名证书的文件照样拒绝。
    Assert-Throws '未签名的文件以 -TestCertificate Restore' { & $script -Mode Restore -Edition wubi -Directory $staged -PackageRoot $package -RepoRoot $repoRoot -TestCertificate } '签名状态'
    Remove-Item -LiteralPath (Join-Path $staged 'wubi/tsf_dll/64/msime_host_api_wubi.dll')
    Assert-Throws '少送回一个文件 Restore' { & $script -Mode Restore -Edition wubi -Directory $staged -PackageRoot $package -RepoRoot $repoRoot } 'tsf_dll/64/msime_host_api_wubi.dll'
    Assert-Throws '没有本版本的 Restore' { & $script -Mode Restore -Edition pinyin -Directory $staged -PackageRoot $package -RepoRoot $repoRoot } '没有 pinyin'

    # PDB 不再暂存时 Server 入口会被当成上游二进制，Stage 必须失败而不是只签 TIP。
    Remove-Item -LiteralPath (Join-Path $package 'server_exe/MetasequoiaImeWatchdog.pdb')
    Assert-Throws '缺 PDB 时 Stage' { & $script -Mode Stage -Edition wubi -Directory $staged -PackageRoot $package -RepoRoot $repoRoot } 'MetasequoiaImeWatchdog.exe'
    Write-Host 'SignPath 二进制认定规则通过'
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
