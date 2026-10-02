import type { ClipboardHistoryClient } from "./clipboard-history-section";
import { ClipboardHistorySection } from "./clipboard-history-section";
import { CloudPanelSessionNotice } from "./cloud-panel-session-notice";
import { LocalModesSection, type LocalModePreferences } from "./local-modes-section";

export interface UtilitiesSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  clipboard?: ClipboardHistoryClient;
  historyEnabled: boolean;
  persistedHistoryEnabled: boolean;
  revision?: number;
  page?: string;
  ios: boolean;
  macos: boolean;
  onToggleClipboard: (enabled: boolean) => void;
  onError: (message: string) => void;
  openCloudClipboard?: () => Promise<void>;
  openCloudDictionary?: () => Promise<void>;
  onOpenPanel: (action: (() => Promise<void>) | undefined) => Promise<void>;
  localModes: LocalModePreferences;
  onLocalModesChange: (preferences: LocalModePreferences) => void;
}

/** Utility settings shared by desktop and mobile settings hosts. */
export function UtilitiesSettingsSection({
  disabled,
  hidden,
  clipboard,
  historyEnabled,
  persistedHistoryEnabled,
  revision,
  page,
  ios,
  macos,
  onToggleClipboard,
  onError,
  openCloudClipboard,
  openCloudDictionary,
  onOpenPanel,
  localModes,
  onLocalModesChange,
}: UtilitiesSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="实用功能">
      <ClipboardHistorySection
        client={clipboard}
        historyEnabled={historyEnabled}
        persistedHistoryEnabled={persistedHistoryEnabled}
        revision={revision}
        page={page}
        ios={ios}
        onToggle={onToggleClipboard}
        onError={onError}
      >
        {macos ? (
          <CloudPanelSessionNotice />
        ) : (
          <>
            {openCloudClipboard && (
              <button
                type="button"
                className="secondary"
                onClick={() => void onOpenPanel(openCloudClipboard)}
              >
                打开云剪贴板
              </button>
            )}
            {openCloudDictionary && (
              <button
                type="button"
                className="secondary"
                onClick={() => void onOpenPanel(openCloudDictionary)}
              >
                打开云词库
              </button>
            )}
          </>
        )}
      </ClipboardHistorySection>
      <LocalModesSection preferences={localModes} ios={ios} onChange={onLocalModesChange} />
    </fieldset>
  );
}
