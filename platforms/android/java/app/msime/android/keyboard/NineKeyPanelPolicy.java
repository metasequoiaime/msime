package app.msime.android;

import java.util.List;

/**
 * 全拼九键展开候选面板（三栏：左拼音或笔画、中候选、右功能键）里不依赖 Android 视图的判断。
 *
 * <p>笔画前缀和「只留单字」的状态归引擎（`View.nine_key_strokes`、`View.nine_key_single_character`），这里只算下一次要发给 `msime_client_set_nine_key_filter` 的值，宿主不另存一份。
 */
public final class NineKeyPanelPolicy {
    /** 与 C ABI 的上限相同；更长的前缀不可能是任何字的笔顺。 */
    public static final int MAX_STROKES = 64;

    /** 面板左栏放什么：数字串可能的拼音，或者五个笔画键。 */
    public enum Mode { SPELLING, STROKE }

    /** 面板里的 ⌫ 这一下做什么：先撤掉一笔笔画筛选，没有笔画时才是普通的退格。 */
    public enum Backspace { POP_STROKE, ENGINE }

    /** 拼音栏里一项的种类：完整音节、限定下一个音节首字母的大写字母、直接上屏的数字本身。 */
    public enum SpellingKind { SYLLABLE, INITIAL, DIGIT }

    // 与 `msime_client_set_nine_key_filter` 的字节一一对应：h 横、s 竖、p 撇、n 点、z 折；字形与笔画键盘、笔画方案的读音行是同一套。笔画键盘的通配键不是笔顺的一部分，不在这里。
    private static final List<StrokeKeyboardLayout.Key> STROKES = strokeKeys();

    private NineKeyPanelPolicy() {}

    // 不用 Stream.toList()：它要 API 34，宿主的 minSdk 是 28。
    private static List<StrokeKeyboardLayout.Key> strokeKeys() {
        java.util.ArrayList<StrokeKeyboardLayout.Key> keys = new java.util.ArrayList<>(5);
        for (List<StrokeKeyboardLayout.Key> row : StrokeKeyboardLayout.rows()) {
            for (StrokeKeyboardLayout.Key key : row) {
                if (key.input() != StrokeKeyboardLayout.WILDCARD) keys.add(key);
            }
        }
        return List.copyOf(keys);
    }

    /** 左栏笔画模式下的五个笔画键，按 横 竖 撇 点 折 的顺序。 */
    public static List<StrokeKeyboardLayout.Key> strokes() {
        return STROKES;
    }

    /** 展开面板是否画成三栏：九键字母键面上的全拼正在组字时。英文、本地模式、注音九键和其他方案仍是原来的整块候选面板，它们没有拼音栏可放，也没有九键筛选可用。 */
    public static boolean threeColumn(boolean quanpinNineKeyFace, boolean letterLayer,
                                      boolean localMode, boolean composing) {
        return quanpinNineKeyFace && letterLayer && !localMode && composing;
    }

    /** 面板里的 ⌫：笔画模式下有笔画就先撤一笔，否则交给引擎退格（引擎在全部锁定时撤销最后一次锁定，否则删一个数字）。 */
    public static Backspace backspace(Mode mode, String strokes) {
        return mode == Mode.STROKE && strokes != null && !strokes.isEmpty()
            ? Backspace.POP_STROKE : Backspace.ENGINE;
    }

    /** 追加一笔后的前缀；不是五个笔画字母之一、或已到上限时原样返回，调用方据此不发请求。 */
    public static String appendStroke(String strokes, char code) {
        String current = strokes == null ? "" : strokes;
        if (glyph(code) == null || current.length() >= MAX_STROKES) return current;
        return current + code;
    }

    /** 撤掉最后一笔后的前缀。 */
    public static String popStroke(String strokes) {
        return strokes == null || strokes.isEmpty() ? "" : strokes.substring(0, strokes.length() - 1);
    }

    /** 把笔画字母串画成字形（`hs` → 一丨），认不出的字节跳过。 */
    public static String glyphs(String strokes) {
        if (strokes == null) return "";
        StringBuilder text = new StringBuilder(strokes.length());
        for (int index = 0; index < strokes.length(); index++) {
            String glyph = glyph(strokes.charAt(index));
            if (glyph != null) text.append(glyph);
        }
        return text.toString();
    }

    /** 笔画栏顶部那一格的读屏文字。 */
    public static String strokesDescription(String strokes) {
        String glyphs = glyphs(strokes);
        return glyphs.isEmpty() ? "未选笔画" : "已选笔画 " + glyphs;
    }

    private static String glyph(char code) {
        for (StrokeKeyboardLayout.Key key : STROKES) {
            if (key.input() == code) return key.glyph();
        }
        return null;
    }

    /** 切换拼音/笔画后要发给引擎的笔画前缀，null 表示不必发请求：离开笔画模式时清空已有的笔画，笔画筛选只属于笔画模式；进入笔画模式不改筛选。 */
    public static String strokesAfterToggle(Mode mode, String strokes) {
        return mode == Mode.STROKE && strokes != null && !strokes.isEmpty() ? "" : null;
    }

    public static Mode toggledMode(Mode mode) {
        return mode == Mode.STROKE ? Mode.SPELLING : Mode.STROKE;
    }

    /** 拼音/笔画键的键面：显示当前所在的模式。 */
    public static String modeTitle(Mode mode) {
        return mode == Mode.STROKE ? "笔画" : "拼音";
    }

    /** 读屏文字说的是按下去会切到哪一边。 */
    public static String modeDescription(Mode mode) {
        return mode == Mode.STROKE ? "切换到拼音" : "切换到笔画筛选";
    }

    /** 全部/单字键的键面：显示当前的筛选。 */
    public static String singleCharacterTitle(boolean singleCharacter) {
        return singleCharacter ? "单字" : "全部";
    }

    public static String singleCharacterDescription(boolean singleCharacter) {
        return singleCharacter ? "显示全部候选" : "只显示单字";
    }

    /** 拼音栏一项的种类：一个数字是数字本身，大写字母是限定首字母，其余是音节。 */
    public static SpellingKind kind(String spelling) {
        if (spelling != null && spelling.length() == 1) {
            char only = spelling.charAt(0);
            if (only >= '0' && only <= '9') return SpellingKind.DIGIT;
            if (only >= 'A' && only <= 'Z') return SpellingKind.INITIAL;
        }
        return SpellingKind.SYLLABLE;
    }

    /** 拼音栏一项的读屏文字；注音九键的选择条列的是读音。 */
    public static String spellingDescription(String spelling, boolean zhuyin) {
        if (zhuyin) return "选择读音 " + spelling;
        return switch (kind(spelling)) {
            case DIGIT -> "输入数字 " + spelling;
            case INITIAL -> "限定首字母 " + spelling;
            case SYLLABLE -> "选择拼音 " + spelling;
        };
    }
}
