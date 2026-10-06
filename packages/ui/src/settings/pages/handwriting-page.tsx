import { useSettingsForm } from "../settings-form-context";
import { HandwritingSettingsSection } from "../handwriting-settings-section";
import { handwritingSdkPrivacyUrl } from "../handwriting-platform-notice";
import { SettingsPageFieldset } from "../settings-page-fieldset";

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
    <SettingsPageFieldset disabled={busy} hidden={page !== "handwriting"} ariaLabel="手写输入">
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
          client.openExternalUrl ? () => void openExternalUrl(handwritingSdkPrivacyUrl) : undefined
        }
      />
    </SettingsPageFieldset>
  );
}
