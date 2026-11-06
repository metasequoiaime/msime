package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import android.view.MotionEvent;
import android.widget.Button;

/**
 * 十四／十八键的分词键上滑放下网格辅助码的分隔符（{@link GridHelpcodePolicy}）。
 *
 * <p>这次组字不能用网格辅助码（`grid_helpcode_available` 为假）或还没滑过阈值时，整次触摸照常交给按钮：点按仍是分词或打开符号面板。滑过阈值的那一刻给按钮补一个 CANCEL，它就不再点按；松手时放下分隔符，引擎收下了才与按钮的点按一样算一次按键、给一次按键反馈。
 */
final class ImeGridHelpcodeEntry {
    private ImeGridHelpcodeEntry() {}

    static void bind(MSIMEInputService s, Button key) {
        final float[] down = new float[2];
        final boolean[] entering = {false};
        key.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    down[0] = DimensionPolicy.fromPixels(s, event.getX());
                    down[1] = DimensionPolicy.fromPixels(s, event.getY());
                    entering[0] = false;
                    return false;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (entering[0]) return true;
                    if (!available(s) || !GridHelpcodePolicy.entrySwipe(
                            DimensionPolicy.fromPixels(s, event.getX()) - down[0],
                            DimensionPolicy.fromPixels(s, event.getY()) - down[1])) return false;
                    entering[0] = true;
                    MotionEvent cancel = MotionEvent.obtain(event);
                    cancel.setAction(MotionEvent.ACTION_CANCEL);
                    key.onTouchEvent(cancel);
                    cancel.recycle();
                    key.setPressed(false);
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    if (!entering[0]) return false;
                    entering[0] = false;
                    // 上滑和松手之间组字可能已经变了（光标移进组码、被别的更新上屏）；引擎不收时不给反馈、不计键。
                    if (s.gridHelpcode(GridHelpcodePolicy.BEGIN, (char) 0)) {
                        s.imeKeyFeedback.playFeedback(key);
                        s.countKey(key);
                    }
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    boolean consumed = entering[0];
                    entering[0] = false;
                    return consumed;
                }
                default -> {
                    return entering[0];
                }
            }
        });
    }

    /** 视图说这次组字能用网格辅助码。 */
    static boolean available(MSIMEInputService s) {
        return s.view != null
            && InputViewValuePolicy.booleanValue(s.view, "grid_helpcode_available", false);
    }
}
