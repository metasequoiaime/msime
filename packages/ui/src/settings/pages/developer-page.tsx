import { McpConnectSection } from "../mcp-connect";
import { useSettingsForm } from "../settings-form-context";
import { DiagnosticLogsSection } from "../diagnostic-logs-section";
import { DataDirectorySection } from "../data-directory-section";
import { InputMethodServiceSection } from "../input-method-service-section";
import { UninstallSection } from "../uninstall-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/**
 * 维护与诊断页：依次是重启或重新注册输入法服务、诊断日志、数据目录、本地 `msime-mcp` 服务，最后是 macOS 的卸载。设计稿里的「显示调试信息」「日志级别」「导出诊断包」在任何宿主上都没有对应能力，所以不画；它的「重置所有设置」就是表单自己的「恢复默认设置」。
 */
export function DeveloperSettingsPage() {
  const {
    client,
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
    showRestartInputMethod,
    showInstallInputSource,
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
  const { onChooseDataDirectory, onDiagnosticLogChange, onConfirmUninstall } =
    createAboutSettingsActions({
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
      <DataDirectorySection
        visible={Boolean((macosPlatform || linuxPlatform) && client.dataDirectory)}
        linux={linuxPlatform}
        dataDirectory={dataDirectory}
        busy={dataDirectoryBusy}
        result={dataDirectoryResult}
        onChoose={onChooseDataDirectory}
      />
      {client.mcpServerStatus && (
        <McpConnectSection
          status={client.mcpServerStatus}
          install={client.installMcpClient}
          copyText={client.copyText}
        />
      )}
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
