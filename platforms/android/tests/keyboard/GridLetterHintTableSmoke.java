import app.msime.android.ChineseSymbolFaces;
import app.msime.android.EighteenKeyLayout;
import app.msime.android.FourteenKeyLayout;
import app.msime.android.GridLetterHintTable;

import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** 14 键与 18 键角标符号表（#11063）：每键一个、不含数字、键盘内不重复，中文标点键面和 Engine 路由与 26 键同一套规则。 */
public final class GridLetterHintTableSmoke {
    public static void main(String[] args) {
        String fourteen = "qw;er#ty…ui(op)|as@df-gh、jk:l\"|zx~cv?bn!m/";
        String eighteen = "q;we…rt-y¥u&io(p)|a@sd#fg%h*jk:l\"|z~xc、v?bn!m/";
        exact(FourteenKeyLayout.rows(), false, fourteen);
        exact(EighteenKeyLayout.rows(), true, eighteen);

        for (String symbol : new String[] {"%", "*", "&", "¥"}) {
            for (List<FourteenKeyLayout.Key> row : FourteenKeyLayout.rows()) {
                for (FourteenKeyLayout.Key key : row) {
                    check(!symbol.equals(GridLetterHintTable.hint(key, false)),
                        "14 键把 " + symbol + " 留在符号面板");
                }
            }
        }
        check(GridLetterHintTable.hint(new FourteenKeyLayout.Key("we"), false) == null,
            "18 键的键不在 14 键表里");
        check(GridLetterHintTable.hint(new FourteenKeyLayout.Key("qw"), true) == null,
            "14 键的键不在 18 键表里");
        check(GridLetterHintTable.hint(null, false) == null, "缺少键时没有符号");

        // 中文标点：( ) " ? ! : ; 画中文形式并走 Engine，其余是字符本身、按字面上屏。
        Map<String, String> chinese = Map.of("(", "（", ")", "）", "\"", "“", "?", "？",
            "!", "！", ":", "：", ";", "；");
        ChineseSymbolFaces.Marks marks = ChineseSymbolFaces.Marks.DEFAULT;
        for (boolean grid : new boolean[] {false, true}) {
            for (List<FourteenKeyLayout.Key> row : grid ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows()) {
                for (FourteenKeyLayout.Key key : row) {
                    String hint = GridLetterHintTable.hint(key, grid);
                    check(hint.equals(GridLetterHintTable.face(key, grid, false, marks)),
                        "英文标点键面是字符本身：" + key.letters());
                    check(chinese.getOrDefault(hint, hint).equals(GridLetterHintTable.face(key, grid, true, marks)),
                        "中文标点键面：" + key.letters());
                    check(GridLetterHintTable.routesThroughEngine(hint) == chinese.containsKey(hint),
                        "Engine 路由：" + key.letters() + " " + hint);
                }
            }
        }
        FourteenKeyLayout.Key l = new FourteenKeyLayout.Key("l");
        ChineseSymbolFaces.Marks corner = ChineseSymbolFaces.Marks.of("corner", "solid");
        check("『".equals(GridLetterHintTable.face(l, false, true, corner))
                && "『".equals(GridLetterHintTable.face(l, true, true, corner)),
            "直角引号样式下 L 键画『");
        check("\"".equals(GridLetterHintTable.face(l, true, false, corner)), "英文标点下 L 键仍是 \"");
        check(GridLetterHintTable.face(new FourteenKeyLayout.Key("we"), false, true, marks) == null,
            "没有符号的键没有键面");
        System.out.println("Android 14/18-key corner symbols: " + fourteen + " and " + eighteen + " passed");
    }

    /** 表与 `rows` 一一对应：逐行逐键相同，每键一个符号，没有数字，同一键盘里不重复。 */
    private static void exact(List<List<FourteenKeyLayout.Key>> rows, boolean eighteen, String expected) {
        StringBuilder actual = new StringBuilder();
        Set<String> seen = new HashSet<>();
        for (int index = 0; index < rows.size(); index++) {
            if (index > 0) actual.append('|');
            for (FourteenKeyLayout.Key key : rows.get(index)) {
                String hint = GridLetterHintTable.hint(key, eighteen);
                check(hint != null && hint.codePointCount(0, hint.length()) == 1,
                    "每键一个符号：" + key.letters());
                check(!Character.isDigit(hint.codePointAt(0)), "角标不放数字：" + key.letters());
                check(seen.add(hint), "同一键盘里符号不重复：" + hint);
                actual.append(key.letters()).append(hint);
            }
        }
        check(expected.equals(actual.toString()), (eighteen ? "18" : "14") + " 键表：" + actual);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
