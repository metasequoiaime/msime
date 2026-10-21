# 直接检查产品使用的清单链接规则，并在同一个构建目录覆盖旧缓存。
param([string]$BuildDirectory = 'target/windows-server-manifest')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ServerManifestProbe {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr LoadLibraryEx(string path, IntPtr file, uint flags);
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr FindResource(IntPtr module, IntPtr name, IntPtr type);
    [DllImport("kernel32.dll")] static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll")] static extern IntPtr LockResource(IntPtr resource);
    [DllImport("kernel32.dll")] static extern uint SizeofResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll")] static extern bool FreeLibrary(IntPtr module);
    public static string Read(string path) {
        IntPtr module = LoadLibraryEx(path, IntPtr.Zero, 0x22);
        if (module == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
        try {
            IntPtr resource = FindResource(module, new IntPtr(1), new IntPtr(24));
            if (resource == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
            var size = SizeofResource(module, resource);
            var bytes = new byte[size];
            Marshal.Copy(LockResource(LoadResource(module, resource)), bytes, 0, (int)size);
            return System.Text.Encoding.UTF8.GetString(bytes).TrimEnd('\0');
        } finally { FreeLibrary(module); }
    }
}
'@
foreach ($mode in @('OFF', 'ON', 'OFF')) {
    & cmake -S $PSScriptRoot -B $BuildDirectory -A x64 "-DMSIME_SERVER_UIACCESS=$mode"
    if ($LASTEXITCODE -ne 0) { throw "Server manifest configure failed: $mode" }
    & cmake --build $BuildDirectory --config RelWithDebInfo
    if ($LASTEXITCODE -ne 0) { throw "Server manifest link failed: $mode" }
    $program = (Resolve-Path (Join-Path $BuildDirectory 'RelWithDebInfo/msime-client-server.exe')).Path
    [xml]$manifest = [ServerManifestProbe]::Read($program)
    $execution = $manifest.SelectSingleNode("//*[local-name()='requestedExecutionLevel']")
    $expected = if ($mode -eq 'ON') { 'true' } else { 'false' }
    if (-not $execution -or $execution.level -ne 'asInvoker' -or $execution.uiAccess -ne $expected) {
        throw "Embedded Server manifest does not match $mode"
    }
    if ($mode -eq 'OFF') {
        $process = Start-Process -FilePath $program -PassThru
        try {
            if (-not $process.WaitForExit(10000) -or $process.ExitCode -ne 0) { throw 'Unsigned ordinary process cannot run' }
        } finally {
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
            $process.Dispose()
        }
    }
    Write-Output "Embedded Server UIAccess=$expected verified"
}
