package app.msime.android;

/** 十八键双拼横滑限定一个字母；大写是共享网格的精确标记，不是直接上屏。 */
public final class EighteenKeySwipePolicy {
    public static final float THRESHOLD_DP = 24;

    private EighteenKeySwipePolicy() {}

    /** 左滑取左字母，右滑取右字母；纵移、抖动和无效键面返回 0。 */
    public static char letter(String letters, float dx, float dy) {
        if (letters == null || letters.length() != 2
                || letters.charAt(0) < 'a' || letters.charAt(0) > 'z'
                || letters.charAt(1) < 'a' || letters.charAt(1) > 'z'
                || !Float.isFinite(dx) || !Float.isFinite(dy)
                || Math.abs(dx) <= THRESHOLD_DP || Math.abs(dx) <= 1.5f * Math.abs(dy)) return 0;
        return Character.toUpperCase(letters.charAt(dx < 0 ? 0 : 1));
    }

    /** 一次按压的选择状态；接管后回中心、多指或取消都不能补一次普通点击。 */
    public static final class Gesture {
        private final String letters;
        private int pointer;
        private float downX;
        private float downY;
        private boolean consumed;
        private boolean cancelled;
        private boolean held;
        private char selected;

        public Gesture(String letters) { this.letters = letters; }

        public void start(int pointerId, float x, float y) {
            pointer = pointerId;
            downX = x;
            downY = y;
            consumed = cancelled = held = false;
            selected = 0;
        }

        public void move(int pointerId, int pointers, float x, float y) {
            if (cancelled) return;
            if (pointers != 1 || pointerId != pointer || !Float.isFinite(x) || !Float.isFinite(y)) {
                cancel();
                return;
            }
            if (held) return;
            selected = letter(letters, x - downX, y - downY);
            consumed |= selected != 0;
        }

        public void hold() { held = true; selected = 0; }
        public void cancel() { cancelled = consumed = true; selected = 0; }
        public boolean consumed() { return consumed; }
        public char selected() { return selected; }
    }
}
