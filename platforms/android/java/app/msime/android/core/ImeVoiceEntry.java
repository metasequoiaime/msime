package app.msime.android;

import android.view.ViewGroup;

/**
 * 键盘内语音识别的入口（扩展点）。startVoiceRecognition 先问这里：返回 true 表示已在键区里接手，不再打开识别窗口；现在总是返回 false，走原来的 VoiceRecognitionActivity 流程。
 */
final class ImeVoiceEntry {
    private final MSIMEInputService s;

    ImeVoiceEntry(MSIMEInputService s) {
        this.s = s;
    }

    /** 在键区里开始识别；现在不接手，交回原流程。 */
    boolean startInKeyboard(ViewGroup keyArea) {
        return false;
    }
}
