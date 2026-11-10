package app.msime.android;

import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.view.Gravity;
import android.view.MotionEvent;
import android.widget.FrameLayout;
import android.widget.TextView;

/** 空格复制手势的浮层提示：不参与键盘测量，也不拦截键盘触摸。 */
final class SpaceGestureHintOverlay extends TextView {
    private final MSIMEInputService service;

    SpaceGestureHintOverlay(MSIMEInputService service) {
        super(service);
        this.service = service;
        setSingleLine(true);
        setGravity(Gravity.CENTER);
        setContentDescription("空格手势提示");
        setAccessibilityLiveRegion(ACCESSIBILITY_LIVE_REGION_POLITE);
        ViewPolicy.setTextSizeSp(this, 12);
        ViewPolicy.setTypefaceWeight(this, Typeface.BOLD);
        ViewPolicy.setNonInteractive(this);
        ViewPolicy.hide(this);
    }

    void showLabel(String label) {
        setText(label);
        setContentDescription(label);
        ViewPolicy.setTextColor(this, Color.WHITE);
        ViewPolicy.setBackground(this, DrawablePolicy.pill(Color.BLACK));
        int horizontal = service.pixels(10);
        int vertical = service.pixels(4);
        ViewPolicy.setPadding(this, horizontal, vertical, horizontal, vertical);
        if (getParent() instanceof FrameLayout parent) parent.bringChildToFront(this);
        ViewPolicy.show(this);
    }

    void hideLabel() {
        ViewPolicy.hide(this);
        setText("");
        setContentDescription("空格手势提示");
    }

    @Override public boolean onTouchEvent(MotionEvent event) { return false; }
}
