import { useSettingsForm } from "../settings-form-context";
import { ClipboardHistorySection } from "../clipboard-history-section";
import { CLOUD_PANEL_SESSION_NOTE } from "../cloud-panel-session-notice";
import { GroupList } from "../../core/platform-controls";
import { OpenPanelRow } from "../open-panel-row";
import { SettingsPageFieldset } from "../settings-page-fieldset";
import { CloudClipboardRelaySection } from "../cloud-clipboard-relay-section";
import { withCloudClipboardRelay } from "../cloud-clipboard-relay";
import { clipboardHistoryLimit } from "../clipboard-history-preferences";

/** 设置表单的「剪贴板」页（路由 id 为 `tools`）：本设备上保存的剪贴板历史和云剪贴板，宿主支持时还有云剪贴板的同步方式（水杉账号或自建中继）。云词库在「词库」页。 */
export function ToolsSettingsPage() {
  const {
    client,
    iosPlatform,
    mobilePlatform,
    macosPlatform,
    snapshot,
    draft,
    setDraft,
    busy,
    setError,
    page,
    openPanel,
    clipboardHistory,
    toggleClipboardHistory,
    introTabs,
    selectIntroTab,
  } = useSettingsForm();
  // 已保存的中继配置决定云剪贴板走哪条路；在本页保存或改回账号后，历史记录一组要按新的路重新查可用状态。
  const savedRelay = snapshot?.preferences.cloud_clipboard_relay;
  const cloudRoute = savedRelay?.url ? `relay\n${savedRelay.url}\n${savedRelay.token}` : "account";
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "tools"} ariaLabel="剪贴板">
      <ClipboardHistorySection
        activeTab={introTabs.tools}
        onTabChange={(tab) => selectIntroTab("tools", tab)}
        client={client.clipboard}
        historyEnabled={clipboardHistory}
        historyLimit={clipboardHistoryLimit(draft)}
        persistedHistoryEnabled={snapshot?.preferences.clipboard_history ?? false}
        revision={snapshot?.revision}
        page={page}
        ios={iosPlatform}
        tabbed={!mobilePlatform}
        onToggle={toggleClipboardHistory}
        onLimitChange={(limit) =>
          setDraft((current) =>
            current ? { ...current, clipboard_history_limit: limit } : current,
          )
        }
        onError={setError}
        cloudRequest={page === "tools" ? client.cloudClipboardRequest : undefined}
        cloudRoute={cloudRoute}
        // macOS 的云剪贴板入口说明放在「剪贴板」设置内；关闭历史记录后仍能查看。
        note={macosPlatform ? CLOUD_PANEL_SESSION_NOTE : undefined}
      >
        {!macosPlatform && client.openCloudClipboard && (
          <GroupList title="云剪贴板">
            <OpenPanelRow
              title="云剪贴板"
              action={() => openPanel(client.openCloudClipboard)}
              label="打开云剪贴板"
            />
          </GroupList>
        )}
        {client.cloudClipboardRelay && (
          <CloudClipboardRelaySection
            relay={draft.cloud_clipboard_relay}
            disabled={busy}
            onChange={(relay) =>
              setDraft((current) => (current ? withCloudClipboardRelay(current, relay) : current))
            }
          />
        )}
      </ClipboardHistorySection>
    </SettingsPageFieldset>
  );
}
