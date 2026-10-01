import * as settings from "../settings-style";
import { McpConnectSection } from "../mcp-connect";
import { useSettingsForm } from "../settings-form-context";
import { DiagnosticLogsSection } from "../diagnostic-logs-section";
import { DataDirectorySection } from "../data-directory-section";
import { InputMethodServiceSection } from "../input-method-service-section";
import { createAboutSettingsActions } from "../about-settings-actions";

/**
 * 维护与诊断页：重启或重新注册输入法服务、本地 `msime-mcp` 服务、诊断日志和数据目录。设计稿里的「显示调试信息」「日志级别」「导出诊断包」在任何宿主上都没有对应能力，所以不画；它的「重置所有设置」就是表单自己的「恢复默认设置」。
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
  } = useSettingsForm();
  const { onChooseDataDirectory, onDiagnosticLogChange } = createAboutSettingsActions({
    checkForUpdate,
    chooseDataDirectory,
    confirmUninstall,
    selectPage,
    setDraft,
  });
  return (
    <fieldset disabled={busy} hidden={page !== "developer"} aria-label="维护与诊断">
      <div className={settings.groups}>
        <InputMethodServiceSection
          visible={Boolean(showRestartInputMethod)}
          macos={macosPlatform}
          linux={linuxPlatform}
          restartInputMethod={client.restartInputMethod}
          installInputSource={showInstallInputSource ? client.installInputSource : undefined}
        />
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
