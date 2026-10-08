package app.msime.android;

/**
 * 组词时候选栏上方读音行的高度。
 *
 * <p>设计稿里这一行是 14dp，按系统字体不放大时 12sp 的读音排的。读音的字号是 {@code ImeToolbar.styleTopRow} 定的 12sp，但它按系统字体缩放走（最多放大到 1.15 倍），厂商字体（如 vivo）的 ascent 到 descent 也比默认字体高。行高固定 14dp 时，y、g、p 这类字母的下伸部被行底切掉，y 看起来像 v，g 只剩上半截（#5591）。所以行高取设计高度和读音文字实际高度（字体 ascent 到 descent，再加上下内边距）里大的那个。
 */
public final class ReadingRowPolicy {
    private ReadingRowPolicy() {}

    /**
     * 读音行的像素高度。
     *
     * @param designPx 设计高度换算成的像素，读音再小也不低于它
     * @param ascent 读音画笔的 {@code FontMetricsInt.ascent}，基线以上为负
     * @param descent 读音画笔的 {@code FontMetricsInt.descent}，基线以下为正
     * @param verticalPadding 读音视图的上下内边距之和
     */
    public static int heightPx(int designPx, int ascent, int descent, int verticalPadding) {
        int text = BoundsPolicy.nonNegative(descent - ascent) + BoundsPolicy.nonNegative(verticalPadding);
        return Math.max(BoundsPolicy.nonNegative(designPx), text);
    }
}
