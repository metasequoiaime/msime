import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { ClipboardHistorySection } from "../clipboard-history-section";
import { GroupList, Row } from "../../core/platform-controls";

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
        />
        {(macosPlatform || client.openCloudClipboard || client.openCloudDictionary) && (
          <GroupList title="云端面板">
            {macosPlatform ? (
              <p className={settings.groupNote}>
                云剪贴板和云词典需要当前输入法进程提供输入会话；请从输入法悬浮工具栏或输入法菜单打开对应面板。
              </p>
            ) : (
              <>
                {client.openCloudClipboard && (
                  <Row title="云剪贴板">
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void openPanel(client.openCloudClipboard)}
                    >
                      打开云剪贴板
                    </button>
                  </Row>
                )}
                {client.openCloudDictionary && (
                  <Row title="云词典">
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void openPanel(client.openCloudDictionary)}
                    >
                      打开云词典
                    </button>
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
