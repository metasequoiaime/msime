package app.msime.android;

/**
 * 删除键上滑「快速删除」（#5585）：按住删除键往上滑，离开键的上沿 {@link #REVEAL_DP} dp 后停止连删、在键的上方弹出「快速删除」框；手指滑到框的下沿或更高时框变色，表示已经待命，这时松手删掉光标前的全部文字。待命后往下滑回框外即取消待命，再松手什么也不删；框一旦弹出，就算滑回键上，这一次按压也不会再连删或删除。
 *
 * <p>坐标都在同一个覆盖层的坐标系里（y 向下增大），单位是像素，`density` 是 dp 到像素的倍数。无 Android 依赖，供 JVM 回归验证。
 */
public final class BackspaceSwipePolicy {
    /** 手指高出删除键上沿这么多才弹出框，免得按键时手指轻微上移就误触。 */
    public static final float REVEAL_DP = 8f;
    /** 待命线通常在键上沿往上这么多：手指要明确地再往上滑一段。 */
    public static final float ARM_DP = 40f;
    /** 框的高度。 */
    public static final float BOX_HEIGHT_DP = 40f;
    /** 键上方空间不够时（九键的删除键在第一排，上面只有工具栏），框至少保留的高度。 */
    public static final float MIN_BOX_HEIGHT_DP = 28f;
    /** 框的最小宽度，放得下「快速删除」四个字。 */
    public static final float MIN_BOX_WIDTH_DP = 120f;
    /** 框与覆盖层边缘的最小距离。 */
    public static final float MARGIN_DP = 4f;
    /** 待命线至少比弹出线再高这么多：弹出和待命必须是两步，手指弹出框后还要明确地再往上滑一段才会待命。 */
    public static final float MIN_ARM_TRAVEL_DP = 8f;

    /** 这一次按压所处的阶段。 */
    public enum Phase {
        /** 还没弹出框：照常按删除键处理。 */
        IDLE,
        /** 框已弹出但没待命：松手什么也不删。 */
        REVEALED,
        /** 待命：松手删掉光标前的全部文字。 */
        ARMED
    }

    /** 框在覆盖层里的位置。 */
    public record Box(float left, float top, float right, float bottom) {}

    private BackspaceSwipePolicy() { }

    /**
     * 这次按压能不能上滑快速删除：键上方要放得下至少 {@link #MIN_BOX_HEIGHT_DP} 高的框，且待命线比弹出线至少高 {@link #MIN_ARM_TRAVEL_DP}。工具栏隐藏、又没在组字时，九键、注音、笔画第一排的删除键几乎贴着覆盖层上沿，框画不出来，若照样判定，手指刚滑出键沿就直接待命、松手删光却从没看到框，所以这时整个上滑手势不生效，删除键照常连删。
     *
     * @param keyTop 删除键上沿的 y
     */
    public static boolean available(float keyTop, float density) {
        return keyTop - (REVEAL_DP + MIN_ARM_TRAVEL_DP) * density
            >= (MARGIN_DP + MIN_BOX_HEIGHT_DP) * density;
    }

    /**
     * 待命线：手指的 y 小于等于它就待命。通常在键上沿往上 {@link #ARM_DP}；键上方放不下时下移，保证框至少有 {@link #MIN_BOX_HEIGHT_DP} 高，但始终比弹出线高 {@link #MIN_ARM_TRAVEL_DP}。只在 {@link #available} 为真时有意义。
     *
     * @param keyTop 删除键上沿的 y
     */
    public static float armLine(float keyTop, float density) {
        float preferred = keyTop - ARM_DP * density;
        float lowest = (MARGIN_DP + MIN_BOX_HEIGHT_DP) * density;
        float highestAllowed = keyTop - (REVEAL_DP + MIN_ARM_TRAVEL_DP) * density;
        return BoundsPolicy.bounded(preferred, lowest, highestAllowed);
    }

    /**
     * 框的位置：下沿就是待命线，在删除键上方水平居中（宽度取两个键宽与 {@link #MIN_BOX_WIDTH_DP} 的较大者），整体限制在覆盖层内。
     *
     * @param surfaceWidth 覆盖层宽度
     */
    public static Box box(float keyLeft, float keyTop, float keyRight, float surfaceWidth, float density) {
        float margin = MARGIN_DP * density;
        float bottom = armLine(keyTop, density);
        float top = BoundsPolicy.atLeast(bottom - BOX_HEIGHT_DP * density, margin);
        if (top > bottom) top = bottom;
        float width = BoundsPolicy.atMost(
            BoundsPolicy.atLeast(2 * (keyRight - keyLeft), MIN_BOX_WIDTH_DP * density),
            BoundsPolicy.nonNegative(surfaceWidth - 2 * margin));
        float center = (keyLeft + keyRight) / 2f;
        float left = BoundsPolicy.bounded(center - width / 2f, margin,
            BoundsPolicy.atLeast(surfaceWidth - margin - width, margin));
        return new Box(left, top, left + width, bottom);
    }

    /**
     * 手指移动到 `fingerY` 后的阶段。弹出过的框不会收回：之后只在待命与不待命之间切换。键上方放不下框（{@link #available} 为假）时始终是 {@link Phase#IDLE}。
     *
     * @param current 移动前的阶段
     * @param fingerY 手指的 y
     * @param keyTop 删除键上沿的 y
     */
    public static Phase next(Phase current, float fingerY, float keyTop, float density) {
        if (!available(keyTop, density)) return Phase.IDLE;
        if (fingerY <= armLine(keyTop, density)) return Phase.ARMED;
        if (current != Phase.IDLE) return Phase.REVEALED;
        return fingerY <= keyTop - REVEAL_DP * density ? Phase.REVEALED : Phase.IDLE;
    }

    /** 松手时是否删掉光标前的全部文字。 */
    public static boolean clearsOnRelease(Phase phase) {
        return phase == Phase.ARMED;
    }

    /** 每轮最多读、删的 UTF-16 单元数：编辑器的 getTextBeforeCursor 常常只给一段，大文本要分几轮。 */
    public static final int CLEAR_CHUNK = 4096;
    /** 最多删几轮（约一百万个字符），编辑器不配合时也会停下。 */
    public static final int CLEAR_MAX_ROUNDS = 256;

    /** 删掉光标前文字要用到的两个编辑器操作，对应 InputConnection 的同名方法。 */
    public interface Editor {
        /** 光标前最多 `length` 个 UTF-16 单元；编辑器不支持时为 null。 */
        CharSequence textBeforeCursor(int length);

        /** 删掉光标前 `length` 个 UTF-16 单元；编辑器拒绝时为 false。 */
        boolean deleteBeforeCursor(int length);
    }

    /**
     * 一段一段删掉光标前的全部文字：每轮读光标前至多 {@link #CLEAR_CHUNK} 个单元并按读到的长度整段删掉，读到空、编辑器拒绝删除或满 {@link #CLEAR_MAX_ROUNDS} 轮时停下。按读到的长度删，代理对不会被拆开。不靠「这轮读到的和上一轮一样」判断编辑器没删：一长串重复的文字（满屏的「哈」）每一段本来就一样，那样判断会删到一半就停。文字只在调用期间的局部变量里，不保存。
     *
     * @return 删掉的 UTF-16 单元数
     */
    public static long clearBeforeCursor(Editor editor) {
        long deleted = 0;
        for (int round = 0; round < CLEAR_MAX_ROUNDS; round++) {
            CharSequence before = editor.textBeforeCursor(CLEAR_CHUNK);
            if (before == null || before.length() == 0) break;
            int length = before.length();
            if (!editor.deleteBeforeCursor(length)) break;
            deleted += length;
        }
        return deleted;
    }
}
