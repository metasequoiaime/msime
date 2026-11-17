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

function VCRuntimeFileStatus(const Filename, DisplayName: String): String;
var
  VersionMS, VersionLS: Cardinal;
  Version: String;
  NativeFilename: String;
begin
  NativeFilename := VCRuntimeNativePath(Filename);
  if not FileExists(NativeFilename) then
  begin
    Result := DisplayName + '=缺失';
    exit;
  end;
  if not GetVersionNumbers(NativeFilename, VersionMS, VersionLS) then
  begin
    Result := DisplayName + '=无法读取版本';
    exit;
  end;
  if not GetVersionNumbersString(NativeFilename, Version) then
    Version := '未知版本';
  Result := DisplayName + '=' + Version;
  if not VCRuntimeFileIsCompatible(Filename) then
    Result := Result + '（低于' + VCRuntimeMinimumVersion + '）';
end;

{ 日志和判断读同一组实际文件，不显示可能残留或不完整的注册表登记。 }
function ReadVCRuntimeVersion: String;
begin
  Result := VCRuntimeFileStatus(
    ExpandConstant('{sys}\msvcp140.dll'), 'msvcp140.dll');
  Result := Result + '; ' + VCRuntimeFileStatus(
    ExpandConstant('{sys}\vcruntime140.dll'), 'vcruntime140.dll');
  Result := Result + '; ' + VCRuntimeFileStatus(
    ExpandConstant('{sys}\vcruntime140_1.dll'), 'vcruntime140_1.dll');
end;
