// 版本依据：当前发布工具链的 crtversion.h 实测为 14.51.36244.0（Windows CI 42805626142）。
// 微软支持约定要求 redist 不早于构建工具集；这不是由两次复验推算的最低不崩版本。
const
  VCRuntimeMinimumVersion = '14.51.36244.0';
  VCRuntimeMinimumMS = 14 * 65536 + 51;
  VCRuntimeMinimumBuild = 36244;

// Inno 6 的文件函数遵守 64 位安装模式；Inno 7 按进程架构显式重写路径。
// 不启动子进程，不依赖注册表键是否存在，也不把 SysWOW64 的 x86 文件当作 x64。
function VCRuntimeNativePath(const Path: String): String;
begin
#if Ver >= EncodeVer(7, 0, 0)
  Result := ApplyPathRedirRulesForCurrentProcess(True, Path);
#else
  Result := Path;
#endif
end;

function VCRuntimeFileIsCompatible(const Filename: String): Boolean;
var
  VersionMS, VersionLS: Cardinal;
begin
  Result := False;
  if not GetVersionNumbers(VCRuntimeNativePath(Filename), VersionMS, VersionLS) then
    exit;
  Result := (VersionMS > VCRuntimeMinimumMS) or
    ((VersionMS = VCRuntimeMinimumMS) and ((VersionLS shr 16) >= VCRuntimeMinimumBuild));
end;

function VCRuntimeIsInstalled: Boolean;
begin
  Result := IsWin64 and
    VCRuntimeFileIsCompatible(ExpandConstant('{sys}\msvcp140.dll')) and
    VCRuntimeFileIsCompatible(ExpandConstant('{sys}\vcruntime140.dll')) and
    VCRuntimeFileIsCompatible(ExpandConstant('{sys}\vcruntime140_1.dll'));
end;

{ 日志和判断读同一份实际文件，不显示可能缺失或过期的注册表值。 }
function ReadVCRuntimeVersion: String;
var
  Version: String;
begin
  Result := '';
  if GetVersionNumbersString(VCRuntimeNativePath(ExpandConstant('{sys}\msvcp140.dll')), Version) then
    Result := 'msvcp140=' + Version;
  if GetVersionNumbersString(VCRuntimeNativePath(ExpandConstant('{sys}\vcruntime140.dll')), Version) then
    Result := Result + '; vcruntime140=' + Version;
  if GetVersionNumbersString(VCRuntimeNativePath(ExpandConstant('{sys}\vcruntime140_1.dll')), Version) then
    Result := Result + '; vcruntime140_1=' + Version;
end;
