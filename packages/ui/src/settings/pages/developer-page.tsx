import * as settings from "../settings-style";
import { McpConnectSection } from "../mcp-connect";
import { useSettingsForm } from "../settings-form-context";
import { DiagnosticLogsSection } from "../diagnostic-logs-section";
import { DataDirectorySection } from "../data-directory-section";
import { createAboutSettingsActions } from "../about-settings-actions";

/**
 * The 开发者选项 page: the local `msime-mcp` server, the diagnostic logs and where the data lives. The design's 显示调试信息, 日志级别 and 导出诊断包 have no counterpart in any host and are not drawn; its 重置所有设置 is the form's own 恢复默认设置.
 */
export function DeveloperSettingsPage() {
  const {
    client,
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
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
  } = useSettingsForm();
  const { onChooseDataDirectory, onDiagnosticLogChange } = createAboutSettingsActions({
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage,
    setDraft,
  });
  return (
    <fieldset disabled={busy} hidden={page !== "developer"} aria-label="开发者选项">
      <div className={settings.groups}>
        {client.mcpServerStatus && (
          <McpConnectSection
            status={client.mcpServerStatus}
            install={client.installMcpClient}
            copyText={client.copyText}
          />
        )}
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
      </div>
    </fieldset>
  );
}
