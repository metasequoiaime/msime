package app.msime.android;

/**
 * 剪贴板面板里本机历史的左滑删除（#5962）：一条往左滑，卡片让开、露出后面的删除按钮，再点一下才删掉。两步，不做一滑到底直接删，也不做撤销；置顶的条目同样可以滑。
 *
 * <p>卡片在一个纵向滚动的面板里，自己还有点按（插入）和长按（打开操作行）。所以横滑只在手指越过 touch slop、而且明显是横向时才接手：斜着拖、竖着拖都留给面板滚动，没越过 slop 的轻微移动照旧算点按或长按。没打开的卡片只认向左；打开着的左右都认，向右拖就是收回。
 *
 * <p>坐标单位是像素，`density` 是 dp 到像素的倍数，位移以向右为正。不依赖 Android，供 JVM 冒烟验证。
 */
public final class ClipboardSwipePolicy {
    /** 露出来的删除区宽度。 */
    public static final float REVEAL_DP = 72f;
    /** 双列时一格较窄，露出来的部分最多占格宽的这么多，卡片自己还留得下一半。 */
    public static final float MAX_REVEAL_FRACTION = .5f;
    /** 横向位移至少是纵向的这么多倍才算横滑（约 34° 以内）；更斜的拖动归面板的纵向滚动。 */
    public static final float HORIZONTAL_DOMINANCE = 1.5f;
    /** 松手时的横向速度达到这么快就按甩动的方向停，不看拖了多远。 */
    public static final float FLING_DP_PER_SECOND = 600f;
    /** 打开、收回的动画时长。 */
    public static final long SETTLE_MILLIS = 160;

    private ClipboardSwipePolicy() {}

    /**
     * 删除区的宽度：通常是 {@link #REVEAL_DP}，格子窄时不超过格宽的 {@link #MAX_REVEAL_FRACTION}。
     *
     * @param cellWidth 卡片所在那一格的宽度；还不知道（小于等于 0）时按 {@link #REVEAL_DP}
     */
    public static float revealWidth(float density, float cellWidth) {
        if (density <= 0) throw new IllegalArgumentException("density must be positive");
        float preferred = REVEAL_DP * density;
        return cellWidth > 0 ? BoundsPolicy.atMost(preferred, cellWidth * MAX_REVEAL_FRACTION) : preferred;
    }

    /**
     * 这次拖动是否交给卡片横滑。一旦为真，卡片就接手这次按压：面板不再拦截它去滚动，按下态、长按和点按都撤销，松手不会插入。
     *
     * @param dx 手指相对按下点的横向位移
     * @param dy 手指相对按下点的纵向位移
     * @param touchSlop 系统的 touch slop（像素），没越过它的移动仍算点按
     * @param open 按下时这一条是否已经打开
     */
    public static boolean claims(float dx, float dy, float touchSlop, boolean open) {
        float horizontal = Math.abs(dx);
        if (horizontal <= BoundsPolicy.nonNegative(touchSlop)) return false;
        if (horizontal < Math.abs(dy) * HORIZONTAL_DOMINANCE) return false;
        return open || dx < 0;
    }

    /** 拖动中卡片的位移：起点位移加上手指相对按下点的位移，夹在 `[-reveal, 0]`，不会往右拖出空白，也不会拖过删除区。起点取接手那一刻卡片的位移减去当时的手指位移，卡片从原地开始跟手，不会因为越过 slop 而跳一下。 */
    public static float offset(float startOffset, float dx, float reveal) {
        return BoundsPolicy.bounded(startOffset + dx, -Math.abs(reveal), 0f);
    }

    /**
     * 松手后停在打开还是收回：横向速度达到 {@link #FLING_DP_PER_SECOND} 时按甩动方向（向左打开、向右收回）；慢慢拖的按位置，露出超过一半就打开。
     *
     * @param offset 松手时卡片的位移（小于等于 0）
     * @param velocityX 松手时的横向速度，像素每秒，向右为正
     */
    public static boolean settlesOpen(float offset, float velocityX, float reveal, float density) {
        float fling = FLING_DP_PER_SECOND * density;
        if (velocityX <= -fling) return true;
        if (velocityX >= fling) return false;
        return offset <= -Math.abs(reveal) / 2f;
    }
}
