package app.msime.android;

/**
 * 组词时候选栏上方读音行的高度。
 *
 * <p>设计稿里这一行是 14dp，按系统字体不放大时 12sp 的读音排的。读音的字号跟设置「预编辑字号」走（{@link #textSizeSp}，由 {@code ImeStyler.applySkin} 设置，#6107），还按系统字体缩放走（最多放大到 1.15 倍），厂商字体（如 vivo）的 ascent 到 descent 也比默认字体高。行高固定 14dp 时，y、g、p 这类字母的下伸部被行底切掉，y 看起来像 v，g 只剩上半截（#5591）。所以行高取设计高度和读音文字实际高度（字体 ascent 到 descent，再加上下内边距）里大的那个。内边距包括读音行底部和候选行之间固定的间距（{@code ImeToolbar.READING_GAP_DP}），字号再大这段间距也不会被读音占掉。
 */
public final class ReadingRowPolicy {
    /** 读音的设计字号（sp）：「预编辑字号」是共享默认值时读音画这么大，与修 #6107 之前一直看到的 12sp 相同。 */
    public static final float DESIGN_TEXT_SP = 12f;
    /** 共享偏好 `candidate_preedit_font_size` 的默认值（client-core `default_candidate_preedit_font_size`），由 ReadingRowPolicySmoke 对着 Rust 源码锁住。 */
    public static final int DEFAULT_PREEDIT_FONT_SIZE = 15;

    private ReadingRowPolicy() {}

    /**
     * 读音字号（sp，系统字体缩放之前）：按「预编辑字号」相对共享默认值的比例缩放设计字号，与 iOS {@code CandidateFontPreference.preeditScale} 同一种换算。没改过设置的用户读音仍是 12sp，设置 12–32 对应 9.6–25.6sp，调大调小都跟着变。
     *
     * @param preeditFontSize 设置「预编辑字号」（`candidate_preedit_font_size`，宿主已钳到 12–32）
     */
    public static float textSizeSp(int preeditFontSize) {
        return DESIGN_TEXT_SP * preeditFontSize / DEFAULT_PREEDIT_FONT_SIZE;
    }

    /**
     * 读音行的像素高度。
     *
     * @param designPx 设计高度换算成的像素，读音再小也不低于它
     * @param ascent 读音画笔的 {@code FontMetricsInt.ascent}，基线以上为负
     * @param descent 读音画笔的 {@code FontMetricsInt.descent}，基线以下为正
     * @param verticalPadding 读音视图和读音行的上下内边距之和（含读音与候选行之间的间距）
     */
    public static int heightPx(int designPx, int ascent, int descent, int verticalPadding) {
        int text = BoundsPolicy.nonNegative(descent - ascent) + BoundsPolicy.nonNegative(verticalPadding);
        return BoundsPolicy.atLeast(text, BoundsPolicy.nonNegative(designPx));
    }
}
