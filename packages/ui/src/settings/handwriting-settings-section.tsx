import * as surface from "../keyboard/panel-surface-style";
import { GroupList, Row } from "../core/platform-controls";
import { handwritingPrivacyText } from "./handwriting-platform-notice";
import * as settings from "./settings-style";
import { OpenPanelButton } from "./open-panel-button";

/** 「手写输入」页每个平台一组：在键盘类宿主上是开启手写的方法、系统设置按钮、隐私说明和 SDK 的隐私行；在会打开自己面板的桌面宿主上是启动按钮和预览。 */
export function HandwritingSettingsSection({
  ios,
  android,
  harmony,
  macos,
  mobile,
  openSystemKeyboardSettings,
  openHandwriting,
  onOpenHandwriting,
  onOpenSdkPrivacy,
}: {
  ios: boolean;
  android: boolean;
  harmony: boolean;
  macos: boolean;
  mobile: boolean;
  openSystemKeyboardSettings?: () => void | Promise<void>;
  openHandwriting?: () => void | Promise<void>;
  onOpenHandwriting: () => void;
  /** 打开 Google ML Kit 的隐私条款；宿主无法打开外部链接时不传，这一行随之隐藏。鸿蒙用系统能力识别，从不显示它。 */
  onOpenSdkPrivacy?: () => void;
}) {
  const systemSettingsRow = (title: string, label: string) =>
    openSystemKeyboardSettings && (
      <Row title={title}>
        <button
          type="button"
          className="secondary"
          onClick={() => void openSystemKeyboardSettings()}
        >
          {label}
        </button>
      </Row>
    );
  const sdkPrivacyRow = onOpenSdkPrivacy && (
    <Row title="隐私" description="Google ML Kit 的服务条款与数据说明">
      <button type="button" className="secondary" onClick={onOpenSdkPrivacy}>
        手写 SDK 隐私说明
      </button>
    </Row>
  );
  return ios ? (
    <GroupList title="iOS 键盘手写">
      <p className={settings.groupNote}>
        请在 iOS
        系统键盘设置中启用水杉键盘，并在键盘内切换到“手写”输入方案。首次使用会按需下载中文识别模型；需要开启“允许完全访问”才能下载模型，下载后可离线识别。
      </p>
      {systemSettingsRow("系统键盘设置", "打开系统键盘设置")}
      <p className={settings.groupNote}>{handwritingPrivacyText("ios")}</p>
      {sdkPrivacyRow}
    </GroupList>
  ) : android ? (
    <GroupList title="Android 键盘手写">
      <p className={settings.groupNote}>
        请在 Android 系统输入法设置中启用水杉键盘，再从键盘方案切换到“手写”。首次使用时按需下载
        Google ML Kit 中文手写模型；模型就绪后可离线识别。
      </p>
      {systemSettingsRow("系统输入法设置", "打开系统输入法设置")}
      <p className={settings.groupNote}>{handwritingPrivacyText("android")}</p>
      {sdkPrivacyRow}
    </GroupList>
  ) : harmony ? (
    <GroupList title="HarmonyOS 键盘手写">
      <p className={settings.groupNote}>
        {mobile
          ? "请在系统输入法设置中启用水杉输入法，再从键盘的方案选择器切换到“手写”。"
          : "请在系统输入法设置中启用水杉输入法；2-in-1 候选窗口不绘制键面，请先从悬浮工具栏打开屏幕键盘，再从方案选择器切换到“手写”。"}
      </p>
      {systemSettingsRow("系统输入法设置", "打开系统输入法设置")}
      <p className={settings.groupNote}>{handwritingPrivacyText("harmony")}</p>
    </GroupList>
  ) : macos ? (
    <GroupList title="macOS 手写识别板">
      <p className={settings.groupNote}>
        手写面板需要当前输入法进程提供 IMK
        输入会话；请从输入法悬浮工具栏或输入法菜单打开，识别候选会直接回到当前输入上下文。
      </p>
    </GroupList>
  ) : (
    <GroupList title="手写识别板">
      <Row title="打开手写识别板" description="使用鼠标或触控方式手写输入，自动识别候选汉字">
        <OpenPanelButton
          action={onOpenHandwriting}
          className={`secondary ${settings.openButton}`}
        />
      </Row>
      <div className={settings.groupPreview} aria-label="手写识别板预览">
        <div className={settings.panelPreviewLabel}>预览</div>
        <div className={surface.mock}>
          <div className={surface.mockCanvas}>
            <span className={surface.mockStroke}>水</span>
          </div>
          <div className={surface.mockCandidates}>
            <span>水</span>
            <span>永</span>
            <span>木</span>
            <span>未</span>
          </div>
        </div>
      </div>
    </GroupList>
  );
}
