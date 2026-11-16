#ifndef FixtureRoot
#error FixtureRoot is required
#endif
#ifndef ExpectedKernelHash
#error ExpectedKernelHash is required
#endif

[Setup]
AppName=MSIME synthetic runtime probe
AppVersion=1.0
DefaultDirName={tmp}\msime-runtime-probe
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=vc-runtime-probe
#ifdef NativeProbe
SetupArchitecture=x64
#endif

[Code]
#include "..\VcRuntimePrerequisite.iss"

procedure CheckFile(const Name: String; Expected: Boolean);
var
  ActualVersion: String;
begin
  if VCRuntimeFileIsCompatible('{#FixtureRoot}\' + Name) <> Expected then
  begin
    GetVersionNumbersString('{#FixtureRoot}\' + Name, ActualVersion);
    RaiseException('Unexpected runtime compatibility: ' + Name + ' version=' + ActualVersion);
  end;
end;

function InitializeSetup: Boolean;
begin
  { 只读合成 DLL 和系统文件，始终在真正安装前退出。 }
  Result := False;
  try
    CheckFile('old.dll', False);
    CheckFile('old-minor.dll', False);
    CheckFile('before-minimum.dll', False);
    CheckFile('minimum.dll', True);
    CheckFile('reporter-fixed.dll', True);
    CheckFile('new-minor.dll', True);
    CheckFile('missing.dll', False);
    CheckFile('plain.txt', False);
    if CompareText(GetSHA256OfFile(VCRuntimeNativePath(ExpandConstant('{sys}\kernel32.dll'))),
        '{#ExpectedKernelHash}') <> 0 then
      RaiseException('Runtime lookup selected the wrong system directory');
    SaveStringToFile('{#FixtureRoot}\result.txt', 'PASSED', False);
  except
    SaveStringToFile('{#FixtureRoot}\result.txt', 'FAILED: ' + GetExceptionMessage, False);
  end;
end;
