package app.msime.android;

import android.content.Context;
import android.view.MotionEvent;
import android.widget.FrameLayout;
import java.util.function.BooleanSupplier;

/**
 * 输入法窗口的根容器：退格交给编辑器、还没确认时，按住新开始的触摸（按键、候选、工具栏都在它下面），确认后按原顺序分发。规则见 {@link TouchHoldQueue}，何时按住见 {@link BackspaceSettle}。
 *
 * <p>没装 `hold` 或不在按住时和普通 `FrameLayout` 完全一样。
 */
public final class TouchHoldLayout extends FrameLayout {
    private final TouchHoldQueue<MotionEvent> queue = new TouchHoldQueue<>(new TouchHoldQueue.Events<>() {
        @Override public int action(MotionEvent event) { return event.getActionMasked(); }
        @Override public MotionEvent copy(MotionEvent event) { return MotionEvent.obtain(event); }
    });
    private BooleanSupplier hold;

    public TouchHoldLayout(Context context) {
        super(context);
    }

    /** `hold` 返回 true 时按住新开始的触摸。 */
    public void setHold(BooleanSupplier hold) {
        this.hold = hold;
    }

    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        if (queue.overflowing()) release();
        if (hold != null && queue.offer(event, hold.getAsBoolean())) return true;
        return super.dispatchTouchEvent(event);
    }

    /**
     * 放行排着的触摸。放出的某次触摸又让键盘进入按住（例如放出的是退格），就停在下一次触摸的按下前面，等下一次放行。
     */
    public void release() {
        while (!queue.isEmpty()) {
            if (hold != null && hold.getAsBoolean() && queue.nextStartsGesture() && !queue.overflowing()) return;
            MotionEvent event = queue.poll();
            super.dispatchTouchEvent(event);
            event.recycle();
        }
    }
}
