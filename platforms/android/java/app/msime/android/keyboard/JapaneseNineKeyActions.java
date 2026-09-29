package app.msime.android;

/** Japanese side-key labels; composition and conversion remain owned by Engine. */
public final class JapaneseNineKeyActions {
    private JapaneseNineKeyActions() { }

    public static String spaceTitle(boolean composing) {
        return composing ? "変換" : "空白";
    }

    public static String returnTitle(boolean composing) {
        return composing ? "確定" : "改行";
    }
}
