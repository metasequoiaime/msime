import * as settings from "../settings-style";
import { useSettingsForm } from "../settings-form-context";
import { GroupList, Row } from "../../core/platform-controls";
import { HandwritingSettingsSection } from "../handwriting-settings-section";

const handwritingSdkPrivacyUrl = "https://developers.google.com/ml-kit/terms";

/** The 手写输入 page of the settings form. */
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
  const privacyRow = client.openExternalUrl && (
    <Row title="隐私" description="Google ML Kit 的服务条款与数据说明">
      <button
        type="button"
        className="secondary"
        onClick={() => void openExternalUrl(handwritingSdkPrivacyUrl)}
      >
        手写 SDK 隐私说明
      </button>
    </Row>
  );
  return (
    <fieldset disabled={busy} hidden={page !== "handwriting"} aria-label="手写输入">
      <div className={settings.groups}>
        {iosPlatform && (
          <GroupList title="手写输入">
            <p className={settings.groupNote}>
              首次在键盘中使用手写时下载中文模型，需要完全访问权限。下载后可离线识别，笔迹和识别结果不会上传。Google
              ML Kit 会发送性能及使用统计。
            </p>
            {privacyRow}
          </GroupList>
        )}
        {harmonyPlatform && (
          <GroupList title="手写输入">
            <p className={settings.groupNote}>
              手写使用系统的文字识别能力，笔迹留在本机、不上传。设备未提供该能力时手写方案会明确提示，不会改用其他识别方式。
            </p>
          </GroupList>
        )}
        {androidPlatform && (
          <GroupList title="Android 手写输入">
            <p className={settings.groupNote}>
              首次在 Android 键盘中切换到手写时，可能需要下载 Google ML Kit
              中文手写模型。模型下载完成后可离线识别；笔迹和识别结果只用于当前输入，不会上传。Google
              ML Kit 可能发送性能及使用统计。
            </p>
            {privacyRow}
          </GroupList>
        )}
        <HandwritingSettingsSection
          ios={iosPlatform}
          android={androidPlatform}
          harmony={harmonyPlatform}
          macos={macosPlatform}
          mobile={mobilePlatform}
          openSystemKeyboardSettings={client.openSystemKeyboardSettings}
          openHandwriting={client.openHandwriting}
          onOpenHandwriting={() => void openPanel(client.openHandwriting)}
        />
      </div>
    </fieldset>
  );
}
