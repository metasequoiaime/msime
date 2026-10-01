import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { ClipboardHistorySection } from "../clipboard-history-section";
import { CLOUD_PANEL_SESSION_NOTE } from "../cloud-panel-session-notice";
import { GroupList, Row } from "../../core/platform-controls";
import { OpenPanelButton } from "../open-panel-button";

/** The 云剪贴板 page of the settings form (route id `tools`): the clipboard history kept on this device and the cloud panels. */
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
    <fieldset disabled={busy} hidden={page !== "tools"} aria-label="云剪贴板">
      <div className={settings.groups}>
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
        {(macosPlatform || client.openCloudClipboard || client.openCloudDictionary) && (
          <GroupList title="云端面板">
            {macosPlatform ? (
              <p className={settings.groupNote}>{CLOUD_PANEL_SESSION_NOTE}</p>
            ) : (
              <>
                {client.openCloudClipboard && (
                  <Row title="云剪贴板">
                    <OpenPanelButton
                      action={() => openPanel(client.openCloudClipboard)}
                      label="打开云剪贴板"
                    />
                  </Row>
                )}
                {client.openCloudDictionary && (
                  <Row title="云词典">
                    <OpenPanelButton
                      action={() => openPanel(client.openCloudDictionary)}
                      label="打开云词典"
                    />
                  </Row>
                )}
              </>
            )}
          </GroupList>
        )}
      </div>
    </fieldset>
  );
}
