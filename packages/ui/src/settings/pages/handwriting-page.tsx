import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { HandwritingSettingsSection } from "../handwriting-settings-section";
import { handwritingSdkPrivacyUrl } from "../handwriting-platform-notice";

/** 设置表单的「手写输入」页：只有宿主所在平台的那一组。 */
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
