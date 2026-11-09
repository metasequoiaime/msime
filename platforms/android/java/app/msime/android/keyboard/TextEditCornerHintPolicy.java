package app.msime.android;

/**
 * 文本编辑面板侧键右上角的上滑角标（全选的「选行」，#11111）画不画：角标要整个落在键里，并且和居中的键面文字横向或纵向错开。键太窄又太矮、两者会压在一起时不画，上滑照旧。
 *
 * <p>位置都按画出来的字形（墨迹）算，单位是像素，原点在键的左上角。
 */
public final class TextEditCornerHintPolicy {
    private TextEditCornerHintPolicy() { }

    /**
     * @param keyWidth 键宽
     * @param labelRight 键面文字墨迹的右边
     * @param labelTop 键面文字墨迹的上边
     * @param hintLeft 角标墨迹的左边
     * @param hintBottom 角标墨迹的下边
     * @param gap 两者之间至少留的空隙
     */
    public static boolean fits(float keyWidth, float labelRight, float labelTop,
            float hintLeft, float hintBottom, float gap) {
        if (keyWidth <= 0 || hintLeft < gap) return false;
        return labelRight + gap <= hintLeft || hintBottom + gap <= labelTop;
    }
}
