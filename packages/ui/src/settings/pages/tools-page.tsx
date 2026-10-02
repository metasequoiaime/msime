import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { ClipboardHistorySection } from "../clipboard-history-section";
import { CLOUD_PANEL_SESSION_NOTE } from "../cloud-panel-session-notice";
import { GroupList } from "../../core/platform-controls";
import { OpenPanelRow } from "../open-panel-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** 设置表单的「剪贴板」页（路由 id 为 `tools`）：本设备上保存的剪贴板历史和云剪贴板。云词库在「词库」页。 */
export function ToolsSettingsPage() {
  const {
    client,
    iosPlatform,
    macosPlatform,
    snapshot,
    busy,
    setError,
    page,
    openPanel,
    clipboardHistory,
    toggleClipboardHistory,
  } = useSettingsForm();
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "tools"} ariaLabel="剪贴板">
      <ClipboardHistorySection
        client={client.clipboard}
        historyEnabled={clipboardHistory}
        persistedHistoryEnabled={snapshot?.preferences.clipboard_history ?? false}
        revision={snapshot?.revision}
        page={page}
        ios={iosPlatform}
        onToggle={toggleClipboardHistory}
        onError={setError}
        cloudRequest={page === "tools" ? client.cloudClipboardRequest : undefined}
      />
      {(macosPlatform || client.openCloudClipboard) && (
        <GroupList title="云剪贴板">
          {macosPlatform ? (
            <p className={settings.groupNote}>{CLOUD_PANEL_SESSION_NOTE}</p>
          ) : (
            <OpenPanelRow
              title="云剪贴板"
              action={() => openPanel(client.openCloudClipboard)}
              label="打开云剪贴板"
            />
          )}
        </GroupList>
      )}
    </SettingsPageFieldset>
  );
}
