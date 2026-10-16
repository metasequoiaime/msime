import type { ImeSetupClient } from "../core/host-contracts";

export interface HomePageActions {
  openKeyboard?: () => Promise<void>;
  openEmojiPanel?: () => Promise<void>;
  openClipboardPanel?: () => Promise<void>;
  openSystemKeyboardSettings?: () => Promise<void>;
  showInputMethodPicker?: () => Promise<void>;
  /** 本输入法是否已启用、是否为当前输入法，供触屏首页的状态卡片使用；没有它的宿主照常显示卡片并提供两个步骤。 */
  setup?: ImeSetupClient;
}

/** 宿主缺少键盘窗口或异步打开失败时，执行调用方提供的回退。 */
export function openHomeKeyboard(
  openKeyboard: (() => Promise<void>) | undefined,
  fallback: () => void,
) {
  if (!openKeyboard) {
    fallback();
    return;
  }
  void openKeyboard().catch(() => fallback());
}
