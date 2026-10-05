package app.msime.android;

import android.view.ViewGroup;
import android.widget.LinearLayout;

/**
 * 包住键区的容器（扩展点）：键行与底行经这里放进键盘的竖向布局。现在原样放入，不加任何外层；单手模式、导航栏配色以后在这里包一层。
 */
final class ImeFrame {
    private final MSIMEInputService s;
    /** onCreateInputView 里建好的键盘竖向布局，键区放进它。 */
    LinearLayout keyboard;

    ImeFrame(MSIMEInputService s) {
        this.s = s;
    }

    /** 按默认布局参数放入键区。 */
    void wrap(ViewGroup keyArea) {
        keyboard.addView(keyArea);
    }

    /** 按给定布局参数放入键区。 */
    void wrap(ViewGroup keyArea, ViewGroup.LayoutParams params) {
        keyboard.addView(keyArea, params);
    }
}
