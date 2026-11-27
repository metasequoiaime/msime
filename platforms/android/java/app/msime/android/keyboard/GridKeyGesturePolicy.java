package app.msime.android;

/**
 * 14 键、18 键与 26 键字母键一次按压的手势判定（#11063；26 键从 #11822 起也用这一套，判定与 14/18 键相同）：
 *
 * <ul>
 *   <li>沿「滑动输入符号」的方向（缺省上滑）纵移严格超过「滑动距离」（与 26 键共用，{@link SwipeHintPolicy#MIN_THRESHOLD_DP}..{@link SwipeHintPolicy#MAX_THRESHOLD_DP} dp，默认 {@link SwipeHintPolicy#DEFAULT_THRESHOLD_DP}），且纵向位移大于横向，松手输入这个键的角标符号（{@link GridLetterHintTable}）；
 *   <li>反方向同样的纵移，或空闲时长按，打开这个键的字母和符号选项，并可继续拖选；26 键只有中文的反方向打开选项，英文和按大小写写字的方案（越南语、藏文）反方向只是一次点按（{@link Gesture#Gesture(String, boolean, boolean)}），后者的长按直接输入角标（{@link Gesture#holdSymbol}）；
 *   <li>18 键双拼的两字母键另有横滑限定字母，阈值和方向照旧（{@link EighteenKeySwipePolicy}）；
 *   <li>「滑动输入符号」关闭时两种纵滑都不响应，长按照旧；
 *   <li>纵移超过 {@link SwipeHintPolicy#THRESHOLD_DP}（「滑动距离」的出厂值，固定）却没触发手势时——关掉了「滑动输入符号」、调大了「滑动距离」而没滑到，或 26 键反方向不开选项——松手仍是这个键的一次点按，即使手指已经移出键帽、按钮自己取消了点击；
 *   <li>组字中（14/18 键看 Engine 的 `editing_text` 不空，26 键看候选栏，见 {@link SwipeHintPolicy#tapsOnly}；由调用方经 {@link Gesture#start} 和 {@link Gesture#composing} 传入）纵滑和长按都按一次普通点按处理，免得组字时误出符号或先结束组字；横滑限定字母照常可用。上屏后联想不算组字。
 * </ul>
 *
 * <p>先触发的那一种接管这次按压。纵滑出符号后可以滑回取消（#11822，规则见 {@link SymbolSwipe}）：沿设定方向的位移退回「滑动距离」的一半以内就是 {@link Mode#DISARMED}，松手什么都不输入，再滑过「滑动距离」又回到 {@link Mode#SYMBOL}；之后不会再变回点按、选项或横滑。长按直接出的符号（{@link Gesture#holdSymbol}）不能滑回取消。横滑可以反向重选，回到起点不输入。多指、换指和系统取消让这次按压什么都不输入（26 键的调用方只报按下的那根手指，不因多指取消）。纵滑与横滑的条件互斥（纵滑要求 |dy| &gt; |dx|，横滑要求 |dx| &gt; 1.5|dy|），同一次移动不会两者都成立。
 */
public final class GridKeyGesturePolicy {
    /** 这次按压此刻处在哪一种手势里。 */
    public enum Mode {
        /** 还没触发任何手势，松手是普通点按。 */
        PENDING,
        /** 纵滑已触发，松手输入角标符号。 */
        SYMBOL,
        /** 纵滑出过符号、又滑回「滑动距离」一半以内：松手什么都不输入；再滑过「滑动距离」回到 {@link #SYMBOL}。 */
        DISARMED,
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
     * @param thresholdDp 「滑动距离」（dp），严格大于才算
     * @param dx 相对按下点的横向位移（dp）
     * @param dy 相对按下点的纵向位移（dp，向下为正）
     */
    public static boolean vertical(String direction, float thresholdDp, float dx, float dy) {
        return SymbolSwipe.arms(direction, thresholdDp, dx, dy, true);
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
        private final boolean oppositeOptions;
        private String direction;
        private float thresholdDp = SwipeHintPolicy.THRESHOLD_DP;
        private boolean tapsOnly;
        private boolean verticalTap;
        private int pointer;
        private float downX;
        private float downY;
        private Mode mode = Mode.PENDING;
        private boolean heldSymbol;
        private char selected;

        /**
         * @param exactLetters 横滑可限定的两个小写字母（14/18 键全拼和双拼的两字母键，见 {@link EighteenKeySwipePolicy#exactLetters}）；null 表示这个键没有横滑
         * @param hasSymbol 这个键有角标符号
         */
        public Gesture(String exactLetters, boolean hasSymbol) {
            this(exactLetters, hasSymbol, true);
        }

        /**
         * @param exactLetters 同 {@link #Gesture(String, boolean)}
         * @param hasSymbol 同 {@link #Gesture(String, boolean)}
         * @param oppositeOptions 反方向纵滑打开选项（14/18 键、26 键中文）；为 false 时反方向的纵移只是一次点按（26 键英文和按大小写写字的方案）
         */
        public Gesture(String exactLetters, boolean hasSymbol, boolean oppositeOptions) {
            this.exactLetters = exactLetters;
            this.hasSymbol = hasSymbol;
            this.oppositeOptions = oppositeOptions;
        }

        /**
         * 按下：记下起点和这次按压的设置。
         *
         * @param symbolDirection 「滑动输入符号」的方向；关闭时传 null
         * @param symbolThresholdDp 「滑动距离」（dp），两个纵向方向共用
         * @param composing 按下这一刻正在组字
         */
        public void start(int pointerId, float x, float y, String symbolDirection, float symbolThresholdDp,
                boolean composing) {
            pointer = pointerId;
            downX = x;
            downY = y;
            direction = symbolDirection;
            thresholdDp = symbolThresholdDp;
            tapsOnly = composing;
            verticalTap = false;
            mode = Mode.PENDING;
            heldSymbol = false;
            selected = 0;
        }

        /** 按压中途开始组字（另一根手指打了字）：还没触发手势时，之后的纵滑和长按都按点按处理。 */
        public void composing() {
            if (mode == Mode.PENDING) tapsOnly = true;
        }

        /**
         * 手指移动。只喂 `ACTION_MOVE`（#11822）：抬起那一点不喂，抬指时触点常会一跳，单凭它不能触发手势。
         *
         * @param pointers 此刻按着的手指数
         * @return 这次移动是否改变了 {@link #mode} 或 {@link #selected}
         */
        public boolean move(int pointerId, int pointers, float x, float y) {
            Mode before = mode;
            char beforeSelected = selected;
            if (mode == Mode.CANCELLED || mode == Mode.OPTIONS) return false;
            if (mode == Mode.SYMBOL || mode == Mode.DISARMED) {
                // 选项打开后，换指和多指由选项自己的拖选与取消处理；符号这里自己取消。
                if (invalid(pointerId, pointers, x, y)) cancel();
                else if (!heldSymbol && mode == Mode.SYMBOL && SymbolSwipe.disarms(direction, thresholdDp, y - downY))
                    mode = Mode.DISARMED;
                else if (mode == Mode.DISARMED && vertical(direction, thresholdDp, x - downX, y - downY))
                    mode = Mode.SYMBOL;
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
            } else if (!tapsOnly && hasSymbol && vertical(direction, thresholdDp, dx, dy)) {
                mode = Mode.SYMBOL;
            } else if (!tapsOnly && oppositeOptions && vertical(opposite(direction), thresholdDp, dx, dy)) {
                mode = Mode.OPTIONS;
            } else if (vertical(SwipeHintPolicy.UP, SwipeHintPolicy.THRESHOLD_DP, dx, dy)
                    || vertical(SwipeHintPolicy.DOWN, SwipeHintPolicy.THRESHOLD_DP, dx, dy)) {
                // 用固定的 14 dp 而不是「滑动距离」：关掉滑动时与加这项设置之前相同；距离调大后没滑到的纵移也算点按，不会因为移出键帽而丢键。
                verticalTap = true;
            }
            return mode != before;
        }

        /** 长按计时到：空闲且还没触发手势时打开选项，返回 true；组字中或已触发其他手势时不做事。 */
        public boolean hold() {
            if (mode != Mode.PENDING || tapsOnly) return false;
            mode = Mode.OPTIONS;
            return true;
        }

        /** 长按计时到、而这个键的长按直接输入角标（26 键按大小写写字的方案）：空闲且还没触发手势时锁定为符号，返回 true；组字中、已触发其他手势或没有角标时不做事。 */
        public boolean holdSymbol() {
            if (mode != Mode.PENDING || tapsOnly || !hasSymbol) return false;
            mode = Mode.SYMBOL;
            heldSymbol = true;
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
         * 松手时是否要由手势补一次普通点按：组字中的按压，以及纵移超过 14 dp 却没触发手势（关闭符号滑动，或没滑到「滑动距离」）的按压，即使位移让按钮取消点击，也仍按一次点按处理。其他空闲按压交给按钮。
         */
        public boolean forcesTap() { return mode == Mode.PENDING && (tapsOnly || verticalTap); }

        private boolean invalid(int pointerId, int pointers, float x, float y) {
            return pointers != 1 || pointerId != pointer || !Float.isFinite(x) || !Float.isFinite(y);
        }
    }
}
