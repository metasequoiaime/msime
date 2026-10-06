# Synthetic headers, deliberately not loadable images. For parser tests only.
function Write-PEFixture {
    param([string]$Path, [string]$Architecture = 'x64', [string]$Kind = 'exe')
    New-Item -ItemType Directory -Force (Split-Path -Parent $Path) | Out-Null
    $bytes = [byte[]]::new(1024)
    [BitConverter]::GetBytes([uint16]0x5a4d).CopyTo($bytes, 0)
    [BitConverter]::GetBytes([uint32]128).CopyTo($bytes, 60)
    [BitConverter]::GetBytes([uint32]0x4550).CopyTo($bytes, 128)
    $machine = switch ($Architecture) {
        'x64' { 0x8664 }
        'x86' { 0x14c }
        'arm64' { 0xaa64 }
        'arm64x' { 0xa64e }
        default { throw "Unsupported fixture architecture: $Architecture" }
    }
    $magic = if ($Architecture -eq 'x86') { 0x10b } else { 0x20b }
    [BitConverter]::GetBytes([uint16]$machine).CopyTo($bytes, 132)
    if ($Architecture -in 'arm64', 'arm64x') {
        # One section mapping RVA 0x1000 to file offset 512, holding a load configuration whose CHPEMetadataPointer is set only for arm64x.
        [BitConverter]::GetBytes([uint16]1).CopyTo($bytes, 134)
        [BitConverter]::GetBytes([uint32]0x1000).CopyTo($bytes, 152 + 112 + 8 * 10)
        [BitConverter]::GetBytes([uint32]0x100).CopyTo($bytes, 152 + 112 + 8 * 10 + 4)
        $section = 152 + 240
        [BitConverter]::GetBytes([uint32]0x200).CopyTo($bytes, $section + 8)
        [BitConverter]::GetBytes([uint32]0x1000).CopyTo($bytes, $section + 12)
        [BitConverter]::GetBytes([uint32]0x200).CopyTo($bytes, $section + 16)
        [BitConverter]::GetBytes([uint32]512).CopyTo($bytes, $section + 20)
        if ($Architecture -eq 'arm64x') { [BitConverter]::GetBytes([uint64]0x180002000).CopyTo($bytes, 512 + 0xc8) }
    }
    [BitConverter]::GetBytes([uint16]240).CopyTo($bytes, 148)
    $flags = if ($Kind -eq 'dll') { 0x2002 } else { 2 }
    [BitConverter]::GetBytes([uint16]$flags).CopyTo($bytes, 150)
    [BitConverter]::GetBytes([uint16]$magic).CopyTo($bytes, 152)
    [IO.File]::WriteAllBytes($Path, $bytes)
}
