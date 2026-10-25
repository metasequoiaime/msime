import { useSettingsForm } from "../settings-form-context";
import { DiagnosticLogsSection } from "../diagnostic-logs-section";
import { CommandLineToolsSection } from "../command-line-tools-section";
import { commandLineCandidatesEnabled } from "../diagnostic-log-preferences";
import { DataDirectorySection } from "../data-directory-section";
import { InputMethodServiceSection } from "../input-method-service-section";
import { UninstallSection } from "../uninstall-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** 维护与诊断页：只放输入法服务、诊断日志、命令行工具、数据目录和卸载等维护操作。 */
export function DeveloperSettingsPage() {
  const {
    client,
    draft,
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
    showRestartInputMethod,
    showInstallInputSource,
    refreshInputSourceStartup,
    setDraft,
    busy,
    setError,
    dataDirectory,
    dataDirectoryBusy,
    dataDirectoryResult,
    chooseDataDirectory,
    diagnosticLog,
    page,
    checkForUpdate,
    confirmUninstall,
    selectPage,
    removeUserDataOnUninstall,
    setRemoveUserDataOnUninstall,
    uninstallConfirmation,
    uninstallBusy,
    uninstallResult,
    requestUninstall,
    cancelUninstall,
  } = useSettingsForm();
  const {
    onChooseDataDirectory,
    onDiagnosticLogChange,
    onCommandLineCandidatesChange,
    onConfirmUninstall,
  } = createAboutSettingsActions({
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage,
    setDraft,
  });
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "developer"} ariaLabel="维护与诊断">
      <InputMethodServiceSection
        visible={Boolean(showRestartInputMethod)}
        macos={macosPlatform}
        linux={linuxPlatform}
        restartInputMethod={client.restartInputMethod}
        installInputSource={showInstallInputSource ? client.installInputSource : undefined}
        onRepairFinished={refreshInputSourceStartup}
      />
      <DiagnosticLogsSection
        visible={!client.host || linuxPlatform || windowsPlatform || macosPlatform}
        linux={linuxPlatform}
        macos={macosPlatform}
        windows={windowsPlatform || !client.host}
        values={diagnosticLog}
        openDirectory={client.openDiagnosticLogDirectory}
        onChange={onDiagnosticLogChange}
        onError={setError}
      />
      {/* 只有随包安装了 msime-mcp 命令的桌面宿主显示：macOS 和 Linux 的包里另有 msime 链接，Windows 只有 msime-mcp.exe；HarmonyOS 和手机没有这个命令。 */}
      <CommandLineToolsSection
        visible={macosPlatform || linuxPlatform || windowsPlatform}
        windows={windowsPlatform}
        candidates={commandLineCandidatesEnabled(draft)}
        onCandidatesChange={onCommandLineCandidatesChange}
      />
      <DataDirectorySection
        visible={Boolean((macosPlatform || linuxPlatform) && client.dataDirectory)}
        linux={linuxPlatform}
        dataDirectory={dataDirectory}
        busy={dataDirectoryBusy}
        result={dataDirectoryResult}
        onChoose={onChooseDataDirectory}
      />
      {macosPlatform && (
        <UninstallSection
          uninstallInputSource={client.uninstallInputSource}
          removeUserData={removeUserDataOnUninstall}
          uninstallBusy={uninstallBusy}
          uninstallConfirmation={uninstallConfirmation}
          uninstallResult={uninstallResult}
          onRemoveUserDataChange={setRemoveUserDataOnUninstall}
          onRequestUninstall={requestUninstall}
          onConfirmUninstall={onConfirmUninstall}
          onCancelUninstall={cancelUninstall}
        />
      )}
    </SettingsPageFieldset>
  );
}
