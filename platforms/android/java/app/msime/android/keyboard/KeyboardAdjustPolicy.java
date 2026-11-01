package app.msime.android;

/**
 * 「键盘调整」遮罩的纯逻辑：哪些柄显示、每个柄的拖动怎样换算成键盘高度、底部抬高、宽度和水平位置，读屏的步长，以及宽度和位置在可用宽度里落到哪几个像素。不依赖 Android 运行时，由 KeyboardAdjustPolicySmoke 覆盖。
 *
 * <p>四个柄各自只动自己那条边，对边在屏幕上不动：上沿柄只改高度；下沿柄改底部抬高，高度反向变化同样多，上沿不动，往上拖到高度下限后整块继续上移、上沿跟着上去；左、右沿柄改宽度，同时改水平位置让另一侧边不动。「移动」整块平移，大小不变：竖直方向只改抬高，水平方向只改位置。
 *
 * <p>竖直方向的单位都是 dp：高度是设计范围的调整量（{@link KeyboardHeightPolicy}，-46..129），抬高是键区下沿到底栏或导航区的额外距离（{@link KeyboardBottomBarPolicy#extraPaddingDp}，0 到 160 减去底栏占用的 40）。两者在键盘总高里一比一相加，所以下沿柄把一份挪给另一份时上沿不动；高度让到下限以后只剩抬高在加，上沿才开始跟着往上走。高度只在窗口封顶（{@link KeyboardHeightPolicy#windowHeightAdjustment}）以内拖动：封顶以上的那段画出来一样高，拖进去手指和边就对不上了。水平方向用像素：宽度存成可用宽度的百分比，位置存成剩余空间（可用宽度减键盘宽度）里的千分比。
 */
public final class KeyboardAdjustPolicy {
    /** 宽度的百分比范围与缺省值（本地设置 `platform.android.keyboard_width_percent`）。 */
    public static final int MIN_WIDTH_PERCENT = 60;
    public static final int MAX_WIDTH_PERCENT = 100;
    public static final int DEFAULT_WIDTH_PERCENT = 100;
    /** 键盘最窄多少 dp：与 {@link #MIN_WIDTH_PERCENT} 取更宽的那个，窄屏上 60% 太窄、按不准。 */
    public static final int MIN_WIDTH_DP = 240;
    /** 水平位置：剩余空间里的千分比（本地设置 `platform.android.keyboard_position_x`），0 贴左、1000 贴右，缺省居中。 */
    public static final int MAX_POSITION = 1000;
    public static final int DEFAULT_POSITION = 500;
    /** 读屏每次增减的步长：高度 5%、抬高 4 dp（与设置页「键盘底部高度」滑块一格相同）、宽度 5%、位置 50‰。 */
    public static final int HEIGHT_STEP_PERCENT = 5;
    public static final int LIFT_STEP_DP = KeyboardBottomBarPolicy.PADDING_STEP_DP;
    public static final int WIDTH_STEP_PERCENT = 5;
    public static final int POSITION_STEP = 50;

    /** 遮罩上可以拖动或调节的部件：四条边的柄和中间的「移动」。 */
    public enum Part { TOP, BOTTOM, LEFT, RIGHT, MOVE }

    private KeyboardAdjustPolicy() { }

    /**
     * 此刻哪几项生效，也就是哪些柄显示。
     *
     * @param lift 底部抬高生效：浮动键盘、实体键盘收起键区、手机横屏时不生效，与 {@link KeyboardBottomBarPolicy#extraPaddingDp} 的条件相同
     * @param width 宽度和水平位置生效：抬高不生效时同样不生效，单手模式和分离式键盘画着时也不生效
     */
    public record Availability(boolean lift, boolean width) {
        /**
         * @param floating 键盘悬在应用上面（浮动键盘或跟随光标的候选栏）
         * @param keysCollapsed 实体键盘的候选条模式收起了键区
         * @param phoneLandscape 手机横屏
         * @param oneHanded 单手模式此刻生效
         * @param split 分离式键盘此刻画着
         */
        public static Availability of(boolean floating, boolean keysCollapsed, boolean phoneLandscape,
                boolean oneHanded, boolean split) {
            boolean lift = !floating && !keysCollapsed && !phoneLandscape;
            return new Availability(lift, lift && !oneHanded && !split);
        }

        /** 这个部件显示不显示：上沿柄总在；下沿柄和「移动」跟着抬高；左右柄跟着宽度。 */
        public boolean shows(Part part) {
            return switch (part) {
                case TOP -> true;
                case BOTTOM, MOVE -> lift;
                case LEFT, RIGHT -> width;
            };
        }

        /** 「移动」能不能左右移：宽度和位置不生效时只改抬高。 */
        public boolean movesHorizontally() {
            return width;
        }
    }

    /** 竖直方向的一组值：设计范围的高度调整量与底部抬高，都是 dp。 */
    public record Vertical(int heightAdjustment, int liftDp) { }

    /** 水平方向的一组值：宽度百分比与水平位置千分比，即存进本地设置的两项。 */
    public record Horizontal(int widthPercent, int position) {
        public static final Horizontal DEFAULT = new Horizontal(DEFAULT_WIDTH_PERCENT, DEFAULT_POSITION);
    }

    /** 读屏调节的范围：当前值，以及此刻还能不能增、能不能减（下沿柄的抬高还要看高度有没有到下限）。 */
    public record Range(int min, int max, int current, boolean canIncrease, boolean canDecrease) {
        static Range of(int min, int max, int current) {
            return new Range(min, max, current, current < max, current > min);
        }
    }

    // ---- 竖直方向 ----

    /** 抬高的上限：底栏画着时它的 40 dp 计在 160 dp 的目标总距离里。 */
    public static int maxLiftDp(boolean barShown) {
        return KeyboardBottomBarPolicy.MAX_PADDING_DP - (barShown ? KeyboardBottomBarPolicy.BAR_HEIGHT_DP : 0);
    }

    /** 存着的底部高度此刻抬起多少（开关与高度两项，见 {@link KeyboardBottomBarPolicy#extraPaddingDp}）。 */
    public static int liftDp(boolean enabled, int heightDp, boolean barShown) {
        return KeyboardBottomBarPolicy.extraPaddingDp(enabled, heightDp, barShown, false, false, false);
    }

    /** 抬高 {@code liftDp} 对应要存的目标总距离：底栏画着时加上底栏的 40 dp；0 表示关掉开关。 */
    public static int storedBottomHeightDp(int liftDp, boolean barShown) {
        int lift = BoundsPolicy.bounded(liftDp, 0, maxLiftDp(barShown));
        return lift == 0 ? 0 : lift + (barShown ? KeyboardBottomBarPolicy.BAR_HEIGHT_DP : 0);
    }

    /** 这个窗口高度里高度调整量能拖到的上限：设计上限按窗口封顶之后的值。 */
    public static int maxHeightAdjustment(int windowHeightDp) {
        return KeyboardHeightPolicy.windowHeightAdjustment(
            KeyboardHeightPolicy.MAX_DESIGN_HEIGHT_ADJUSTMENT_DP, windowHeightDp);
    }

    /** 拖动从画出来的高度起算：存着的值高过窗口封顶时按封顶后的值，免得往下拖一段才开始变矮。 */
    public static int effectiveHeight(int heightAdjustment, int windowHeightDp) {
        return KeyboardHeightPolicy.windowHeightAdjustment(heightAdjustment, windowHeightDp);
    }

    /** 上沿柄：手指从按下处竖直移动 {@code dyDp}（向下为正）后的高度调整量；往上拖变高。 */
    public static int dragTop(int startHeight, int dyDp, int windowHeightDp) {
        return BoundsPolicy.bounded(effectiveHeight(startHeight, windowHeightDp) - dyDp,
            KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP, maxHeightAdjustment(windowHeightDp));
    }

    /**
     * 下沿柄：手指从按下处往上移了 u（{@code dyDp = -u}）。
     *
     * <p>往上（u ≥ 0）：抬高跟着手指走，到上限停；高度尽量反向让出同样多，但不低于下限。所以先拿高度换抬高、上沿不动，高度到下限后整块继续上移、上沿跟着上去，直到抬高到上限。往下（u < 0）：抬高减少、高度反向增加同样多，上沿不动，抬高到 0 或高度到上限时停住。两项都不越界。
     */
    public static Vertical dragBottom(int startHeight, int startLift, int dyDp, int windowHeightDp, boolean barShown) {
        int maxHeight = maxHeightAdjustment(windowHeightDp);
        int minHeight = KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP;
        int height = BoundsPolicy.bounded(effectiveHeight(startHeight, windowHeightDp), minHeight, maxHeight);
        int maxLift = maxLiftDp(barShown);
        int lift = BoundsPolicy.bounded(startLift, 0, maxLift);
        int up = -dyDp;
        if (up >= 0) {
            int nextLift = Math.min(lift + up, maxLift);
            return new Vertical(height - Math.min(nextLift - lift, height - minHeight), nextLift);
        }
        int down = Math.max(up, Math.max(-lift, height - maxHeight));
        return new Vertical(height - down, lift + down);
    }

    /** 「移动」的竖直方向：整块上下平移，只改抬高，大小不变。 */
    public static int dragLift(int startLift, int dyDp, boolean barShown) {
        return BoundsPolicy.bounded(startLift - dyDp, 0, maxLiftDp(barShown));
    }

    /** 上沿柄的读屏范围，单位是高度百分比，上限随窗口封顶。 */
    public static Range heightRange(int heightAdjustment, int windowHeightDp) {
        return Range.of(KeyboardHeightPolicy.MIN_HEIGHT_PERCENT,
            KeyboardHeightPolicy.heightAdjustmentToPercent(maxHeightAdjustment(windowHeightDp)),
            KeyboardHeightPolicy.heightAdjustmentToPercent(effectiveHeight(heightAdjustment, windowHeightDp)));
    }

    /** 上沿柄读屏增减一步（{@code direction} 为 1 或 -1）后的高度调整量：按百分比走 5%，再夹进窗口封顶。 */
    public static int stepHeight(int heightAdjustment, int direction, int windowHeightDp) {
        Range range = heightRange(heightAdjustment, windowHeightDp);
        int percent = BoundsPolicy.bounded(range.current() + Integer.signum(direction) * HEIGHT_STEP_PERCENT,
            range.min(), range.max());
        return BoundsPolicy.bounded(KeyboardHeightPolicy.heightPercentToAdjustment(percent),
            KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP, maxHeightAdjustment(windowHeightDp));
    }

    /** 下沿柄的读屏范围，单位是抬高的 dp：抬高没到上限就能增加（高度到下限后整块上移），没到 0 且高度没到上限就能减少。 */
    public static Range bottomRange(int heightAdjustment, int liftDp, int windowHeightDp, boolean barShown) {
        int max = maxLiftDp(barShown);
        int lift = BoundsPolicy.bounded(liftDp, 0, max);
        Vertical up = dragBottom(heightAdjustment, lift, -1, windowHeightDp, barShown);
        Vertical down = dragBottom(heightAdjustment, lift, 1, windowHeightDp, barShown);
        return new Range(0, max, lift, up.liftDp() != lift, down.liftDp() != lift);
    }

    /** 下沿柄读屏增减一步：抬高 4 dp，高度尽量反向变化同样多，规则同 {@link #dragBottom}。 */
    public static Vertical stepBottom(int heightAdjustment, int liftDp, int direction, int windowHeightDp,
            boolean barShown) {
        return dragBottom(heightAdjustment, liftDp, -Integer.signum(direction) * LIFT_STEP_DP, windowHeightDp, barShown);
    }

    /** 「移动」竖直方向的读屏范围（只改抬高时它就是「移动」的主范围）。 */
    public static Range liftRange(int liftDp, boolean barShown) {
        int max = maxLiftDp(barShown);
        return Range.of(0, max, BoundsPolicy.bounded(liftDp, 0, max));
    }

    // ---- 水平方向（像素） ----

    /** 键盘最窄多少像素：可用宽度的 60% 与 240 dp 取更宽的，但不超过可用宽度。 */
    public static int minWidthPx(int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return 0;
        int byPercent = (int) Math.ceil(availablePx * (double) MIN_WIDTH_PERCENT / 100d);
        return Math.min(availablePx, Math.max(byPercent, minWidthDpPx));
    }

    /** 宽度百分比画成多少像素：不窄于 {@link #minWidthPx}，不宽于可用宽度。 */
    public static int widthPx(int widthPercent, int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return 0;
        int width = (int) Math.round(availablePx * (double) widthPercent / 100d);
        return BoundsPolicy.bounded(width, minWidthPx(availablePx, minWidthDpPx), availablePx);
    }

    /** 键盘左边离可用区域左边多少像素：剩余空间乘以位置千分比。 */
    public static int leftPx(int position, int availablePx, int widthPx) {
        int free = Math.max(0, availablePx - widthPx);
        return (int) Math.round(free * (double) BoundsPolicy.bounded(position, 0, MAX_POSITION) / MAX_POSITION);
    }

    /** 键盘两侧各空出多少像素（左、右），即外框的左右内边距。 */
    public static int[] insetsPx(Horizontal value, int availablePx, int minWidthDpPx) {
        int width = widthPx(value.widthPercent(), availablePx, minWidthDpPx);
        int left = leftPx(value.position(), availablePx, width);
        return new int[] {left, Math.max(0, availablePx - width - left)};
    }

    /** 宽度的读屏范围：下限是 {@link #minWidthPx} 折成的百分比（向上取整）。 */
    public static Range widthRange(Horizontal value, int availablePx, int minWidthDpPx) {
        int min = minWidthPercent(availablePx, minWidthDpPx);
        int current = BoundsPolicy.bounded(value.widthPercent(), min, MAX_WIDTH_PERCENT);
        return Range.of(min, MAX_WIDTH_PERCENT, current);
    }

    /**
     * 左柄或右柄（{@code side}）的读屏范围：范围同 {@link #widthRange(Horizontal, int, int)}，能不能增减按这一侧实际伸缩得动不动算。柄只动自己那条边，键盘贴着左边时左柄没法再往外加宽，贴着右边时右柄同样；只看百分比没到 100 就报「能增」，读屏用户会得到一个做了也没反应的动作。
     */
    public static Range widthRange(Horizontal value, Part side, int availablePx, int minWidthDpPx) {
        Range range = widthRange(value, availablePx, minWidthDpPx);
        return new Range(range.min(), range.max(), range.current(),
            !stepWidth(value, side, 1, availablePx, minWidthDpPx).equals(value),
            !stepWidth(value, side, -1, availablePx, minWidthDpPx).equals(value));
    }

    /** 位置的读屏范围：键盘占满可用宽度时没有可移的余地。 */
    public static Range positionRange(Horizontal value, int availablePx, int minWidthDpPx) {
        int width = widthPx(value.widthPercent(), availablePx, minWidthDpPx);
        int position = BoundsPolicy.bounded(value.position(), 0, MAX_POSITION);
        if (availablePx - width <= 0) return new Range(0, MAX_POSITION, position, false, false);
        return Range.of(0, MAX_POSITION, position);
    }

    static int minWidthPercent(int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return MIN_WIDTH_PERCENT;
        int percent = (int) Math.ceil(minWidthPx(availablePx, minWidthDpPx) * 100d / availablePx);
        return BoundsPolicy.bounded(percent, MIN_WIDTH_PERCENT, MAX_WIDTH_PERCENT);
    }

    private static int percentFor(int widthPx, int availablePx, int minWidthDpPx) {
        int percent = (int) Math.round(widthPx * 100d / availablePx);
        return BoundsPolicy.bounded(percent, minWidthPercent(availablePx, minWidthDpPx), MAX_WIDTH_PERCENT);
    }

    private static int positionFor(int leftPx, int availablePx, int widthPx) {
        int free = availablePx - widthPx;
        if (free <= 0) return DEFAULT_POSITION;
        return BoundsPolicy.bounded((int) Math.round(leftPx * (double) MAX_POSITION / free), 0, MAX_POSITION);
    }

    /** 右沿柄：手指水平移动 {@code dxPx}（向右为正），右边跟着手指，左边不动。 */
    public static Horizontal dragRight(Horizontal start, int dxPx, int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return start;
        int width = widthPx(start.widthPercent(), availablePx, minWidthDpPx);
        int left = leftPx(start.position(), availablePx, width);
        int target = BoundsPolicy.bounded(width + dxPx, minWidthPx(availablePx, minWidthDpPx), availablePx - left);
        int percent = percentFor(target, availablePx, minWidthDpPx);
        return new Horizontal(percent, positionFor(left, availablePx, widthPx(percent, availablePx, minWidthDpPx)));
    }

    /** 左沿柄：手指水平移动 {@code dxPx}（向右为正），左边跟着手指，右边不动。 */
    public static Horizontal dragLeft(Horizontal start, int dxPx, int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return start;
        int width = widthPx(start.widthPercent(), availablePx, minWidthDpPx);
        int right = leftPx(start.position(), availablePx, width) + width;
        int target = BoundsPolicy.bounded(width - dxPx, minWidthPx(availablePx, minWidthDpPx), right);
        int percent = percentFor(target, availablePx, minWidthDpPx);
        int drawn = widthPx(percent, availablePx, minWidthDpPx);
        return new Horizontal(percent, positionFor(Math.max(0, right - drawn), availablePx, drawn));
    }

    /** 「移动」的水平方向：整块左右平移，宽度不变。 */
    public static Horizontal dragPosition(Horizontal start, int dxPx, int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return start;
        int width = widthPx(start.widthPercent(), availablePx, minWidthDpPx);
        int left = BoundsPolicy.bounded(leftPx(start.position(), availablePx, width) + dxPx, 0,
            Math.max(0, availablePx - width));
        return new Horizontal(start.widthPercent(), positionFor(left, availablePx, width));
    }

    /** 左右柄读屏增减一步：宽度按百分比走 5%，从这一侧伸缩，另一侧边不动。 */
    public static Horizontal stepWidth(Horizontal value, Part side, int direction, int availablePx, int minWidthDpPx) {
        if (availablePx <= 0) return value;
        Range range = widthRange(value, availablePx, minWidthDpPx);
        int percent = BoundsPolicy.bounded(range.current() + Integer.signum(direction) * WIDTH_STEP_PERCENT,
            range.min(), range.max());
        int delta = widthPx(percent, availablePx, minWidthDpPx) - widthPx(value.widthPercent(), availablePx, minWidthDpPx);
        return side == Part.LEFT ? dragLeft(value, -delta, availablePx, minWidthDpPx)
            : dragRight(value, delta, availablePx, minWidthDpPx);
    }

    /** 「移动」水平方向读屏增减一步：位置走 50‰。 */
    public static Horizontal stepPosition(Horizontal value, int direction) {
        return new Horizontal(value.widthPercent(), BoundsPolicy.bounded(
            value.position() + Integer.signum(direction) * POSITION_STEP, 0, MAX_POSITION));
    }
}
