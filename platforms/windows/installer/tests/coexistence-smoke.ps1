param(
    # 同一个版本号的几个安装包，键是版本 id（shared/contracts/editions.json），值是安装包路径；必须含 full。
    [Parameter(Mandatory)][hashtable]$Installers
)
# 在一次性的 Windows 机器上把几个版本先后静默装上，检查它们各自的程序目录、HKLM 键、TIP、COM 注册、看门狗任务和数据目录同时存在、互不覆盖；再卸载不是 full 的版本，检查 full 的这些东西都还在、可以用；最后卸载 full。整个过程中机器上还有一份伪造的 msime-windows（只有它的 HKLM 键和带所有权标记的数据目录），检查没有哪个版本接管、嵌套或删除它。需要管理员会话，发布 runner 就是。
# Server 带 uiAccess，未签名的包在 runner 上拉不起来，所以这里只看安装结果，不看两个 Server 同时运行；那一层要在签名后的包上实机验证。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $Installers.ContainsKey('full') -or $Installers.Count -lt 2) { throw 'Provide the full installer and at least one other edition' }
$table = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../../../shared/contracts/editions.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$logs = Join-Path $env:RUNNER_TEMP 'msime-coexistence-smoke'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$failures = [Collections.Generic.List[string]]::new()
function Check([bool]$Condition, [string]$What) {
    if ($Condition) { Write-Output "ok: $What" } else { Write-Output "FAIL: $What"; $failures.Add($What) }
}
function Identity([string]$Edition) {
    $entry = @($table.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
    if ($entry.Count -ne 1) { throw "Edition $Edition has no Windows identifiers" }
    $entry[0].platforms.windows
}
function InprocServer([string]$ClassesRoot, [string]$Clsid) {
    $key = "$ClassesRoot\CLSID\$Clsid\InprocServer32"
    if (Test-Path -LiteralPath $key) { (Get-Item -LiteralPath $key).GetValue('') } else { $null }
}
# 一个版本装好之后应有的样子；每装一个或卸一个版本，都把还装着的每个版本重查一遍。
function Check-Installed([string]$Edition, [string]$When) {
    $identity = Identity $Edition
    $appKey = "HKLM:\$($identity.registry_key)"
    $pf64 = Join-Path $env:ProgramFiles $identity.install_dir
    $pf32 = Join-Path ${env:ProgramFiles(x86)} $identity.install_dir
    # 64 位 TIP 在 System32\IME 下各版本自己的目录里，见 msime_setup.iss 的 MySystemTipDir。
    $sys64 = Join-Path $env:windir "System32\IME\$($identity.install_dir)"
    $app = if (Test-Path -LiteralPath $appKey) { Get-ItemProperty -LiteralPath $appKey } else { $null }
    Check ($null -ne $app) "${When}: $Edition keeps HKLM\$($identity.registry_key)"
    if ($null -eq $app) { return }
    Check ($app.ServerPath -eq (Join-Path $pf64 'server\MetasequoiaImeServer.exe') -and (Test-Path -LiteralPath $app.ServerPath -PathType Leaf)) "${When}: $Edition ServerPath is its own Server"
    Check ((InprocServer 'HKLM:\SOFTWARE\Classes' $identity.clsid) -eq (Join-Path $sys64 "$($app.VersionDir)\MetasequoiaImeTsf.dll")) "${When}: $Edition 64-bit COM server is its own TSF DLL"
    Check ((InprocServer 'HKLM:\SOFTWARE\WOW6432Node\Classes' $identity.clsid) -eq (Join-Path $pf32 "$($app.VersionDir)\MetasequoiaImeTsf.dll")) "${When}: $Edition 32-bit COM server is its own TSF DLL"
    Check (Test-Path -LiteralPath "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$($identity.clsid)") "${When}: $Edition TIP registered"
    Check (Test-Path -LiteralPath (Join-Path $sys64 "$($app.VersionDir)\$($identity.host_dll)") -PathType Leaf) "${When}: $Edition $($identity.host_dll) beside its TSF DLL"
    $task = Get-ScheduledTask -TaskName $identity.watchdog_task -ErrorAction SilentlyContinue
    $action = if ($task) { @($task.Actions)[0].Execute.Trim('"') } else { $null }
    Check ($action -eq (Join-Path $pf64 'server\MetasequoiaImeWatchdog.exe')) "${When}: $Edition watchdog task runs its own Watchdog"
    # 所有权标记的文件名接版本的名字后缀（edition_windows.py 的 data_dir_marker，full 是 .metasequoiaime-data.full）：一个版本的安装器认不出别的版本的标记，不会接管它们的数据目录。
    Check (Test-Path -LiteralPath (Join-Path $app.DataDir ('.metasequoiaime-data' + $identity.name_suffix)) -PathType Leaf) "${When}: $Edition DataDir still owned ($($app.DataDir))"
    $declaration = Join-Path $pf64 'server\edition.json'
    Check ((Test-Path -LiteralPath $declaration) -and (Get-Content -LiteralPath $declaration -Raw -Encoding UTF8 | ConvertFrom-Json).edition -eq $Edition) "${When}: $Edition declares itself"
}
function Check-Removed([string]$Edition, [string]$When) {
    $identity = Identity $Edition
    Check (-not (Test-Path -LiteralPath (Join-Path $env:ProgramFiles $identity.install_dir))) "${When}: $Edition program directory removed"
    Check (-not (Test-Path -LiteralPath (Join-Path $env:windir "System32\IME\$($identity.install_dir)"))) "${When}: $Edition System32 TSF directory removed"
    Check (-not (Test-Path -LiteralPath "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$($identity.clsid)")) "${When}: $Edition TIP removed"
    Check ($null -eq (InprocServer 'HKLM:\SOFTWARE\Classes' $identity.clsid)) "${When}: $Edition COM registration removed"
    Check ($null -eq (Get-ScheduledTask -TaskName $identity.watchdog_task -ErrorAction SilentlyContinue)) "${When}: $Edition watchdog task removed"
}
function Uninstall([string]$Edition) {
    $uninstaller = Join-Path (Join-Path $env:ProgramFiles (Identity $Edition).install_dir) 'unins000.exe'
    # 卸载默认保留数据目录；这里带 /REMOVEDATA，核对删掉自己的数据目录时不会碰到别的版本和 msime-windows 的。
    Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/REMOVEDATA', "/LOG=`"$logs\uninstall-$Edition.log`"" -Wait
    # 卸载程序会把自己复制到临时目录再运行并立即返回，所以等程序目录消失，而不是等进程。
    $deadline = (Get-Date).AddMinutes(3)
    while ((Test-Path -LiteralPath $uninstaller) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 2 }
    Start-Sleep -Seconds 5
}

# msime-windows 是另一个产品，身份记在 platforms/windows/scripts/edition_windows.py 的 MSIME_WINDOWS：HKLM 键 Software\Metasequoia\MetasequoiaIME 下的 DataDir，目录里是不带后缀的 .metasequoiaime-data 标记。这里只伪造这两样，不装它本身。
$foreignKey = 'HKLM:\Software\Metasequoia\MetasequoiaIME'
$foreignData = Join-Path $env:RUNNER_TEMP 'msime-coexistence-data-msime-windows'
if (Test-Path -LiteralPath $foreignData) { Remove-Item -LiteralPath $foreignData -Recurse -Force }
New-Item -ItemType Directory -Force -Path $foreignData | Out-Null
Set-Content -LiteralPath (Join-Path $foreignData '.metasequoiaime-data') -Value 'This directory is managed by Metasequoia IME.'
Set-Content -LiteralPath (Join-Path $foreignData 'config.toml') -Value '# synthetic msime-windows configuration'
New-Item -Force -Path $foreignKey | Out-Null
New-ItemProperty -LiteralPath $foreignKey -Name DataDir -Value $foreignData -PropertyType String -Force | Out-Null
function Check-Foreign([string]$When) {
    $recorded = if (Test-Path -LiteralPath $foreignKey) { (Get-ItemProperty -LiteralPath $foreignKey).DataDir } else { $null }
    Check ($recorded -eq $foreignData) "${When}: msime-windows' HKLM key and DataDir untouched"
    Check ((Test-Path -LiteralPath (Join-Path $foreignData '.metasequoiaime-data') -PathType Leaf) -and (Test-Path -LiteralPath (Join-Path $foreignData 'config.toml') -PathType Leaf)) "${When}: msime-windows' data directory untouched"
}

# full 最先装，其他版本装在它之后：这是已经装着 full 的用户再装一个版本的顺序。每个版本用自己的数据目录。
$order = @('full') + @($Installers.Keys | Where-Object { $_ -ne 'full' } | Sort-Object)
foreach ($edition in $order) {
    $installer = (Resolve-Path -LiteralPath $Installers[$edition]).Path
    $dataDir = Join-Path $env:RUNNER_TEMP "msime-coexistence-data-$edition"
    if (Test-Path -LiteralPath $dataDir) { Remove-Item -LiteralPath $dataDir -Recurse -Force }
    $run = Start-Process -FilePath $installer -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DATADIR=`"$dataDir`"", "/LOG=`"$logs\install-$edition.log`"" -Wait -PassThru
    if ($run.ExitCode -ne 0) { Get-Content -LiteralPath "$logs\install-$edition.log" -Tail 60; throw "$edition installer exited with $($run.ExitCode)" }
    foreach ($installed in $order[0..([Array]::IndexOf($order, $edition))]) { Check-Installed $installed "after installing $edition" }
}
# 一个版本拒绝接管另一个版本的数据目录：把五笔版重新装到 full 的数据目录上必须失败，而且 full 的数据目录原样留着。
$other = $order[1]
$fullData = (Get-ItemProperty -LiteralPath "HKLM:\$((Identity 'full').registry_key)").DataDir
$run = Start-Process -FilePath (Resolve-Path -LiteralPath $Installers[$other]).Path -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DATADIR=`"$fullData`"", "/LOG=`"$logs\takeover-$other.log`"" -Wait -PassThru
Check ($run.ExitCode -ne 0) "$other refuses full's data directory"
Check-Installed 'full' "after $other tried to take over full's data directory"
# 反过来也一样：full 的安装器只认 .metasequoiaime-data.full 这个标记，目录里有别的版本的标记就不认，所以把 full 重新装到它们的数据目录上同样失败，它们的数据目录原样留着。
$otherData = (Get-ItemProperty -LiteralPath "HKLM:\$((Identity $other).registry_key)").DataDir
$run = Start-Process -FilePath (Resolve-Path -LiteralPath $Installers['full']).Path -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DATADIR=`"$otherData`"", "/LOG=`"$logs\takeover-full.log`"" -Wait -PassThru
Check ($run.ExitCode -ne 0) "full refuses $other's data directory"
Check-Installed $other "after full tried to take over $other's data directory"
# msime-windows 的数据目录同样不归任何版本管：装到它上面、或者嵌进它里面都要失败。full 以前就是 msime-windows 的身份，它最容易出事，所以每个版本都试一遍。
Check-Foreign 'after installing every edition'
foreach ($edition in $order) {
    foreach ($target in @($foreignData, (Join-Path $foreignData 'nested'))) {
        $run = Start-Process -FilePath (Resolve-Path -LiteralPath $Installers[$edition]).Path -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DATADIR=`"$target`"", "/LOG=`"$logs\takeover-msime-windows-$edition-$([IO.Path]::GetFileName($target)).log`"" -Wait -PassThru
        Check ($run.ExitCode -ne 0) "$edition refuses msime-windows' data directory ($target)"
    }
    Check-Installed $edition "after $edition tried to take over msime-windows' data directory"
}
Check-Foreign 'after every edition tried to take over its data directory'

foreach ($edition in @($order | Where-Object { $_ -ne 'full' })) {
    Uninstall $edition
    Check-Removed $edition "after uninstalling $edition"
    Check-Installed 'full' "after uninstalling $edition"
}
Uninstall 'full'
Check-Removed 'full' 'after uninstalling full'
Check-Foreign 'after uninstalling every edition'
Remove-Item -LiteralPath $foreignKey -Recurse -Force
Remove-Item -LiteralPath $foreignData -Recurse -Force

if ($failures.Count -gt 0) {
    Get-ChildItem -LiteralPath $logs -Filter '*.log' | ForEach-Object { Write-Output "---- $($_.Name) (tail) ----"; Get-Content -LiteralPath $_.FullName -Tail 30 }
    throw "$($failures.Count) coexistence check(s) failed"
}
Write-Output "Editions $($order -join ', ') install side by side, and uninstalling one leaves the others and msime-windows intact"
