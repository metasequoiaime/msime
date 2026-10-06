[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$LiteralPath,
    # arm64x is an ARM64 image that also carries ARM64EC code (hybrid metadata in its load configuration), which emulated x64 processes load on Windows on Arm.
    [Parameter(Mandatory)][ValidateSet('x86', 'x64', 'arm64', 'arm64x')][string]$Architecture,
    [Parameter(Mandatory)][ValidateSet('exe', 'dll')][string]$Kind
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Bounded header inspection only; never load or execute an unverified image.
# Layout: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
$stream = [IO.File]::Open($LiteralPath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
$reader = [IO.BinaryReader]::new($stream)
try {
    if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { throw 'Missing DOS image header' }
    $stream.Position = 0x3c
    $offset = $reader.ReadUInt32()
    if ($offset -lt 64 -or [long]$offset + 26 -gt $stream.Length) { throw 'Invalid PE header offset' }
    $stream.Position = $offset
    if ($reader.ReadUInt32() -ne 0x00004550) { throw 'Missing PE signature' }
    $machine = $reader.ReadUInt16()
    $sectionCount = $reader.ReadUInt16()
    $expected = switch ($Architecture) {
        'x64' { 0x8664 }
        'x86' { 0x14c }
        'arm64' { 0xaa64 }
        'arm64x' { 0xa64e }
        default { throw "Unsupported architecture: $Architecture" }
    }
    if ($machine -ne $expected) { throw 'PE architecture mismatch' }
    $stream.Position = [long]$offset + 20
    $optionalSize = $reader.ReadUInt16()
    $characteristics = $reader.ReadUInt16()
    if ($optionalSize -lt 2 -or [long]$offset + 24 + $optionalSize -gt $stream.Length) {
        throw 'Truncated PE optional header'
    }
    $magic = $reader.ReadUInt16()
    $expectedMagic = if ($Architecture -eq 'x86') { 0x10b } else { 0x20b }
    if ($magic -ne $expectedMagic) { throw 'PE optional header architecture mismatch' }
    if ($Architecture -in 'arm64', 'arm64x') {
        # The load configuration is data directory 10 of a PE32+ optional header, whose directories start at byte 112. Its CHPEMetadataPointer (byte 0xC8) is set only in an Arm64X image.
        $optionalStart = [long]$offset + 24
        if ($optionalSize -lt 112 + 8 * 11) { throw 'PE optional header has no load configuration directory' }
        $stream.Position = $optionalStart + 112 + 8 * 10
        $loadConfigRva = $reader.ReadUInt32()
        $loadConfigSize = $reader.ReadUInt32()
        $hybrid = $false
        if ($loadConfigRva -ne 0 -and $loadConfigSize -ge 0xd0) {
            $sectionTable = $optionalStart + $optionalSize
            for ($index = 0; $index -lt $sectionCount; $index++) {
                $stream.Position = $sectionTable + 40 * $index + 8
                $virtualSize = $reader.ReadUInt32()
                $virtualAddress = $reader.ReadUInt32()
                $rawSize = $reader.ReadUInt32()
                $rawPointer = $reader.ReadUInt32()
                if ($loadConfigRva -ge $virtualAddress -and $loadConfigRva -lt $virtualAddress + [Math]::Max($virtualSize, $rawSize)) {
                    $position = [long]$loadConfigRva - $virtualAddress + $rawPointer + 0xc8
                    if ($position + 8 -gt $stream.Length) { throw 'Truncated PE load configuration' }
                    $stream.Position = $position
                    $hybrid = $reader.ReadUInt64() -ne 0
                    break
                }
            }
        }
        if ($hybrid -ne ($Architecture -eq 'arm64x')) { throw 'PE Arm64X hybrid metadata mismatch' }
    }
    if (($characteristics -band 2) -eq 0 -or
        (($characteristics -band 0x2000) -ne 0) -ne ($Kind -eq 'dll')) {
        throw 'PE executable/DLL kind mismatch'
    }
} finally {
    $reader.Dispose()
}
