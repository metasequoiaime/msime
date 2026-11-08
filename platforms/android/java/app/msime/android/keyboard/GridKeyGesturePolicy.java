package app.msime.android;

/**
 * 14 键与 18 键字母键一次按压的手势判定（#11063），规则跟 26 键走：
 *
 * <ul>
 *   <li>沿「滑动输入符号」的方向（缺省上滑）纵移严格超过 {@link SwipeHintPolicy#THRESHOLD_DP}，且纵向位移大于横向，松手输入这个键的角标符号（{@link GridLetterHintTable}）；
 *   <li>反方向同样的纵移，或空闲时长按，打开这个键的字母和符号选项，并可继续拖选；
 *   <li>18 键双拼的两字母键另有横滑限定字母，阈值和方向照旧（{@link EighteenKeySwipePolicy}）；
 *   <li>「滑动输入符号」关闭时两种纵滑都不响应，长按照旧；
 *   <li>组字中（Engine 的 `editing_text` 不空）纵滑和长按都按一次普通点按处理，免得组字时误出符号或先结束组字；横滑限定字母照常可用。上屏后联想不算组字。
 * </ul>
 *
 * <p>先触发的那一种锁定这次按压：纵滑一旦触发，回到起点也仍输入符号；横滑可以反向重选，回到起点不输入。多指、换指和系统取消让这次按压什么都不输入。纵滑与横滑的条件互斥（纵滑要求 |dy| &gt; |dx|，横滑要求 |dx| &gt; 1.5|dy|），同一次移动不会两者都成立。
 */
public final class GridKeyGesturePolicy {
    /** 这次按压此刻处在哪一种手势里。 */
    public enum Mode {
        /** 还没触发任何手势，松手是普通点按。 */
        PENDING,
        /** 纵滑已触发，松手输入角标符号。 */
        SYMBOL,
        /** 横滑已接管，松手输入 {@link Gesture#selected} 限定的字母（为 0 时不输入）。 */
        LETTER,
        /** 字母和符号选项已打开，后续移动交给选项拖选。 */
        OPTIONS,
        /** 多指、换指或系统取消：什么都不输入。 */
        CANCELLED
    }

    private GridKeyGesturePolicy() {}

    /**
     * 是否已沿 `direction` 构成纵滑。
     *
     * @param direction {@link SwipeHintPolicy#UP} 或 {@link SwipeHintPolicy#DOWN}；null 表示「滑动输入符号」关闭
     * @param dx 相对按下点的横向位移（dp）
     * @param dy 相对按下点的纵向位移（dp，向下为正）
     */
    public static boolean vertical(String direction, float dx, float dy) {
        if (direction == null || !Float.isFinite(dx) || !Float.isFinite(dy)) return false;
        return SwipeHintPolicy.swiped(direction, 0f, dy) && Math.abs(dy) > Math.abs(dx);
    }

    /** 与 `direction` 相反的方向；null 仍是 null。 */
    public static String opposite(String direction) {
        if (direction == null) return null;
        return SwipeHintPolicy.UP.equals(direction) ? SwipeHintPolicy.DOWN : SwipeHintPolicy.UP;
    }

    /** 一个键上一次按压的状态；每次按下用 {@link #start} 复位。 */
    public static final class Gesture {
        private final String exactLetters;
        private final boolean hasSymbol;
        private String direction;
        private boolean tapsOnly;
        private int pointer;
        private float downX;
        private float downY;
        private Mode mode = Mode.PENDING;
        private char selected;

        /**
         * @param exactLetters 横滑可限定的两个小写字母（18 键双拼的两字母键）；null 表示这个键没有横滑
         * @param hasSymbol 这个键有角标符号
         */
        public Gesture(String exactLetters, boolean hasSymbol) {
            this.exactLetters = exactLetters;
            this.hasSymbol = hasSymbol;
        }

        /**
         * 按下：记下起点和这次按压的设置。
         *
         * @param symbolDirection 「滑动输入符号」的方向；关闭时传 null
         * @param composing 按下这一刻正在组字
         */
        public void start(int pointerId, float x, float y, String symbolDirection, boolean composing) {
            pointer = pointerId;
            downX = x;
            downY = y;
            direction = symbolDirection;
            tapsOnly = composing;
            mode = Mode.PENDING;
            selected = 0;
        }

        /** 按压中途开始组字（另一根手指打了字）：还没触发手势时，之后的纵滑和长按都按点按处理。 */
        public void composing() {
            if (mode == Mode.PENDING) tapsOnly = true;
        }

        /**
         * 手指移动。
         *
         * @param pointers 此刻按着的手指数
         * @return 这次移动是否改变了 {@link #mode} 或 {@link #selected}
         */
        public boolean move(int pointerId, int pointers, float x, float y) {
            Mode before = mode;
            char beforeSelected = selected;
            if (mode == Mode.CANCELLED || mode == Mode.OPTIONS || mode == Mode.SYMBOL) {
                // 选项打开后，换指和多指由选项自己的拖选与取消处理。
                if (mode == Mode.SYMBOL && invalid(pointerId, pointers, x, y)) cancel();
                return mode != before;
            }
            if (invalid(pointerId, pointers, x, y)) {
                cancel();
                return true;
            }
            float dx = x - downX;
            float dy = y - downY;
            if (mode == Mode.LETTER) {
                selected = EighteenKeySwipePolicy.letter(exactLetters, dx, dy);
                return selected != beforeSelected;
            }
            char letter = exactLetters == null ? 0 : EighteenKeySwipePolicy.letter(exactLetters, dx, dy);
            if (letter != 0) {
                mode = Mode.LETTER;
                selected = letter;
            } else if (!tapsOnly && hasSymbol && vertical(direction, dx, dy)) {
                mode = Mode.SYMBOL;
            } else if (!tapsOnly && vertical(opposite(direction), dx, dy)) {
                mode = Mode.OPTIONS;
            }
            return mode != before;
        }

        /** 长按计时到：空闲且还没触发手势时打开选项，返回 true；组字中或已触发其他手势时不做事。 */
        public boolean hold() {
            if (mode != Mode.PENDING || tapsOnly) return false;
            mode = Mode.OPTIONS;
            return true;
        }

        /** 系统取消、多指或换指。 */
        public void cancel() {
            mode = Mode.CANCELLED;
            selected = 0;
        }

        public Mode mode() { return mode; }

        /** {@link Mode#LETTER} 时选中的精确字母（大写标记，见 {@link EighteenKeySwipePolicy}）；否则为 0。 */
        public char selected() { return selected; }

        /** 手势接管了这次按压，按钮自己的点击要取消。 */
        public boolean consumed() { return mode != Mode.PENDING; }

        /**
         * 松手时是否要由手势补一次普通点按：组字中没有触发手势的按压一律算一次点按，即使位移让按钮自己取消了点击。空闲时交给按钮，返回 false。
         */
        public boolean forcesTap() { return mode == Mode.PENDING && tapsOnly; }

        private boolean invalid(int pointerId, int pointers, float x, float y) {
            return pointers != 1 || pointerId != pointer || !Float.isFinite(x) || !Float.isFinite(y);
        }
    }
}
