# Runs the Inno Setup uninstaller the installer registered. Its uninstall key is the installer's AppId plus Inno's "_is1" suffix (platforms/windows/installer/msime_setup.iss), so the key is matched by name rather than by display name.
$ErrorActionPreference = 'Stop'

$productCode = '{A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}_is1'
[array]$keys = Get-UninstallRegistryKey -SoftwareName 'Metasequoia IME*' | Where-Object { $_.PSChildName -eq $productCode }

if ($keys.Count -eq 0) {
    Write-Warning "$env:ChocolateyPackageName has already been uninstalled by other means."
    return
}
if ($keys.Count -gt 1) {
    $keys | ForEach-Object { Write-Warning "- $($_.DisplayName) $($_.DisplayVersion)" }
    throw "$($keys.Count) matching uninstall entries found; refusing to guess which one to remove."
}

$uninstaller = $keys[0].UninstallString.Trim('"')
Uninstall-ChocolateyPackage -PackageName $env:ChocolateyPackageName -FileType 'exe' -File $uninstaller `
    -SilentArgs "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG=`"$env:TEMP\$env:ChocolateyPackageName.Uninstall.log`"" `
    -ValidExitCodes @(0)

# The Inno uninstaller relaunches itself from a temporary copy and returns at once (platforms/windows/installer/tests/install-smoke.ps1 waits the same way), so wait for it to remove its own file before Chocolatey reports the package as gone.
$deadline = (Get-Date).AddMinutes(3)
while ((Test-Path -LiteralPath $uninstaller) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
}
if (Test-Path -LiteralPath $uninstaller) {
    Write-Warning "The uninstaller is still running after 3 minutes; see $env:TEMP\$env:ChocolateyPackageName.Uninstall.log."
}
