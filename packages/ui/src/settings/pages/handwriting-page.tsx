import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { HandwritingSettingsSection } from "../handwriting-settings-section";
import { handwritingSdkPrivacyUrl } from "../handwriting-platform-notice";

/** The 手写输入 page of the settings form: one group for the platform the host runs on. */
export function HandwritingSettingsPage() {
  const {
    client,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    macosPlatform,
    busy,
    page,
    openPanel,
    openExternalUrl,
  } = useSettingsForm();
  return (
    <fieldset disabled={busy} hidden={page !== "handwriting"} aria-label="手写输入">
      <div className={settings.groups}>
        <HandwritingSettingsSection
          ios={iosPlatform}
          android={androidPlatform}
          harmony={harmonyPlatform}
          macos={macosPlatform}
          mobile={mobilePlatform}
          openSystemKeyboardSettings={client.openSystemKeyboardSettings}
          openHandwriting={client.openHandwriting}
          onOpenHandwriting={() => void openPanel(client.openHandwriting)}
          onOpenSdkPrivacy={
            client.openExternalUrl
              ? () => void openExternalUrl(handwritingSdkPrivacyUrl)
              : undefined
          }
        />
      </div>
    </fieldset>
  );
}
