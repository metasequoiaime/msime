import app.msime.android.LetterHintTable;

public final class LetterHintTableSmoke {
    public static void main(String[] args) {
        check("…".equals(LetterHintTable.hint("x"))
                && "、".equals(LetterHintTable.hint("c")),
            "中文字母角标的实际提交值保留省略号和顿号");
        String rows = "q1w2e3r4t5y6u7i8o9p0a@s#d¥f%g&h*j(k)l\"z~x…c、v?b!n-m/";
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

        // 中文标点只转写 ( ) " ? !，X/C 的语言模式独立于标点开关。
        java.util.Map<String, String> chinese = java.util.Map.of(
            "j", "（", "k", "）", "l", "“", "v", "？", "b", "！");
        java.util.Map<String, String> chineseFaces = new java.util.HashMap<>(chinese);
        chineseFaces.put("x", "…");
        chineseFaces.put("c", "、");
        for (int index = 0; index < rows.length(); index += 2) {
            String letter = rows.substring(index, index + 1);
            String hint = rows.substring(index + 1, index + 2);
            check(hint.equals(LetterHintTable.face(letter, false)), "非英文半角标点键面：" + letter);
            String englishHint = "x".equals(letter) ? "_" : "c".equals(letter) ? "`" : hint;
            check(englishHint.equals(LetterHintTable.hint(letter, true)), "英文完整角标表：" + letter);
            check(englishHint.equals(LetterHintTable.face(letter, true, false,
                    app.msime.android.ChineseSymbolFaces.Marks.DEFAULT)), "英文键面：" + letter);
            check(chineseFaces.getOrDefault(letter, hint).equals(LetterHintTable.face(letter, true)),
                "Chinese face of " + letter);
            check(LetterHintTable.routesThroughEngine(hint) == chinese.containsKey(letter),
                "Engine route of " + letter);
        }
        check("…".equals(LetterHintTable.face("x", true))
                && "、".equals(LetterHintTable.face("c", true)),
            "Chinese X/C faces retain their punctuation");
        for (String[] entry : new String[][] {{"x", "…", "_"}, {"c", "、", "`"}}) {
            for (String letter : new String[] {entry[0], entry[0].toUpperCase(java.util.Locale.ROOT)}) {
                check(entry[1].equals(LetterHintTable.hint(letter, false)), "中文实际提交值：" + letter);
                check(entry[2].equals(LetterHintTable.hint(letter, true)), "英文实际提交值：" + letter);
                for (boolean punctuation : new boolean[] {false, true}) {
                    check(entry[1].equals(LetterHintTable.face(letter, false, punctuation,
                            app.msime.android.ChineseSymbolFaces.Marks.DEFAULT)),
                        "中文开关标点均保留原角标：" + letter);
                    check(entry[2].equals(LetterHintTable.face(letter, true, punctuation,
                            app.msime.android.ChineseSymbolFaces.Marks.DEFAULT)),
                        "英文开关标点均保持英文角标：" + letter);
                }
            }
        }
        // 直角引号样式下 l 键的 " 提示画『，ASCII 时不变。
        app.msime.android.ChineseSymbolFaces.Marks corner = app.msime.android.ChineseSymbolFaces.Marks.of("corner", "solid");
        check("『".equals(LetterHintTable.face("l", true, corner)), "corner face of l");
        check("\"".equals(LetterHintTable.face("l", false, corner)), "ASCII face of l under corner quotes");
        check("（".equals(LetterHintTable.face("j", true, corner)), "corner quotes leave ( alone");
        check(LetterHintTable.face("1", true) == null && LetterHintTable.face(null, false) == null,
            "non-letters have no face");
        check(!LetterHintTable.routesThroughEngine(null) && !LetterHintTable.routesThroughEngine("")
            && !LetterHintTable.routesThroughEngine("？"), "only single ASCII marks route through the Engine");
        System.out.println("Android letter hints: q1…p0 / a@…l\" / z~x…c、v?b!n-m/ and Chinese faces （）“？！ passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
