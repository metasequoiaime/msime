export type MobileKeyboardFeedback = {
  soundEnabled: boolean;
  hapticsEnabled: boolean;
  /** `system` 跟随系统的触感反馈设置。 */
  hapticStrength: "light" | "medium" | "strong" | "system";
  /** iOS 将这项原生英文键盘偏好保存在 App Group 中。 */
  englishSuggestions?: boolean;
  /** iOS 默认使用键盘皮肤绘制候选栏，此 App Group 开关打开时改用共享候选皮肤和颜色。 */
  candidatePaletteFollowsDesktop?: boolean;
  /** iOS 仅在此 App Group 开关开启时把组合输入作为 marked text 写进文本框；不提供原始、拼音或空白选项，因为候选栏已提供该选择。 */
  inlinePreedit?: boolean;
  /** 设备不能提供按键振动时为 false（iPad 没有 Taptic Engine）：隐藏振动控件，保留已存选择供其他设备使用。 */
  hapticsAvailable?: boolean;
  /** 仅 iPad：全宽键盘的数字行和 Tab 键，保存在 App Group 中；手机没有空间容纳它们，因此不提供该字段。 */
  tabletFullKeys?: boolean;
  /** 仅 iPad：横屏分离式键盘，存在 App Group 的 `keyboard.tablet.split`，默认关。手机上没有这个字段。 */
  tabletSplitKeyboard?: boolean;
  /** 滑行输入：在 26 键字母上连续滑动输入全拼，默认关。只有键盘实现了它的宿主才给出这个字段；HarmonyOS 存在键盘自己的 `key-feedback.json`，属于设备而不进共享偏好。 */
  glideTyping?: boolean;
  /** 仅鸿蒙，与 `glideTyping` 一样存放在键盘自己的 `key-feedback.json` 里：手写板在最后一笔后等待多久再识别，200–1500 ms，步长 100。宿主键盘没有此设置时缺省。 */
  handwritingDelayMs?: number;
  /** 仅鸿蒙：手写笔迹颜色，取皮肤的按键文字颜色或固定的黑、白、蓝。 */
  handwritingStrokeColor?: HandwritingStrokeColor;
  /** 仅鸿蒙：手写笔迹宽度，1–8。 */
  handwritingStrokeWidth?: number;
  /** 按键弹出预览：按下字母键时在其上方放大显示的字符，默认开启。只有键盘实现了它的宿主才发送此字段；鸿蒙存放在键盘自己的 `key-feedback.json` 里。 */
  keyPopup?: boolean;
  /** 滑动输入符号：在字母键上滑动输入其角落的符号，默认开启；长按则不论开关如何都输入该符号。与 `keyPopup` 同时存在。 */
  swipeSymbols?: boolean;
  /** 该滑动的方向，默认向下。 */
  swipeSymbolsDirection?: SwipeSymbolsDirection;
};

export type SwipeSymbolsDirection = "down" | "up";

export type HandwritingStrokeColor = "follow_skin" | "black" | "white" | "blue";

export type MobileKeyboardFeedbackClient = {
  load(): Promise<MobileKeyboardFeedback>;
  save(settings: MobileKeyboardFeedback): Promise<MobileKeyboardFeedback>;
  preview?(strength: MobileKeyboardFeedback["hapticStrength"]): Promise<void>;
  /** 在「音效」里选了一个包后，用它的普通键样本试听一下，`volume` 是 `plugins.key_sound.volume`（0–100）。只有能播放音效包的宿主（HarmonyOS 手机和平板）提供。 */
  previewKeySound?(pack: string, volume: number): Promise<void>;
};

/** 只投影反馈控件的显示状态，保留本设备用不了的已存选择。 */
export function mobileKeyboardFeedbackControlState(value: MobileKeyboardFeedback) {
  const hapticsVisible = value.hapticsAvailable !== false;
  return {
    hapticsVisible,
    hapticStrengthVisible: hapticsVisible && value.hapticsEnabled,
    englishSuggestionsChecked: value.englishSuggestions !== false,
  };
}
