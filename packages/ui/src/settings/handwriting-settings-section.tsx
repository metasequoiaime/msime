import * as surface from "../keyboard/panel-surface-style";
import * as settings from "./settings-style";

export function HandwritingSettingsSection({
  ios,
  android,
  harmony,
  macos,
  mobile,
  openSystemKeyboardSettings,
  openHandwriting,
  onOpenHandwriting,
}: {
  ios: boolean;
  android: boolean;
  harmony: boolean;
  macos: boolean;
  mobile: boolean;
  openSystemKeyboardSettings?: () => void | Promise<void>;
  openHandwriting?: () => void | Promise<void>;
  onOpenHandwriting: () => void;
}) {
  return ios ? (
    <div className={`section ${settings.launchCard}`}>
      <div className="section-title">iOS 键盘手写</div>
      <p className={settings.panelPreviewLabel}>
        请在 iOS
        系统键盘设置中启用水杉键盘，并在键盘内切换到“手写”输入方案。首次使用会按需下载中文识别模型；需要开启“允许完全访问”才能下载模型，下载后可离线识别。
      </p>
      <p className={settings.panelPreviewLabel}>
        手写由键盘扩展在当前输入框内完成，不打开独立的 Tauri
        手写面板；笔迹和识别结果不会上传，Google ML Kit 仅可能发送性能及使用统计。
      </p>
      {openSystemKeyboardSettings && (
        <button
          type="button"
          className="secondary"
          onClick={() => void openSystemKeyboardSettings()}
        >
          打开系统键盘设置
        </button>
      )}
    </div>
  ) : android ? (
    <div className={`section ${settings.launchCard}`}>
      <div className="section-title">Android 键盘手写</div>
      <p className={settings.panelPreviewLabel}>
        请在 Android 系统输入法设置中启用水杉键盘，再从键盘方案切换到“手写”。首次使用时按需下载
        Google ML Kit 中文手写模型；模型就绪后可离线识别。
      </p>
      <p className={settings.panelPreviewLabel}>
        手写识别在 Android
        键盘进程内完成，候选确认后才提交到当前编辑器；笔迹和识别结果不会上传，Google ML Kit
        仅可能发送性能及使用统计。
      </p>
      {openSystemKeyboardSettings && (
        <button
          type="button"
          className="secondary"
          onClick={() => void openSystemKeyboardSettings()}
        >
          打开系统输入法设置
        </button>
      )}
    </div>
  ) : harmony ? (
    <div className={`section ${settings.launchCard}`}>
      <div className="section-title">HarmonyOS 键盘手写</div>
      <p className={settings.panelPreviewLabel}>
        {mobile
          ? "请在系统输入法设置中启用水杉输入法，再从键盘的方案选择器切换到“手写”。"
          : "请在系统输入法设置中启用水杉输入法；2-in-1 候选窗不绘制键面，请先从悬浮工具栏打开屏幕键盘，再从方案选择器切换到“手写”。"}
      </p>
      <p className={settings.panelPreviewLabel}>
        识别由系统的 Core Vision Kit
        在设备上完成，候选确认后才提交到当前编辑器；笔迹和识别结果不离开设备。
      </p>
      {openSystemKeyboardSettings && (
        <button
          type="button"
          className="secondary"
          onClick={() => void openSystemKeyboardSettings()}
        >
          打开系统输入法设置
        </button>
      )}
    </div>
  ) : macos ? (
    <div className={`section ${settings.launchCard}`}>
      <div className="section-title">macOS 手写识别板</div>
      <p className={settings.panelPreviewLabel}>
        手写面板需要当前输入法进程提供 IMK
        输入会话；请从输入法悬浮工具栏或输入法菜单打开，识别候选会直接回到当前输入上下文。
      </p>
    </div>
  ) : (
    <div className={`section ${settings.launchCard}`}>
      <div className={`section-header ${settings.launchRow}`}>
        <span className="section-title">
          打开手写识别板
          <small>使用鼠标或触控方式手写输入，自动识别候选汉字</small>
        </span>
        <button
          type="button"
          className={`secondary ${settings.openButton}`}
          disabled={!openHandwriting}
          onClick={onOpenHandwriting}
        >
          打开
        </button>
      </div>
      <div className={settings.panelPreview} aria-label="手写识别板预览">
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
    </div>
  );
}
