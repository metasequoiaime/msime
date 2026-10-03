; Metasequoia IME — Inno Setup script
; 源文件根目录：本脚本所在目录
;
; 编译方法：
;   1. 安装 Inno Setup 6.6 或更高版本：https://jrsoftware.org/isinfo.php
;   2. 运行 .\Compile-Installer.ps1（或用 Inno Setup Compiler 打开本文件并按 Ctrl+F9）
;   输出安装包默认在：Output\
;
; 本地测试打包顺序：
;   1. Prepare-PackageFiles.ps1         收集本版本的安装文件
;   2. Sign-PackageBinaries-Local.ps1   用本机自签名测试证书给包内 EXE/DLL 签名
;   3. 本文件用 Inno Setup 编译
;   4. Sign-Installer-Local.ps1         用同一张本机测试证书给安装包签名
;
; 也可以直接运行 .\test.ps1 走完整测试流程。
; 只改 TSF / Server 时用 .\test-light.ps1：ISCC /DLightPackage=1，
; 打出不含词库的轻量包，安装时也不会删本机已有词库。
; 本仓库不包含任何预置代码签名证书。

#include "editions.iss"
#define MyAppName      MyEditionAppName
#define MyAppVersion   "0.0.1"
#define MyAppPublisher "Metasequoia"
#define MyAppExeName   "MetasequoiaImeServer.exe"
#define MySettingsExeName "msime-client-settings.exe"
#define MyWatchdogName "MetasequoiaImeWatchdog.exe"
#define MyWatchdogTaskName MyEditionWatchdogTask
#define MyMcpName      "msime-mcp.exe"
; Global::MetasequoiaIMECLSID in platforms/windows/tsf/Global/Globals.cpp.
#define MyTipKey       "SOFTWARE\Microsoft\CTF\TIP\" + MyEditionClsid
#define MyVersionDirBase "msime_v" + MyAppVersion
#define MySourceRoot   "."
#ifdef LightPackage
#define MyOutputSuffix "_light"
#else
#define MyOutputSuffix ""
#endif

[Setup]
AppId={#MyEditionAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyEditionInstallDir}
DefaultGroupName={#MyAppName}
DisableDirPage=yes
; DisableDirPage=yes 时就绪页默认不显示目标目录，显式打开以便用户确认装到哪。
AlwaysShowDirOnReadyPage=yes
DisableProgramGroupPage=yes
OutputDir=Output
OutputBaseFilename={#MyEditionInstallerBaseName}_v{#MyAppVersion}{#MyOutputSuffix}
SetupIconFile={#MySourceRoot}\MetasequoiaIME.ico
Compression=lzma2
SolidCompression=yes
; 安装和卸载界面自动跟随 Windows 的浅色/深色模式。
WizardStyle=modern dynamic
PrivilegesRequired=admin
UsedUserAreasWarning=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={commonpf64}\{#MyEditionInstallDir}\MetasequoiaIME.ico
VersionInfoVersion={#MyAppVersion}

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Dirs]
Name: "{commonpf32}\{#MyEditionInstallDir}\{code:GetVersionDir}"
Name: "{commonpf64}\{#MyEditionInstallDir}\{code:GetVersionDir}"
Name: "{commonpf64}\{#MyEditionInstallDir}\server"
; Server 与设置窗口是中完整性的用户进程，要写这里的配置、用户词库和 runtime-options.json；安装器以高完整性建的目录它们改不动，数据目录放到其他盘时继承来的 ACL 也未必允许普通用户写。ssPostInstall 里的 EnsureImeUserDataDir 再对已有目录补一遍。
Name: "{code:GetDataDir}"; Permissions: users-modify

[Files]
; 独立安装应用图标，供 Windows“已安装的应用”列表稳定显示。
Source: "{#MySourceRoot}\MetasequoiaIME.ico"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}"; Flags: ignoreversion

; 第三方声明随包安装。词库主体含 rime-ice（GPL-3.0）内容，其许可要求保留署名，
; 因此这份文件必须落到用户磁盘上，而不能只存在于源码仓库里。
Source: "{#MySourceRoot}\THIRD_PARTY_NOTICES.txt"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}"; Flags: ignoreversion

; GPLv3 第 4、6 条要求分发时向接收者提供许可证副本，而 THIRD_PARTY_NOTICES.txt 只是指向
; "the LICENSE file"、本身不含 GPL 正文。macOS 与 Linux 的 CMake 安装规则早已随包装入许可证，
; Windows 是唯一大规模分发却漏掉这一步的平台。
Source: "{#MySourceRoot}\LICENSE.txt"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}"; Flags: ignoreversion

; TSF DLL 使用版本独立目录，避免升级时覆盖仍被进程加载的 DLL。
; PDB 与对应 DLL 放在同一目录，调试器可按二进制的内嵌路径自动找到符号。
; Install Host API and ordinary dependencies before registering the TIP.
Source: "{#MySourceRoot}\tsf_dll\32\*.dll"; \
    Excludes: "MetasequoiaImeTsf.dll"; \
    DestDir: "{commonpf32}\{#MyEditionInstallDir}\{code:GetVersionDir}"; \
    Flags: ignoreversion 32bit

Source: "{#MySourceRoot}\tsf_dll\64\*.dll"; \
    Excludes: "MetasequoiaImeTsf.dll"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}\{code:GetVersionDir}"; \
    Flags: ignoreversion

Source: "{#MySourceRoot}\tsf_dll\32\MetasequoiaImeTsf.dll"; \
    DestDir: "{commonpf32}\{#MyEditionInstallDir}\{code:GetVersionDir}"; \
    Flags: ignoreversion regserver 32bit

Source: "{#MySourceRoot}\tsf_dll\64\MetasequoiaImeTsf.dll"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}\{code:GetVersionDir}"; \
    Flags: ignoreversion regserver

Source: "{#MySourceRoot}\tsf_dll\32\*.pdb"; \
    DestDir: "{commonpf32}\{#MyEditionInstallDir}\{code:GetVersionDir}"; \
    Flags: ignoreversion

Source: "{#MySourceRoot}\tsf_dll\64\*.pdb"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}\{code:GetVersionDir}"; \
    Flags: ignoreversion

; server_exe 含本地语音识别运行时（sherpa-onnx-c-api.dll、onnxruntime.dll、onnxruntime_providers_shared.dll）。Server 从自身目录 LoadLibrary 加载它们，因此必须与 MetasequoiaImeServer.exe 同目录；ignoreversion 保证升级时换成本包锁定的版本。
Source: "{#MySourceRoot}\server_exe\*"; \
    DestDir: "{commonpf64}\{#MyEditionInstallDir}\server"; \
    Flags: ignoreversion recursesubdirs createallsubdirs

#ifndef LightPackage
; 包内故意不带 config.toml。通配复制再排除一次，防止以后又把用户配置打进包内。
Source: "{#MySourceRoot}\app_data\*"; DestDir: "{code:GetDataDir}"; \
    Excludes: "\config.toml,\config.default.toml"; \
    Flags: ignoreversion recursesubdirs createallsubdirs uninsneveruninstall

; 用户配置只在首次安装时从出厂模板生成。升级时绝不覆盖已有 config.toml。
Source: "{#MySourceRoot}\app_data\config.default.toml"; \
    DestDir: "{code:GetDataDir}"; DestName: "config.toml"; \
    Flags: onlyifdoesntexist uninsneveruninstall
#endif

[Icons]
Name: "{group}\{#MyAppName}"; \
    Filename: "{commonpf64}\{#MyEditionInstallDir}\server\{#MySettingsExeName}"; \
    WorkingDir: "{commonpf64}\{#MyEditionInstallDir}\server"
Name: "{group}\卸载 {#MyAppName}"; Filename: "{uninstallexe}"

[Registry]
Root: HKLM; Subkey: "{#MyEditionRegistryKey}"; \
    ValueType: string; ValueName: "VersionDir"; ValueData: "{code:GetVersionDir}"; \
    Flags: uninsdeletevalue
Root: HKLM; Subkey: "{#MyEditionRegistryKey}"; \
    ValueType: string; ValueName: "ServerPath"; \
    ValueData: "{commonpf64}\{#MyEditionInstallDir}\server\{#MyAppExeName}"; \
    Flags: uninsdeletevalue
Root: HKLM; Subkey: "{#MyEditionRegistryKey}"; \
    ValueType: string; ValueName: "DataDir"; ValueData: "{code:GetDataDir}"; \
    Flags: uninsdeletevalue

[Code]
const
  DataDirMarkerName = '{#MyEditionDataDirMarker}';
#if !MyEditionIsFull
  { 所有权标记的文件名按版本取（editions.iss，full 是 .metasequoiaime-data，本版本接上自己的名字后缀）。几个版本可以同时安装，full 的安装器只要看到 .metasequoiaime-data 就认目录归自己，所以本版本必须用别的文件名，full 才不会接管、清理或删除本版本的数据目录。 }
  { 本版本写进所有权标记的内容。标记里带着版本 id，OwnsDataDir 只认它，不认别的版本的标记。 }
  DataDirMarkerText = 'This directory is managed by Metasequoia IME (edition {#Edition}).';
#endif

  { WebView2 Evergreen Runtime 在 EdgeUpdate 里的固定客户端 ID。}
  WebView2ClientKey =
    'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';
  { InitializeSetup 在选语言对话框之前就跑，拿不到用户选的语言，界面本身也是中文，
    所以下载页统一给简体中文版，省得用户落到英文页上再自己切。}
  WebView2DownloadUrl =
    'https://developer.microsoft.com/zh-cn/microsoft-edge/webview2';
  VCRuntimeKey = 'Software\Microsoft\VisualStudio\14.0\VC\Runtimes\x64';
  VCRuntimeDownloadUrl =
    'https://learn.microsoft.com/zh-cn/cpp/windows/latest-supported-vc-redist?view=msvc-170';

var
  VersionDirName: String;
  DataDirPage: TInputDirWizardPage;
  NetworkPage: TInputOptionWizardPage;
  CloudCandidatesIndex: Integer;
  UserConfigExistedBeforeInstall: Boolean;
  DataDirValue: String;
  PreviousDataDir: String;
  { Set once MigrateUserDataDir has copied every user item out of a different previous directory; FinishDataDirMove only removes that directory when it is. }
  DataDirMigrated: Boolean;

{ WebView2 Runtime 与 VC 运行库都不随包分发：前者有自己的 Evergreen 更新通道，
  后者是系统级共享组件，安装器不该替用户装。但缺了任何一个，输入法装完就是坏的，
  而故障现场在安装结束之后——Server 起不来，或者点设置什么都不出现，用户看不到
  任何解释。所以在安装开始前查一遍，把原因当场说清楚。

  返回空串表示没装；否则是 pv 里的版本号。}
function ReadWebView2Version: String;
begin
  Result := '';
  { 机器级安装由 32 位的 EdgeUpdate 写入，键落在 WOW6432Node 下。本安装器跑在
    64 位模式，HKLM 默认就是 64 位视图，所以主查 HKLM32；HKLM64 作为后备，
    以防将来的 ARM64 / 原生 64 位布局换了位置。用户级安装写在 HKCU。
    卸载没清干净的机器上 pv 可能残留成 0.0.0.0。}
  if not RegQueryStringValue(HKLM32, WebView2ClientKey, 'pv', Result) then
    if not RegQueryStringValue(HKLM64, WebView2ClientKey, 'pv', Result) then
      if not RegQueryStringValue(HKCU, WebView2ClientKey, 'pv', Result) then
        Result := '';
  Result := Trim(Result);
  if Result = '0.0.0.0' then
    Result := '';
end;

// 用注册表判定，而不是去 System32 里找 vcruntime140.dll：Setup.exe 是 32 位进程，
// Pascal Script 的 FileExists 会被 WOW64 重定向到 SysWOW64（[Files] 和 Exec 里的
// {sys} 由 Inno 自己处理，脚本函数不管），结果查的是 x86 运行库——只装了 x64 redist
// 的机器会被误判成缺少 x64 运行库。绕开重定向的两个办法都不可移植：
// EnableFsRedirection 在 Inno Setup 7 里已被移除，而它的替代品在 6.x 里还不存在；
// {sysnative} 对 64 位安装器又不可用。注册表访问没有这个问题，HKLM32 / HKLM64
// 两个视图都是显式指定的。
//
// 要求 14.20 以上（VC++ 2019 起）而不是只看 Installed=1：只装了 2015/2017 版的
// 机器同样有 Installed=1，却缺少从那一版才开始随 redist 分发的 vcruntime140_1.dll。
// 32 位 TSF DLL 用的是静态 CRT（tsf/CMakeLists.txt 里的 MSVC_RUNTIME_LIBRARY），
// 所以不必检查 x86 redist。
// 注意：Inno Setup 的花括号注释不能嵌套，上面提到的常量会提前闭合 { } 注释，
// 所以这一段只能用行注释。
function VCRuntimeInstalledInView(RootKey: Integer): Boolean;
var
  Installed: Cardinal;
  Major: Cardinal;
  Minor: Cardinal;
begin
  Result :=
    RegQueryDWordValue(RootKey, VCRuntimeKey, 'Installed', Installed) and
    (Installed = 1) and
    RegQueryDWordValue(RootKey, VCRuntimeKey, 'Major', Major) and
    RegQueryDWordValue(RootKey, VCRuntimeKey, 'Minor', Minor) and
    ((Major > 14) or ((Major = 14) and (Minor >= 20)));
end;

{ redist 的安装包是 32 位的，键通常落在 WOW6432Node 下，但较新的版本两个视图都写，
  所以两边都查。}
function VCRuntimeIsInstalled: Boolean;
begin
  Result :=
    VCRuntimeInstalledInView(HKLM32) or VCRuntimeInstalledInView(HKLM64);
end;

{ 只用于写日志，不参与判定。}
function ReadVCRuntimeVersion: String;
begin
  Result := '';
  if not RegQueryStringValue(HKLM32, VCRuntimeKey, 'Version', Result) then
    if not RegQueryStringValue(HKLM64, VCRuntimeKey, 'Version', Result) then
      Result := '';
  Result := Trim(Result);
end;

procedure OpenDownloadPage(const Url: String);
var
  ErrorCode: Integer;
begin
  { 安装器是提权进程，用它拉起的浏览器会以管理员身份运行；交还给原用户。}
  ShellExecAsOriginalUser(
    'open', Url, '', '', SW_SHOWNORMAL, ewNoWait, ErrorCode);
end;

function InitializeSetup: Boolean;
var
  WebView2Version: String;
  NeedsWebView2: Boolean;
  NeedsVCRuntime: Boolean;
  Prompt: String;
begin
  Result := True;

  WebView2Version := ReadWebView2Version;
  NeedsWebView2 := WebView2Version = '';
  if NeedsWebView2 then
    Log('Prerequisite missing: Microsoft Edge WebView2 Runtime.')
  else
    Log('WebView2 Runtime found: ' + WebView2Version);

  NeedsVCRuntime := not VCRuntimeIsInstalled;
  if NeedsVCRuntime then
    Log('Prerequisite missing: Visual C++ 2015-2022 Redistributable (x64).')
  else
    Log('Visual C++ runtime found: ' + ReadVCRuntimeVersion);

  if not (NeedsWebView2 or NeedsVCRuntime) then
    exit;

  Prompt := '这台电脑缺少输入法运行所需的系统组件：' + #13#10;
  if NeedsVCRuntime then
    Prompt := Prompt + #13#10 +
      '● Visual C++ 2015-2022 可再发行组件包（x64）' + #13#10 +
      '   缺少它输入法主程序根本无法启动（提示找不到 VCRUNTIME140.dll 之类）。' + #13#10 +
      '   下载：' + VCRuntimeDownloadUrl + #13#10 +
      '   直链：https://aka.ms/vs/17/release/vc_redist.x64.exe' + #13#10;
  if NeedsWebView2 then
    Prompt := Prompt + #13#10 +
      '● Microsoft Edge WebView2 Runtime' + #13#10 +
      '   缺少它设置窗口，以及表情 / 手写 / 屏幕键盘 / 语音面板都打不开；' + #13#10 +
      '   候选窗口由 Direct2D 绘制，不受影响。' + #13#10 +
      '   下载：' + WebView2DownloadUrl + #13#10;
  Prompt := Prompt + #13#10 +
    '点击「是」打开下载页面并结束本次安装，装好组件后重新运行本安装程序；' + #13#10 +
    '点击「否」继续安装（装完仍需自行补齐上述组件，输入法才能正常工作）。';

  { 静默安装（CI、企业批量部署）不该被一个对话框卡住：默认继续，缺什么已经写进日志了。}
  if
    SuppressibleMsgBox(Prompt, mbConfirmation, MB_YESNO, IDNO) = IDYES
  then
  begin
    if NeedsVCRuntime then
      OpenDownloadPage(VCRuntimeDownloadUrl);
    if NeedsWebView2 then
      OpenDownloadPage(WebView2DownloadUrl);
    Result := False;
  end;
end;

function ResolvePreviousDataDir: String;
var
  Recorded: String;
begin
  if PreviousDataDir = '' then
  begin
    Recorded := '';
    if (RegQueryStringValue(
          HKLM,
          '{#MyEditionRegistryKey}',
          'DataDir',
          Recorded)) and (Trim(Recorded) <> '') then
      PreviousDataDir := RemoveBackslashUnlessRoot(Trim(Recorded))
    else
      PreviousDataDir := ExpandConstant('{localappdata}\{#MyEditionInstallDir}');
  end;
  Result := PreviousDataDir;
end;

function GetDataDir(Param: String): String;
var
  Requested: String;
begin
  if DataDirValue = '' then
  begin
#ifdef LightPackage
    { 轻量包不携带词库和静态资源，只能原地更新已有安装。}
    Requested := ResolvePreviousDataDir;
#else
    Requested := Trim(ExpandConstant('{param:DATADIR|}'));
    if Requested = '' then
      Requested := ResolvePreviousDataDir;
#endif
    DataDirValue := RemoveBackslashUnlessRoot(Trim(Requested));
  end;
  Result := DataDirValue;
end;

function UserConfigPath: String;
begin
  Result := AddBackslash(GetDataDir('')) + 'config.toml';
end;

#ifdef LightPackage
function LightPackageDataDirRejectionReason: String;
var
  Requested: String;
begin
  Result := '';
  Requested := Trim(ExpandConstant('{param:DATADIR|}'));
  if (Requested <> '') and
    (CompareText(
       RemoveBackslashUnlessRoot(Requested), ResolvePreviousDataDir) <> 0) then
    Result :=
      '轻量安装包不包含词库和静态资源，不能更换数据目录。' + #13#10 +
      '请使用完整安装包迁移数据目录。';
end;
#endif

function IsPathInside(const Child, Parent: String): Boolean;
begin
  Result :=
    (CompareText(Child, Parent) = 0) or
    (CompareText(
       Copy(AddBackslash(Child), 1, Length(AddBackslash(Parent))),
       AddBackslash(Parent)) = 0);
end;

function DirectoryIsEmpty(const Directory: String): Boolean;
var
  FindRec: TFindRec;
begin
  Result := True;
  if not DirExists(Directory) then
    Exit;
  if FindFirst(AddBackslash(Directory) + '*', FindRec) then
  begin
    try
      repeat
        if (FindRec.Name <> '.') and (FindRec.Name <> '..') then
        begin
          Result := False;
          Exit;
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

function DataDirMarkerPath(const Directory: String): String;
begin
  Result := AddBackslash(Directory) + DataDirMarkerName;
end;

function OwnsDataDir(const Directory: String): Boolean;
#if MyEditionIsFull
begin
  Result :=
    (CompareText(Directory, ExpandConstant('{localappdata}\{#MyEditionInstallDir}')) = 0) or
    FileExists(DataDirMarkerPath(Directory));
end;
#else
var
  Lines: TArrayOfString;
begin
  { 几个版本可以同时安装。只认写着本版本 id 的所有权标记：带着另一个版本标记的目录不归本安装器管，升级不能清理它，卸载也不能删它。 }
  Result :=
    (CompareText(Directory, ExpandConstant('{localappdata}\{#MyEditionInstallDir}')) = 0) or
    (LoadStringsFromFile(DataDirMarkerPath(Directory), Lines) and
     (GetArrayLength(Lines) > 0) and
     (Lines[0] = DataDirMarkerText));
end;
#endif

procedure WriteDataDirMarker(const Directory: String);
var
  Lines: TArrayOfString;
begin
  if not ForceDirectories(Directory) then
    Exit;
  if FileExists(DataDirMarkerPath(Directory)) then
    Exit;
  SetArrayLength(Lines, 1);
#if MyEditionIsFull
  Lines[0] := 'This directory is managed by Metasequoia IME.';
#else
  Lines[0] := DataDirMarkerText;
#endif
  SaveStringsToFile(DataDirMarkerPath(Directory), Lines, False);
end;

{ 返回空串表示目录可以安全交给输入法管理；否则返回面向用户的原因。
  卸载会递归删除带所有权标记的数据目录，因此不仅要拒绝系统目录本身，还要拒绝
  包含系统/用户关键目录的父级。未标记的非空目录也不能接管。}
function DataDirRejectionReason(const Directory: String): String;
var
  Critical: array[0..6] of String;
  ProtectedDirs: array[0..3] of String;
  Canonical: String;
  Index: Integer;
  ProbePath: String;
  WithSlash: String;
begin
  Result := '';
  if (Length(Directory) < 4) or (Directory[2] <> ':') or
    (Directory[3] <> '\') then
  begin
    Result := '请填写本机磁盘上的完整路径，例如 D:\MetasequoiaIME。';
    Exit;
  end;
  if not DirExists(Copy(Directory, 1, 3)) then
  begin
    Result := '找不到驱动器 ' + Copy(Directory, 1, 2) + '，请换一个位置。';
    Exit;
  end;
  if CompareText(RemoveBackslashUnlessRoot(Directory), Copy(Directory, 1, 2)) = 0 then
  begin
    Result := '不能直接使用驱动器根目录，请指定一个子目录。';
    Exit;
  end;

  Canonical := RemoveBackslashUnlessRoot(ExpandFileName(Directory));
  if CompareText(Canonical, Directory) <> 0 then
  begin
    Result := '请使用规范路径，不要包含重复分隔符或路径别名。';
    Exit;
  end;

  { 字符串安全检查必须在创建目录之前完成，防止用 .. 绕过下面的边界判断。}
  WithSlash := AddBackslash(Directory);
  if (Pos('\..\', WithSlash) > 0) or (Pos('\.\', WithSlash) > 0) or
    (Pos('/', Directory) > 0) then
  begin
    Result := '数据目录不能包含 .、.. 或正斜杠路径段。';
    Exit;
  end;

  ProtectedDirs[0] := ExpandConstant('{win}');
  ProtectedDirs[1] := ExpandConstant('{commonpf64}');
  ProtectedDirs[2] := ExpandConstant('{commonpf32}');
  ProtectedDirs[3] := ExpandConstant('{commonappdata}');
  for Index := 0 to 3 do
  begin
    if (ProtectedDirs[Index] <> '') and IsPathInside(Directory, ProtectedDirs[Index]) then
    begin
      Result := '数据目录不能放在系统或程序目录里面（' + ProtectedDirs[Index] + '）。';
      Exit;
    end;
  end;

  Critical[0] := ExpandConstant('{win}');
  Critical[1] := ExpandConstant('{commonpf64}');
  Critical[2] := ExpandConstant('{commonpf32}');
  Critical[3] := ExpandConstant('{localappdata}');
  Critical[4] := ExpandConstant('{userappdata}');
  Critical[5] := ExpandConstant('{%USERPROFILE|}');
  Critical[6] := ExpandConstant('{commonappdata}');
  for Index := 0 to 6 do
  begin
    if (Critical[Index] <> '') and IsPathInside(Critical[Index], Directory) then
    begin
      Result :=
        '这个目录包含了系统或用户的重要目录（' + Critical[Index] + '），' +
        '卸载时可能连它一起删除。请另选一个专用目录。';
      Exit;
    end;
  end;

  if not ForceDirectories(Directory) then
  begin
    Result := '无法创建目录 ' + Directory + '，请检查权限或换一个位置。';
    Exit;
  end;
  if (not DirectoryIsEmpty(Directory)) and (not OwnsDataDir(Directory)) then
  begin
    Result := '请选择一个空目录；这个目录已有文件且不属于水杉输入法。';
    Exit;
  end;
  { 不覆盖用户恰好已有的同名文件；从空闲名称中选一个写入再删除。}
  ProbePath := '';
  for Index := 0 to 999 do
  begin
    Canonical :=
      AddBackslash(Directory) + 'msime-write-probe-' + IntToStr(Index) + '.tmp';
    if (not FileExists(Canonical)) and (not DirExists(Canonical)) then
    begin
      ProbePath := Canonical;
      Break;
    end;
  end;
  if ProbePath = '' then
  begin
    Result := '无法在目录 ' + Directory + ' 中创建写入探针，请换一个位置。';
    Exit;
  end;
  if not SaveStringToFile(ProbePath, 'probe', False) then
  begin
    Result := '目录 ' + Directory + ' 不可写，请换一个位置。';
    Exit;
  end;
  if not DeleteFile(ProbePath) then
    Result := '无法清理目录 ' + Directory + ' 中的写入探针，请换一个位置。';
end;

procedure DataDirBrowseClick(Sender: TObject);
var
  Chosen: String;
begin
  Chosen := Trim(DataDirPage.Values[0]);
  if BrowseForFolder('请选择输入法数据的存放位置：', Chosen, True) then
    DataDirPage.Values[0] := Chosen;
end;

{ 云候选是唯一一个装完就会联网的功能：输入过程中把当前拼写发给 Google 的 input-tools 服务。
  出厂默认开启，而安装器此前没有任何一屏提到过它，用户要读文档才会知道。这一页把它摆到安装
  过程里，选择写进首次生成的 config.toml。

  升级时跳过：那时 config.toml 已经属于用户，安装器不该替他重新决定。}
procedure InitializeWizard;
begin
#ifndef LightPackage
  { 程序本体保留在 Program Files 以满足 uiAccess；可移动的是词库、配置和皮肤。}
  DataDirPage := CreateInputDirPage(
    wpLicense,
    '选择数据位置',
    '输入法数据存放在哪里',
    '请选择输入法数据（词库、配置、皮肤）的专用空目录。',
    True,
    '{#MyEditionInstallDir}'
  );
  DataDirPage.Add('');
  DataDirPage.Values[0] := GetDataDir('');
  DataDirPage.Buttons[0].OnClick := @DataDirBrowseClick;

  UserConfigExistedBeforeInstall := FileExists(UserConfigPath);
  NetworkPage := CreateInputOptionPage(
    DataDirPage.ID,
    '联网功能',
    '选择安装后哪些功能可以联网',
    '拼音切分、候选排序和词频学习全部在本机完成，不联网。' + #13#10 +
    '下面这一项是唯一一个装完就会发送输入内容的联网功能。AI 联想、候选翻译、语音输入都需要你自己填入 API token 之后才会发出任何请求。' + #13#10 +
    'Server 首次启动时会向 api.msime.app 注册一个本机匿名水杉账号，只发送本机随机生成的标识和口令，不含输入内容，失败时下次启动重试。' + #13#10 +
    '匿名使用统计默认开启，可在「设置 → 关于」里关闭：每天一条活跃记录、每次正常退出一条会话记录，崩溃后下次启动补发一条含异常摘要和调用栈（只有模块文件名和偏移）的崩溃记录；只带随机事件 id、本机随机生成的安装 id、平台名和版本号，不含输入内容、账号或设备信息。' + #13#10#13#10 +
    '安装后随时可以在「设置 → 输入」里改变云候选的选择。',
    False,
    False
  );
  CloudCandidatesIndex := NetworkPage.Add(
    '启用云候选：输入过程中把当前正在输入的拼写通过 HTTPS 发送给 Google 的 input-tools 服务' +
    '（inputtools.google.com），换回一条额外候选。已上屏的文本、词库内容和学习到的词频都不会发送。');
  NetworkPage.Values[CloudCandidatesIndex] := True;
#endif
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := False;
  if (NetworkPage <> nil) and (PageID = NetworkPage.ID) then
    Result := UserConfigExistedBeforeInstall;
end;

{ 只在本次安装刚生成 config.toml 时写入，且只改 [general] 段里的这一个键。找不到就什么都不做——
  这一步失败不应该让安装失败。}
procedure ApplyNetworkChoiceToUserConfig;
var
  Lines: TArrayOfString;
  Index: Integer;
  Trimmed: String;
  InGeneral: Boolean;
begin
  if UserConfigExistedBeforeInstall or (NetworkPage = nil) then
    Exit;
  if NetworkPage.Values[CloudCandidatesIndex] then
    Exit;
  if not LoadStringsFromFile(UserConfigPath, Lines) then
    Exit;

  InGeneral := False;
  for Index := 0 to GetArrayLength(Lines) - 1 do
  begin
    Trimmed := Trim(Lines[Index]);
    if (Length(Trimmed) > 0) and (Trimmed[1] = '[') then
      InGeneral := (Trimmed = '[general]')
    else if InGeneral and (Pos('cloud_candidates', Trimmed) = 1) then
    begin
      Lines[Index] := 'cloud_candidates = false';
      SaveStringsToFile(UserConfigPath, Lines, False);
      Exit;
    end;
  end;
end;

procedure LaunchInstalledComponents;
var
  ErrorCode: Integer;
begin
  { uiAccess=true 的程序不能用 CreateProcess 拉起（会报 740）。
    完成页点击 Finish 后，以原用户身份执行 ShellExecute（等同双击）。}
  ShellExecAsOriginalUser(
    '',
    ExpandConstant('{commonpf64}\{#MyEditionInstallDir}\server\{#MyAppExeName}'),
    '--production',
    '',
    SW_SHOWNORMAL,
    ewNoWait,
    ErrorCode
  );
  ShellExecAsOriginalUser(
    '',
    ExpandConstant('{commonpf64}\{#MyEditionInstallDir}\server\{#MyWatchdogName}'),
    '',
    '',
    SW_SHOWNORMAL,
    ewNoWait,
    ErrorCode
  );
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  Chosen: String;
  Reason: String;
begin
  Result := True;

  if (DataDirPage <> nil) and (CurPageID = DataDirPage.ID) then
  begin
    Chosen := RemoveBackslashUnlessRoot(Trim(DataDirPage.Values[0]));
    Reason := DataDirRejectionReason(Chosen);
    if Reason <> '' then
    begin
      MsgBox(Reason, mbError, MB_OK);
      Result := False;
      Exit;
    end;
    DataDirValue := Chosen;
#ifndef LightPackage
    { 网络选择页只应在所选目录尚无用户配置时出现。}
    UserConfigExistedBeforeInstall := FileExists(UserConfigPath);
#endif
    Exit;
  end;

  if (CurPageID = wpFinished) and (not WizardSilent) then
    LaunchInstalledComponents;
end;

function UpdateReadyMemo(
  Space, NewLine, MemoUserInfoInfo, MemoDirInfo, MemoTypeInfo,
  MemoComponentsInfo, MemoGroupInfo, MemoTasksInfo: String): String;
begin
  Result := MemoDirInfo + NewLine + NewLine +
    '数据目录（词库、用户配置、皮肤）：' + NewLine + Space + GetDataDir('');
  if CompareText(GetDataDir(''), ResolvePreviousDataDir) <> 0 then
    Result := Result + NewLine + NewLine +
      '现有数据将从这里迁移：' + NewLine + Space + ResolvePreviousDataDir;
end;

function GetVersionDir(Param: String): String;
var
  Candidate: String;
  Suffix: Integer;
begin
  if VersionDirName = '' then
  begin
    Candidate := '{#MyVersionDirBase}';
    Suffix := 0;
    while
      DirExists(ExpandConstant(
        '{commonpf32}\{#MyEditionInstallDir}\' + Candidate)) or
      DirExists(ExpandConstant(
        '{commonpf64}\{#MyEditionInstallDir}\' + Candidate))
    do
    begin
      Suffix := Suffix + 1;
      Candidate := '{#MyVersionDirBase}.' + IntToStr(Suffix);
    end;
    VersionDirName := Candidate;
  end;
  Result := VersionDirName;
end;

function IsUserConfigFile(const FileName: String): Boolean;
begin
  Result := CompareText(FileName, 'config.toml') = 0;
end;

function IsUserSkinDirectory(const FileName: String): Boolean;
begin
  { 外部皮肤在 %LOCALAPPDATA%\{#MyEditionInstallDir}\skins，升级安装不得清掉。}
  Result := CompareText(FileName, 'skins') = 0;
end;

{ DataDir 顶层里属于安装包的条目：Prepare-PackageFiles.ps1 放进 app_data 的每一项，每次完整安装都会重新写入。除此之外的一切都是用户状态：config.toml、skins，以及 Server 以 DataDir 为状态根写下的 preferences.json、user\、cache\、logs\、runtime-options.json、统计与剪贴板历史等。}
function IsPackageAppDataItem(const FileName: String): Boolean;
begin
  Result :=
    (CompareText(FileName, 'helpcodes') = 0) or
    (CompareText(FileName, 'audios') = 0) or
    (CompareText(FileName, 'sound-packs') = 0);
end;

function IsPreservedAppDataItem(const FileName: String): Boolean;
begin
  { 所有权标记也要留下。PrepareToInstall 先写标记再清理旧文件，ssPostInstall 才重写；安装若在两者之间失败，没有标记的非空自定义目录就不再被认作我们建的，重试安装会拒绝它，卸载也会跳过它。}
  { 升级只清安装包自己的条目。Server 的状态根就是 DataDir，只按名单保留会在每次完整升级时删掉 preferences.json、user\ 里的用户词库和其余状态。}
  Result :=
    IsUserConfigFile(FileName) or
    IsUserSkinDirectory(FileName) or
    (CompareText(FileName, DataDirMarkerName) = 0) or
    (not IsPackageAppDataItem(FileName));
end;

function InitializeUninstall(): Boolean;
begin
  RegQueryStringValue(
    HKLM,
    '{#MyEditionRegistryKey}',
    'VersionDir',
    VersionDirName
  );
  { DataDir 带 uninsdeletevalue，卸载过程中就被删掉了；到 usPostUninstall 再读只会回落到默认目录，自定义数据目录因此永远删不掉。这里先读出来缓存住。}
  ResolvePreviousDataDir;
  Result := True;
end;

#if MyEditionIsFull
procedure StopProcess(const ImageName: String);
var
  ResultCode: Integer;
begin
  Exec(
    ExpandConstant('{sys}\taskkill.exe'),
    '/F /T /IM "' + ImageName + '"',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
end;

procedure StopImeProcesses;
begin
  { Watchdog 必须先停，否则它可能在升级或卸载期间重新启动 Server。}
  StopProcess('{#MyWatchdogName}');
  StopProcess('{#MyAppExeName}');
  { 设置窗口是独立的 WinUI 3 msime-client-settings.exe；表情 / 手写 / 屏幕键盘面板仍由同目录的 Tauri MSIME.exe 承载。两个进程都不在 Server 的进程树里，覆盖安装和卸载前由安装器统一停止。}
  StopProcess('{#MySettingsExeName}');
  StopProcess('MSIME.exe');
  { AI 助手按需拉起的 msime-mcp.exe 同样不在 Server 的进程树里，助手开着就一直驻留，也会占住 server 目录。}
  StopProcess('{#MyMcpName}');
end;
#else
{ 只停可执行文件在本版本 server 目录里的进程。几个版本的 Server、看门狗、设置窗口、MSIME.exe 和 msime-mcp.exe 同名，按映像名结束（taskkill /IM）会把同时安装的其他版本一起停掉。ProcessName 为空时停目录里的全部进程。 }
procedure StopProcessesUnder(const Directory, ProcessName: String);
var
  ResultCode: Integer;
  Filter: String;
begin
  Filter := '';
  if ProcessName <> '' then
    Filter := ' -and ($_.ProcessName -ieq ''' + ProcessName + ''')';
  Exec(
    ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "' +
    '$root = ''' + AddBackslash(Directory) + '''; ' +
    'Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase)' + Filter + ' } | ' +
    'Stop-Process -Force -ErrorAction SilentlyContinue"',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
end;

procedure StopImeProcesses;
var
  ServerDir: String;
begin
  ServerDir := ExpandConstant('{commonpf64}\{#MyEditionInstallDir}\server');
  { Watchdog 必须先停，否则它可能在升级或卸载期间重新启动 Server。}
  StopProcessesUnder(ServerDir, 'MetasequoiaImeWatchdog');
  { 然后是 Server、WinUI 设置窗口、承载面板的 MSIME.exe 和 AI 助手拉起的 msime-mcp.exe：它们都在 server 目录里，也都不在 Server 的进程树里。}
  StopProcessesUnder(ServerDir, '');
end;
#endif

procedure DeleteWatchdogLogonTask;
var
  ResultCode: Integer;
begin
  { /F makes this idempotent when the task does not exist. }
  Exec(
    ExpandConstant('{sys}\schtasks.exe'),
    '/Delete /F /TN "{#MyWatchdogTaskName}"',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
end;

procedure EnsureImeUserDataDir;
var
  AppDataPath: String;
  ResultCode: Integer;
begin
  // [Dirs] 的 Permissions 只作用于目录本身；安装器以高完整性复制进去的资源和配置还带着高完整性标签和父目录继承来的 ACL，中完整性的 Server 与设置窗口改不动它们。给 Users 修改权限并降到中完整性。
  AppDataPath := GetDataDir('');
  ForceDirectories(AppDataPath);
  Exec(
    ExpandConstant('{sys}\icacls.exe'),
    '"' + AppDataPath + '" /grant *S-1-5-32-545:(OI)(CI)M /T /C /Q',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
  Exec(
    ExpandConstant('{sys}\icacls.exe'),
    '"' + AppDataPath + '" /setintegritylevel (OI)(CI)M /T /C /Q',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  );
end;

procedure CreateWatchdogLogonTask;
var
  WatchdogPath: String;
  Params: String;
  ResultCode: Integer;
begin
  ResultCode := -1;
  WatchdogPath := ExpandConstant(
    '{commonpf64}\{#MyEditionInstallDir}\server\{#MyWatchdogName}');
  { /F replaces the same fixed-name task during an upgrade. /IT keeps the
    task in the interactive user's session; LIMITED avoids an elevated token.
    schtasks splits the /TR value at its first space into program and arguments unless the program itself is quoted, so the Program Files path would become the program "C:\Program" with the rest as its arguments. The escaped inner quotes survive schtasks' own argument parsing and keep the path whole. }
  Params :=
    '/Create /F /TN "{#MyWatchdogTaskName}" /SC ONLOGON ' +
    '/RL LIMITED /IT /TR "\"' + WatchdogPath + '\""';
  if
    (not Exec(
      ExpandConstant('{sys}\schtasks.exe'),
      Params,
      '',
      SW_HIDE,
      ewWaitUntilTerminated,
      ResultCode
    )) or
    (ResultCode <> 0)
  then
    RaiseException(
      '无法创建输入法登录启动任务（退出码：' +
      IntToStr(ResultCode) + '）。');
end;

procedure TryDeleteTree(const Path: String);
begin
  { 忽略返回值：被占用文件保留，其他能删除的文件仍继续清理。}
  DelTree(Path, True, True, True);
end;

procedure CleanAppDataExceptUserFiles;
var
  AppDataPath: String;
  FindRec: TFindRec;
  ItemPath: String;
begin
  AppDataPath := GetDataDir('');
  if not OwnsDataDir(AppDataPath) then
    exit;
  if not DirExists(AppDataPath) then
    exit;

  if FindFirst(AddBackslash(AppDataPath) + '*', FindRec) then
  begin
    try
      repeat
        if
          (FindRec.Name <> '.') and
          (FindRec.Name <> '..') and
          (not IsPreservedAppDataItem(FindRec.Name))
        then
        begin
          ItemPath := AddBackslash(AppDataPath) + FindRec.Name;
          if (FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
            TryDeleteTree(ItemPath)
          else
            DeleteFile(ItemPath);
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

procedure TryDeleteOldVersionDirs(const RootPath: String);
var
  FindRec: TFindRec;
begin
  if FindFirst(
    AddBackslash(RootPath) + 'msime_v*',
    FindRec
  ) then
  begin
    try
      repeat
        if
          ((FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0) and
          (FindRec.Name <> '.') and
          (FindRec.Name <> '..') and
          (CompareText(FindRec.Name, VersionDirName) <> 0)
        then
          TryDeleteTree(AddBackslash(RootPath) + FindRec.Name);
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

{ 迁移到新目录的条目：安装包条目由本次安装重新写入，所有权标记由 PrepareToInstall 另写，写入探针是 DataDirRejectionReason 的残留；runtime-options.json 记着旧目录的绝对路径，不带过去，Server 首次启动时会在新目录里重新生成（FirstRun.h）。}
function IsMigratedDataItem(const FileName: String): Boolean;
begin
  Result :=
    (not IsPackageAppDataItem(FileName)) and
    (CompareText(FileName, DataDirMarkerName) <> 0) and
    (CompareText(FileName, 'runtime-options.json') <> 0) and
    (CompareText(FileName, '.runtime-options-prepared') <> 0) and
    (CompareText(Copy(FileName, 1, 18), 'msime-write-probe-') <> 0);
end;

function RobocopySucceeded(const Params: String): Boolean;
var
  ResultCode: Integer;
begin
  ResultCode := -1;
  Result := Exec(
    ExpandConstant('{sys}\robocopy.exe'),
    Params + ' /COPY:DAT /R:2 /W:1 /NJH /NJS /NP /NFL /NDL',
    '',
    SW_HIDE,
    ewWaitUntilTerminated,
    ResultCode
  ) and (ResultCode >= 0) and (ResultCode < 8);
end;

{ 与来源安装包一样是"移动"数据目录：旧目录里的全部用户状态搬到新目录，安装成功后旧目录被删除。但分两步做：这里只复制，原目录保持不动；删除推迟到 ssPostInstall 的最后（FinishDataDirMove），登录任务失败会在那之前中止，旧数据因此在任何半途失败后都还在。来源安装包直接使用破坏性的移动参数，主库可能先被移走而 WAL 或配置失败，两边都不剩完整状态。}
function MigrateUserDataDir(const OldDir, NewDir: String): String;
var
  FindRec: TFindRec;
  Source: String;
  Destination: String;
begin
  Result := '';
  DataDirMigrated := False;
  if (OldDir = '') or (CompareText(OldDir, NewDir) = 0) or
    (not DirExists(OldDir)) then
    exit;

  Log('Copying user data; the previous directory is kept until installation succeeds.');
  if not ForceDirectories(NewDir) then
  begin
    Result := '无法创建新的数据目录：' + NewDir;
    exit;
  end;

  if FindFirst(AddBackslash(OldDir) + '*', FindRec) then
  begin
    try
      repeat
        Source := AddBackslash(OldDir) + FindRec.Name;
        Destination := AddBackslash(NewDir) + FindRec.Name;
        { 新目录就在这一项里面（新目录是旧目录的子目录）时跳过：它不是用户数据，不能复制进自己。}
        if (FindRec.Name <> '.') and (FindRec.Name <> '..') and
          IsMigratedDataItem(FindRec.Name) and
          (not IsPathInside(NewDir, Source)) then
        begin
          { 目标恰好就是旧目录或它的上级（旧目录是新目录的子目录且同名），复制会写进源头自身。}
          if IsPathInside(OldDir, Destination) then
          begin
            Result := '旧数据目录与新数据目录中的 ' + FindRec.Name + ' 位置冲突，安装已停止；原目录中的数据保持不变。请另选一个数据目录。';
            exit;
          end;
          if (FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
          begin
            if not RobocopySucceeded('"' + Source + '" "' + Destination + '" /E') then
            begin
              Result := '用户数据复制失败（' + FindRec.Name + '），安装已停止；原目录中的数据保持不变。请关闭相关程序并检查目标磁盘后重试。';
              exit;
            end;
          end
          else if not RobocopySucceeded('"' + OldDir + '" "' + NewDir + '" "' + FindRec.Name + '"') then
          begin
            Result := '用户数据复制失败（' + FindRec.Name + '），安装已停止；原目录中的数据保持不变。请关闭相关程序并检查目标磁盘后重试。';
            exit;
          end;
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
  DataDirMigrated := True;
end;

{ 安装全部成功后才删除旧目录，而且只删带所有权标记（或默认位置）的目录。新目录在旧目录里面时不能递归删除整个旧目录——来源安装包就是这样把刚迁过去的数据一起删掉的——只删旧目录顶层除新目录所在那一项之外的条目。}
procedure FinishDataDirMove;
var
  OldDir: String;
  NewDir: String;
  FindRec: TFindRec;
  ItemPath: String;
begin
  if not DataDirMigrated then
    exit;
  OldDir := ResolvePreviousDataDir;
  NewDir := GetDataDir('');
  if (CompareText(OldDir, NewDir) = 0) or (not DirExists(OldDir)) or
    (not OwnsDataDir(OldDir)) then
    exit;
  if not IsPathInside(NewDir, OldDir) then
  begin
    Log('Removing the previous data directory after a successful move.');
    TryDeleteTree(OldDir);
    exit;
  end;
  Log('Removing the previous data directory''s entries around the new one inside it.');
  if FindFirst(AddBackslash(OldDir) + '*', FindRec) then
  begin
    try
      repeat
        if (FindRec.Name <> '.') and (FindRec.Name <> '..') then
        begin
          ItemPath := AddBackslash(OldDir) + FindRec.Name;
          if not IsPathInside(NewDir, ItemPath) then
          begin
            if (FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
              TryDeleteTree(ItemPath)
            else
              DeleteFile(ItemPath);
          end;
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  MigrationError: String;
  DataDirError: String;
begin
  { 先锁定本次目录名，再清理能够释放的旧版本 DLL。}
  VersionDirName := GetVersionDir('');
#ifdef LightPackage
  DataDirError := LightPackageDataDirRejectionReason;
  if DataDirError <> '' then
  begin
    Result := DataDirError;
    exit;
  end;
#endif
  DataDirError := DataDirRejectionReason(GetDataDir(''));
  if DataDirError <> '' then
  begin
    Result := DataDirError;
    exit;
  end;
  { 在复制旧数据之前取得这个空目录的所有权。否则复制或后续安装失败会留下一个
    非空、无标记的半成品，下一次重试会按安全规则拒绝继续。}
  WriteDataDirMarker(GetDataDir(''));
  if not OwnsDataDir(GetDataDir('')) then
  begin
    Result := '无法写入数据目录所有权标记，请检查目录权限后重试。';
    exit;
  end;
  StopImeProcesses;
  MigrationError := MigrateUserDataDir(ResolvePreviousDataDir, GetDataDir(''));
  if MigrationError <> '' then
  begin
    Result := MigrationError;
    exit;
  end;
#ifndef LightPackage
  { 轻量包不带 app_data，只替换 Server/TSF。完整包先清掉安装包自己的条目，用户配置、皮肤和 Server 状态保留。}
  CleanAppDataExceptUserFiles;
#endif
  TryDeleteTree(ExpandConstant(
    '{commonpf64}\{#MyEditionInstallDir}\server'));
  TryDeleteOldVersionDirs(ExpandConstant(
    '{commonpf32}\{#MyEditionInstallDir}'));
  TryDeleteOldVersionDirs(ExpandConstant(
    '{commonpf64}\{#MyEditionInstallDir}'));
  { 随后的 [Files] 与 ssPostInstall 会写入新 Server 和登录任务。}
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    WriteDataDirMarker(GetDataDir(''));
#ifndef LightPackage
    ApplyNetworkChoiceToUserConfig;
#endif
    CreateWatchdogLogonTask;
    EnsureImeUserDataDir;
    { Last: every step above that can fail raises before this, so a failed install keeps the previous data directory. }
    FinishDataDirMove;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    DeleteWatchdogLogonTask;
    StopImeProcesses;
  end
  else if CurUninstallStep = usPostUninstall then
  begin
    { Both TSF DLLs have been unregistered by now. DllUnregisterServer removes the language profile but, like the SampleIME code it came from, never calls ITfInputProcessorProfiles::Unregister, so the TIP key and its category entries stay behind; remove them from both registry views. }
    RegDeleteKeyIncludingSubkeys(HKLM64, '{#MyTipKey}');
    RegDeleteKeyIncludingSubkeys(HKLM32, '{#MyTipKey}');
    TryDeleteTree(ExpandConstant(
      '{commonpf64}\{#MyEditionInstallDir}\server'));
    if VersionDirName <> '' then
    begin
      TryDeleteTree(ExpandConstant(
        '{commonpf32}\{#MyEditionInstallDir}\' + VersionDirName));
      TryDeleteTree(ExpandConstant(
        '{commonpf64}\{#MyEditionInstallDir}\' + VersionDirName));
    end;
    TryDeleteTree(ExpandConstant('{commonpf32}\{#MyEditionInstallDir}'));
    TryDeleteTree(ExpandConstant('{commonpf64}\{#MyEditionInstallDir}'));
    { 用 InitializeUninstall 缓存的路径：此时注册表里的 DataDir 已被删除。}
    if OwnsDataDir(ResolvePreviousDataDir) then
      TryDeleteTree(ResolvePreviousDataDir);
  end;
end;
