import app.msime.android.KeyboardLayout;
import app.msime.android.NineKeyPanelPolicy;
import app.msime.android.NineKeyPanelPolicy.Backspace;
import app.msime.android.NineKeyPanelPolicy.Mode;
import app.msime.android.NineKeyPanelPolicy.SpellingKind;
import app.msime.android.StrokeKeyboardLayout;
import java.util.List;

public final class NineKeyPanelPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 五个笔画键与 msime_client_set_nine_key_filter 的字节一一对应，通配不在其中。
        List<StrokeKeyboardLayout.Key> strokes = NineKeyPanelPolicy.strokes();
        check(strokes.stream().map(StrokeKeyboardLayout.Key::input).toList()
            .equals(List.of('h', 's', 'p', 'n', 'z')), "stroke letters");
        check(strokes.stream().map(StrokeKeyboardLayout.Key::glyph).toList()
            .equals(List.of("一", "丨", "丿", "丶", "乛")), "stroke glyphs");

        int nineKey = KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        check(NineKeyPanelPolicy.threeColumn(nineKey, true, false, true), "quanpin composing");
        for (int other : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT,
                KeyboardLayout.STROKE_LAYOUT, KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT})
            check(!NineKeyPanelPolicy.threeColumn(other, true, false, true), "other layouts keep the grid: " + other);
        check(!NineKeyPanelPolicy.threeColumn(nineKey, false, false, true), "digit layer");
        check(!NineKeyPanelPolicy.threeColumn(nineKey, true, true, true), "local mode");
        check(!NineKeyPanelPolicy.threeColumn(nineKey, true, false, false), "idle");

        check(NineKeyPanelPolicy.appendStroke("", 'h').equals("h"), "first stroke");
        check(NineKeyPanelPolicy.appendStroke("h", 'z').equals("hz"), "second stroke");
        check(NineKeyPanelPolicy.appendStroke("h", 'x').equals("h"), "wildcard is not a filter stroke");
        check(NineKeyPanelPolicy.appendStroke(null, 'p').equals("p"), "missing prefix");
        String full = "h".repeat(NineKeyPanelPolicy.MAX_STROKES);
        check(NineKeyPanelPolicy.appendStroke(full, 's').equals(full), "limit");
        check(NineKeyPanelPolicy.popStroke("hsp").equals("hs"), "pop");
        check(NineKeyPanelPolicy.popStroke("").isEmpty(), "pop empty");
        check(NineKeyPanelPolicy.glyphs("hspnz").equals("一丨丿丶乛"), "glyphs");
        check(NineKeyPanelPolicy.glyphs("hq").equals("一"), "unknown bytes skipped");
        check(NineKeyPanelPolicy.strokesDescription("").equals("未选笔画"), "empty description");
        check(NineKeyPanelPolicy.strokesDescription("hs").equals("已选笔画 一丨"), "description");

        // ⌫：笔画模式下先撤一笔，没有笔画或在拼音模式时交给引擎。
        check(NineKeyPanelPolicy.backspace(Mode.STROKE, "hs") == Backspace.POP_STROKE, "pop first");
        check(NineKeyPanelPolicy.backspace(Mode.STROKE, "") == Backspace.ENGINE, "engine without strokes");
        check(NineKeyPanelPolicy.backspace(Mode.SPELLING, "hs") == Backspace.ENGINE, "engine in spelling mode");

        // 离开笔画模式时清掉笔画筛选，进入笔画模式不改筛选。
        check("".equals(NineKeyPanelPolicy.strokesAfterToggle(Mode.STROKE, "hs")), "leave clears");
        check(NineKeyPanelPolicy.strokesAfterToggle(Mode.STROKE, "") == null, "nothing to clear");
        check(NineKeyPanelPolicy.strokesAfterToggle(Mode.SPELLING, "") == null, "enter keeps");
        check(NineKeyPanelPolicy.toggledMode(Mode.SPELLING) == Mode.STROKE, "toggle to stroke");
        check(NineKeyPanelPolicy.toggledMode(Mode.STROKE) == Mode.SPELLING, "toggle to spelling");
        check(NineKeyPanelPolicy.modeTitle(Mode.SPELLING).equals("拼音"), "mode title");
        check(NineKeyPanelPolicy.modeTitle(Mode.STROKE).equals("笔画"), "stroke title");
        check(NineKeyPanelPolicy.singleCharacterTitle(false).equals("全部"), "all title");
        check(NineKeyPanelPolicy.singleCharacterTitle(true).equals("单字"), "single title");
        check(NineKeyPanelPolicy.singleCharacterDescription(false).equals("只显示单字"), "single action");

        // 拼音栏的三种项：音节、限定首字母的大写字母、直接上屏的数字。
        check(NineKeyPanelPolicy.kind("ming") == SpellingKind.SYLLABLE, "syllable");
        check(NineKeyPanelPolicy.kind("M") == SpellingKind.INITIAL, "initial");
        check(NineKeyPanelPolicy.kind("6") == SpellingKind.DIGIT, "digit");
        check(NineKeyPanelPolicy.kind("a") == SpellingKind.SYLLABLE, "single-letter syllable");
        check(NineKeyPanelPolicy.spellingDescription("ning", false).equals("选择拼音 ning"), "syllable label");
        check(NineKeyPanelPolicy.spellingDescription("N", false).equals("限定首字母 N"), "initial label");
        check(NineKeyPanelPolicy.spellingDescription("6", false).equals("输入数字 6"), "digit label");
        check(NineKeyPanelPolicy.spellingDescription("ㄋㄧˇ", true).equals("选择读音 ㄋㄧˇ"), "zhuyin label");
        System.out.println("Android nine-key panel: strokes, backspace, toggles and spelling labels passed");
    }
}
