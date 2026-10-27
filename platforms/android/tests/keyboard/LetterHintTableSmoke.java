import app.msime.android.LetterHintTable;

public final class LetterHintTableSmoke {
    public static void main(String[] args) {
        String rows = "q1w2e3r4t5y6u7i8o9p0a@s#d¥f%g&h*j(k)l\"z~x_c`v?b!n-m/";
        int count = 0;
        for (int index = 0; index < rows.length(); index += 2) {
            String letter = rows.substring(index, index + 1);
            String hint = rows.substring(index + 1, index + 2);
            check(hint.equals(LetterHintTable.hint(letter)), "hint of " + letter);
            check(hint.equals(LetterHintTable.hint(letter.toUpperCase(java.util.Locale.ROOT))),
                "uppercase " + letter + " reads the same hint");
            count++;
        }
        check(count == 26, "all 26 letters carry a hint");
        check(LetterHintTable.hint("1") == null && LetterHintTable.hint("") == null
            && LetterHintTable.hint(null) == null && LetterHintTable.hint("ab") == null,
            "non-letters have no hint");
        check(!LetterHintTable.hasHint("，") && LetterHintTable.hasHint("m"), "hasHint");

        // 中文标点生效时只有 Engine 会转写的 ( ) " ? ! 换成中文键面，其余 21 键（数字、@#%&*~-/、¥、_、`）不变（#7069、#9992）。
        java.util.Map<String, String> chinese = java.util.Map.of(
            "j", "（", "k", "）", "l", "“", "v", "？", "b", "！");
        java.util.Map<String, String> chineseFaces = new java.util.HashMap<>(chinese);
        chineseFaces.put("x", "…");
        chineseFaces.put("c", "、");
        for (int index = 0; index < rows.length(); index += 2) {
            String letter = rows.substring(index, index + 1);
            String hint = rows.substring(index + 1, index + 2);
            check(hint.equals(LetterHintTable.face(letter, false)), "ASCII face of " + letter);
            check(chineseFaces.getOrDefault(letter, hint).equals(LetterHintTable.face(letter, true)),
                "Chinese face of " + letter);
            check(LetterHintTable.routesThroughEngine(hint) == chinese.containsKey(letter),
                "Engine route of " + letter);
        }
        check("…".equals(LetterHintTable.face("x", true))
                && "、".equals(LetterHintTable.face("c", true)),
            "Chinese X/C faces retain their punctuation");
        // 直角引号样式下 l 键的 " 提示画『，ASCII 时不变。
        app.msime.android.ChineseSymbolFaces.Marks corner = app.msime.android.ChineseSymbolFaces.Marks.of("corner", "solid");
        check("『".equals(LetterHintTable.face("l", true, corner)), "corner face of l");
        check("\"".equals(LetterHintTable.face("l", false, corner)), "ASCII face of l under corner quotes");
        check("（".equals(LetterHintTable.face("j", true, corner)), "corner quotes leave ( alone");
        check(LetterHintTable.face("1", true) == null && LetterHintTable.face(null, false) == null,
            "non-letters have no face");
        check(!LetterHintTable.routesThroughEngine(null) && !LetterHintTable.routesThroughEngine("")
            && !LetterHintTable.routesThroughEngine("？"), "only single ASCII marks route through the Engine");
        System.out.println("Android letter hints: q1…p0 / a@…l\" / z~x_c`v?b!n-m/ and Chinese faces （）“？！ passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
