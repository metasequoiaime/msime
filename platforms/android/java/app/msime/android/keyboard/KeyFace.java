package app.msime.android;

/**
 * 一个键在某个状态下画什么（{@link KeyFaceSpec#face}）：主标签要么是文字、要么是图标，右上角可以另有一段提示。文字和图标是两种内容：画图标时主标签位置没有文字，不进键组的内容域；节点文字（读屏、按键统计）由调用方另设。
 *
 * @param text 主标签的文字；画图标时为 {@code null}
 * @param icon 主标签的图标；画文字时为 {@code null}
 * @param corner 右上角的提示；不画时为空串
 */
public record KeyFace(String text, KeyboardIconKey.Kind icon, String corner) {
    public KeyFace {
        if ((text == null) == (icon == null)) throw new IllegalArgumentException("a key face is either text or an icon");
        corner = corner == null ? "" : corner;
    }

    /** 画文字。 */
    public static KeyFace text(String text) {
        return new KeyFace(text, null, "");
    }

    /** 画图标。 */
    public static KeyFace icon(KeyboardIconKey.Kind icon) {
        return new KeyFace(null, icon, "");
    }

    /** 同一个主标签，右上角换成 {@code corner}（空串或 {@code null} 不画）。 */
    public KeyFace withCorner(String corner) {
        return new KeyFace(text, icon, corner);
    }

    /** 主标签进内容域的文字：画图标时为空串。 */
    public String mainText() {
        return text == null ? "" : text;
    }
}
