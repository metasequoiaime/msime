import { useSettingsForm } from "../settings-form-context";
import { HandwritingSettingsSection } from "../handwriting-settings-section";
import { handwritingSdkPrivacyUrl } from "../handwriting-platform-notice";
import { SettingsPageFieldset } from "../settings-page-fieldset";

/** 「手写输入」的内容：只有宿主所在平台的那一组。单独成页时（触屏宿主）放在「手写输入」页里，桌面宿主上放在「屏幕键盘与手写」页里。 */
export function HandwritingSettingsContent() {
  const {
    client,
    androidPlatform,
    iosPlatform,
    harmonyPlatform,
    mobilePlatform,
    macosPlatform,
    openPanel,
    openExternalUrl,
  } = useSettingsForm();
  return (
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
  );
}

/** 设置表单的「手写输入」页：只有宿主所在平台的那一组。 */
export function HandwritingSettingsPage() {
  const { busy, page } = useSettingsForm();
  return (
    <SettingsPageFieldset disabled={busy} hidden={page !== "handwriting"} ariaLabel="手写输入">
      <HandwritingSettingsContent />
    </SettingsPageFieldset>
  );
}
