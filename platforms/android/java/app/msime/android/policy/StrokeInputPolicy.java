package app.msime.android;

/**
 * 宿主怎样驱动笔画方案：Engine 读 `stroke.db`，按横竖撇点折（h s p n z，x 为通配）的笔顺给出单字候选。
 *
 * <p>The editor belongs to the Engine: this host sends the stroke letters, marks the view's `reading` (the typed strokes drawn as 一丨丿丶乛＊) inline rather than its `editing_text` (the letters), and inserts whatever the Engine commits (the Stroke contract in msime_client.h). Candidates are on the strip as in pinyin, they are read-only, and nothing is learned.
 */
public final class StrokeInputPolicy {
    /** `SchemeType::Stroke`: the value `View.scheme` and `commit_context.scheme` carry for this scheme. */
    public static final int STROKE_SCHEME = InputSchemeTraits.STROKE;

    private StrokeInputPolicy() {}

    /** Whether the Engine's stroke editor takes the keys and its reading is what the editor marks: the Stroke scheme outside dedicated English. Stroke has no local modes. */
    public static boolean active(int scheme, boolean dedicatedEnglish) {
        return scheme == STROKE_SCHEME && !dedicatedEnglish;
    }
}
