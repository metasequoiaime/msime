# Template: platforms/windows/packaging/render.py fills the @...@ fields from a windows-v release.
# Runs the released Inno Setup installer (platforms/windows/installer/msime_setup.iss) silently. It is x64 only (ArchitecturesAllowed=x64compatible), so only the 64-bit URL is given and Chocolatey refuses 32-bit Windows.
$ErrorActionPreference = 'Stop'

$silentArgs = "/SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG=`"$env:TEMP\$env:ChocolateyPackageName.$env:ChocolateyPackageVersion.Install.log`""

# /DataDir: is passed to the installer's /DATADIR= switch, which takes the same directory its wizard page offers. The installer rejects a directory it may not use (not empty and not an earlier MSIME data directory, or inside Windows or Program Files); in a silent install it only logs the reason and exits with a non-zero code, which Chocolatey reports as a failed install. Run as SYSTEM, the installer's default directory is under the system profile inside C:\Windows and is always rejected, so SYSTEM installs need /DataDir:.
$pp = Get-PackageParameters
if ($pp['DataDir']) {
    $silentArgs += " /DATADIR=`"$($pp['DataDir'].TrimEnd('\'))`""
}

$packageArgs = @{
    packageName    = $env:ChocolateyPackageName
    fileType       = 'exe'
    url64bit       = '@INSTALLER_URL@'
    checksum64     = '@SHA256@'
    checksumType64 = 'sha256'
    silentArgs     = $silentArgs
    validExitCodes = @(0)
}

Install-ChocolateyPackage @packageArgs
