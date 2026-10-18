import { McpConnectSection } from "../mcp-connect";
import { useSettingsForm } from "../settings-form-context";
import { DiagnosticLogsSection } from "../diagnostic-logs-section";
import { DataDirectorySection } from "../data-directory-section";
import { InputMethodServiceSection } from "../input-method-service-section";
import { UninstallSection } from "../uninstall-section";
import { createAboutSettingsActions } from "../about-settings-actions";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { SubPageEntries } from "./sub-page-entries";
import { useSettingsTabs } from "../use-settings-tabs";
import * as settings from "../settings-style";

const developerTabs = [
  { id: "developer", title: "维护诊断" },
  { id: "mcp", title: "AI助手" },
] as const;

/** 维护与诊断页：桌面通过顶部标签切换维护诊断与 AI 助手，手机保留二级入口。 */
export function DeveloperSettingsPage() {
  const {
    client,
    linuxPlatform,
    windowsPlatform,
    macosPlatform,
    mobilePlatform,
    requestedSettingsPage,
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
  const desktopMcp = !mobilePlatform && Boolean(client.mcpServerStatus);
  const { tabList, panel } = useSettingsTabs(
    developerTabs,
    "维护与诊断分类",
    desktopMcp,
    requestedSettingsPage === "mcp" ? "mcp" : "developer",
    selectPage,
    settings.dictionaryTabs,
  );
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "developer"} ariaLabel="维护与诊断">
      {tabList}
      {panel(
        "developer",
        <>
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
          {mobilePlatform && (
            <SubPageEntries
              title="AI 助手"
              pages={[
                {
                  id: "mcp",
                  description: "让本机的 AI 助手读诊断日志帮你排查问题，也能读写快捷短语和设置",
                },
              ]}
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
        </>,
      )}
      {desktopMcp && panel("mcp", <McpSettingsContent />)}
    </SettingsPageFieldset>
  );
}

/** 桌面标签与手机二级页面共用连接内容。 */
function McpSettingsContent() {
  const { client } = useSettingsForm();
  if (!client.mcpServerStatus) return null;
  return (
    <McpConnectSection
      status={client.mcpServerStatus}
      install={client.installMcpClient}
      copyText={client.copyText}
    />
  );
}

/** 手机保留 `mcp` 二级页面；桌面由维护页的顶部标签承载。 */
export function McpSettingsPage() {
  const { client, busy, page, mobilePlatform } = useSettingsForm();
  if (!mobilePlatform || !client.mcpServerStatus) return null;
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "mcp"} ariaLabel="连接 AI 助手">
      <McpSettingsContent />
    </SettingsPageFieldset>
  );
}
