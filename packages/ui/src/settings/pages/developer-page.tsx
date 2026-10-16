import { McpConnectSection } from "../mcp-connect";
import { useSettingsForm } from "../settings-form-context";
import { DiagnosticLogsSection } from "../diagnostic-logs-section";
import { DataDirectorySection } from "../data-directory-section";
import { InputMethodServiceSection } from "../input-method-service-section";
import { UninstallSection } from "../uninstall-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SubPageEntries } from "./sub-page-entries";

/**
 * 维护与诊断页：依次是重启输入法服务（macOS 是「修复输入源」）、诊断日志、数据目录、进入本地 `msime-mcp` 服务子页的一行，最后是 macOS 的卸载。设计稿里的「显示调试信息」「日志级别」「导出诊断包」在任何宿主上都没有对应能力，所以不画；它的「重置所有设置」就是表单自己的「恢复默认设置」。
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
      {/* 连接命令、配置和权限开关是这一页最长的一块，收进「连接 AI 助手」子页；宿主不报告 MCP 服务状态时没有这一页，也就没有这一行。 */}
      <SubPageEntries
        title="AI 助手"
        pages={[
          {
            id: "mcp",
            description: "让本机的 AI 助手读诊断日志帮你排查问题，也能读写快捷短语和设置",
          },
        ]}
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

/** 「维护与诊断」下的「连接 AI 助手」子页（路由 id 为 `mcp`）：本机 `msime-mcp` 服务的连接命令、配置、一键写入和两个权限开关。宿主不报告 MCP 服务状态时没有这一页（`settingsPageProjections` 的 `hasMcp`）。 */
export function McpSettingsPage() {
  const { client, busy, page } = useSettingsForm();
  if (!client.mcpServerStatus) return null;
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "mcp"} ariaLabel="连接 AI 助手">
      <McpConnectSection
        status={client.mcpServerStatus}
        install={client.installMcpClient}
        copyText={client.copyText}
      />
    </SettingsPageFieldset>
  );
}
