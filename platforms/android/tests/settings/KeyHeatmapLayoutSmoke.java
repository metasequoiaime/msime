import app.msime.android.KeyHeatmapLayout;
import app.msime.android.KeyHeatmapLayout.Key;
import app.msime.android.KeyPressIds;
import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.NineKeyLayout;
import app.msime.android.NineKeySidebarPolicy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 统计页的按键热力图必须和竖屏默认设置下的触屏键盘一模一样：同样的键、顺序、宽度份额和键面，每个键用键盘按下时登记的 id，次数才落得到它上面。
 *
 * <p>行和宽度从键盘自己的常量推出来比，键盘按键登记的 id 写在 `MSIMEInputService`、`ImeLetterRows`、`ImeLayoutRows` 里，这里读源码核对。
 */
public final class KeyHeatmapLayoutSmoke {
    public static void main(String[] args) throws Exception {
        whitelisted();
        soft();
        nine();
        others();
        shared();
        sources();
        System.out.println("Android key heatmap layout matches the touch keyboard");
    }

    private static void whitelisted() {
        for (List<List<Key>> layout : List.of(KeyHeatmapLayout.SOFT, KeyHeatmapLayout.NINE)) {
            for (List<Key> row : layout) {
                for (Key key : row) {
                    check(key.weight() > 0, "every drawn key has a width: " + key.face());
                    for (String id : key.ids()) check(KeyPressIds.isKnown(id), "a drawn key id is whitelisted: " + id);
                }
            }
        }
    }

    private static void soft() {
        List<List<Key>> rows = KeyHeatmapLayout.SOFT;
        List<List<String>> letters = KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS);
        check(rows.size() == letters.size() + 1, "three letter rows and the action row");
        for (int index = 0; index < letters.size(); index++) {
            List<Key> row = rows.get(index);
            boolean last = index == letters.size() - 1;
            List<Key> letterKeys = last ? row.subList(1, row.size() - 1) : row;
            check(letterKeys.size() == letters.get(index).size(),
                "row " + index + " holds exactly the keyboard's letters, no indent spacers");
            for (int column = 0; column < letterKeys.size(); column++) {
                String letter = letters.get(index).get(column);
                Key key = letterKeys.get(column);
                // 中文全拼的字母键面画大写，送给引擎和计数的仍是同一个字母。
                check(key.face().equals(letter.toUpperCase(java.util.Locale.ROOT)) && key.weight() == 1f
                    && key.ids().equals(List.of(KeyPressIds.forCharacter(letter.charAt(0)))),
                    "letter " + letter + " is drawn uppercase, one key wide, under its own id");
            }
        }
        List<Key> third = rows.get(2);
        check(third.get(0).equals(new Key("分词", KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT, List.of("Quote"))),
            "the third row opens with 分词, counting only Quote; the English-mode ⇧ (ShiftLeft) is listed with the other keys");
        check(third.get(third.size() - 1).equals(new Key("⌫", KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT, List.of("Backspace"))),
            "the third row closes with ⌫ as wide as the keyboard draws it");
        check(faces(rows.get(0)).equals(List.of("Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"))
            && faces(rows.get(1)).equals(List.of("A", "S", "D", "F", "G", "H", "J", "K", "L"))
            && faces(third).equals(List.of("分词", "Z", "X", "C", "V", "B", "N", "M", "⌫")),
            "the 26-key letter rows as the user sees them");
        bottom(rows.get(3), KeyboardLayout.STANDARD_TOUCH_LAYOUT);
    }

    private static void nine() {
        List<List<Key>> rows = KeyHeatmapLayout.NINE;
        List<List<NineKeyLayout.Key>> grid = NineKeyLayout.rows();
        List<String> sidebar = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        // 默认左栏的符号数正好等于网格行数，才能逐行画在网格左边。
        check(sidebar.size() == grid.size() && sidebar.size() <= NineKeySidebarPolicy.VISIBLE_ROWS,
            "the default sidebar fills the column one symbol per grid row");
        check(rows.size() == grid.size() + 1, "three grid rows and the action row");
        List<String> punctuation = NineKeyLayout.punctuation();
        List<Key> actions = List.of(
            new Key("⌫", NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of("Backspace")),
            new Key("重输", NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of()),
            new Key(punctuation.get(punctuation.size() - 1), NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of("SoftPunctuation")));
        for (int index = 0; index < grid.size(); index++) {
            List<Key> row = rows.get(index);
            check(row.size() == grid.get(index).size() + 2, "grid row " + index + " sits between the two side columns");
            check(row.get(0).equals(new Key(sidebar.get(index), NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of("SoftPunctuation"))),
                "the sidebar symbol of row " + index);
            for (int column = 0; column < grid.get(index).size(); column++) {
                NineKeyLayout.Key cell = grid.get(index).get(column);
                check(row.get(column + 1).equals(new Key(NineKeyLayout.face(cell, false),
                        NineKeyLayout.GRID_WEIGHT / grid.get(index).size(), List.of(KeyPressIds.forNineKeyDigit(cell.digit())))),
                    "grid cell " + cell.digit() + " keeps its face, width and Nine id");
            }
            check(row.get(row.size() - 1).equals(actions.get(index)), "the right column of row " + index);
        }
        check(faces(rows.get(0)).equals(List.of("，", "@#", "ABC", "DEF", "⌫"))
            && faces(rows.get(1)).equals(List.of("。", "GHI", "JKL", "MNO", "重输"))
            && faces(rows.get(2)).equals(List.of("？", "PQRS", "TUV", "WXYZ", "！")),
            "the nine-key grid as the user sees it");
        List<String> ids = KeyHeatmapLayout.ids(rows);
        check(!ids.contains("Nine0") && !ids.contains("Comma") && !ids.contains("Period"),
            "the letter page has no 0 key and no comma or period key of its own");
        bottom(rows.get(3), KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT);
    }

    /** 底行：键盘的功能行（默认不交换中/英和 123、没有地球键），逐个比槽位和宽度，再比键面和 id。 */
    private static void bottom(List<Key> row, int touchLayout) {
        List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.designEntries(touchLayout, false, false);
        check(row.size() == entries.size(), "the action row has the keyboard's keys");
        for (int index = 0; index < row.size(); index++) {
            check(row.get(index).weight() == entries.get(index).weight(), "action key " + index + " is as wide as on the keyboard");
        }
        check(faces(row).equals(List.of("符", "123", "空格", "中", "↵")), "符 | 123 | 空格 | 中 | ↵");
        List<List<String>> ids = new ArrayList<>(row.size());
        for (Key key : row) ids.add(key.ids());
        check(ids.equals(List.of(List.of("SoftSymbol"), List.of("SoftLayer"), List.of("Space"), List.of("SoftLanguage"),
            List.of("Enter"))), "the action row ids");
    }

    private static void others() {
        Map<String, Long> counts = Map.of("KeyA", 5L, "Tab", 2L, "F1", 7L, "Nine2", 3L, "Nine0", 1L, "Escape", 0L);
        check(KeyHeatmapLayout.others(counts, false).equals(List.of("F1", "Nine2", "Tab", "Nine0")),
            "the 26-key page lists the nine-key cells and keys it does not draw");
        check(KeyHeatmapLayout.others(counts, true).equals(List.of("F1", "KeyA", "Tab", "Nine0")),
            "the nine-key page lists letters and the 0 key, which its letter page no longer has");
    }

    /** 几个键记同一个 id 时存储分不出是哪一个：同色，但哪个都不印百分数（和 HarmonyOS、iOS 一致）。 */
    private static void shared() {
        check(KeyHeatmapLayout.sharedIds(KeyHeatmapLayout.NINE).equals(Set.of("SoftPunctuation")),
            "on the nine-key board only the sidebar and the right-column punctuation share an id");
        check(KeyHeatmapLayout.sharedIds(KeyHeatmapLayout.SOFT).isEmpty(),
            "on the 26-key board no two keys share an id; 分词 records two ids on one key");
        int punctuation = 0;
        for (List<Key> row : KeyHeatmapLayout.NINE) {
            for (Key key : row) if (key.ids().contains("SoftPunctuation")) punctuation++;
        }
        check(punctuation == 4, "，。？ and ！ all record SoftPunctuation");
    }

    private static void sources() throws Exception {
        String service = read("core/MSIMEInputService.java");
        String letterRows = read("core/ImeLetterRows.java");
        String layoutRows = read("core/ImeLayoutRows.java");
        String bottomRow = read("core/ImeBottomRow.java");
        check(service.contains("keyId(button(controls, \"符\", imePanels::showSymbolPanel), \"SoftSymbol\")")
            && service.contains("keyId(layerButton, \"SoftLayer\")")
            && service.contains("languageButton = keyId(button(controls, \"中/英\", this::toggleInputLanguage), \"SoftLanguage\")"),
            "the keyboard still records 符, 123 and 中 under the ids the heatmap draws");
        check(bottomRow.contains("new SpaceKeyFace(s), \"Space\")") && bottomRow.contains("KeyboardIconKey.Kind.RETURN), \"Enter\")"),
            "the keyboard still records space and return under the ids the heatmap draws");
        check(service.contains("shiftButton.setText(separator ? \"分词\"")
            && letterRows.contains("String id = separator ? \"Quote\" : \"ShiftLeft\";"),
            "the 全拼 case key is still 分词 recording Quote, ⇧ recording ShiftLeft otherwise");
        check(layoutRows.contains("weightedMatchParentParams(NineKeyLayout.SIDE_COLUMN_WEIGHT)")
            && layoutRows.contains("container.addView(grid, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.GRID_WEIGHT));")
            && layoutRows.contains(": KeyPressIds.forNineKeyDigit(key.digit()));")
            && layoutRows.contains("Button rewrite = s.keyboardKey(\"重输\""),
            "the nine-key frame still uses the heatmap's column weights, Nine ids and the uncounted 重输");
        String view = read("home/KeyHeatmapView.java");
        check(view.contains("KeyHeatmapLayout.SOFT") && view.contains("KeyHeatmapLayout.NINE") && !view.contains("new Key("),
            "the view draws the derived layout instead of a hand-written copy");
        check(view.contains("KeyHeatmapLayout.sharedIds(NINE)") && view.contains("!containsAny(shared, key.ids())")
            && !view.contains("labelled"),
            "keys sharing an id are coloured alike and none of them prints a percentage");
        String page = read("home/StatisticsFragment.java");
        check(page.contains("KeyHeatmapLayout.others(presses, nine)")
            && page.contains("ViewPolicy.setVisible(card, !ids.isEmpty());")
            && page.contains("otherKeys(context, others, presses, nineKey);"),
            "the 其他键 card lists the undrawn keys, hides when empty and follows the 26 / 9 switch");
    }

    private static List<String> faces(List<Key> row) {
        List<String> faces = new ArrayList<>(row.size());
        for (Key key : row) faces.add(key.face());
        return faces;
    }

    private static String read(String relative) throws Exception {
        return Files.readString(repoRoot().resolve("platforms/android/java/app/msime/android/" + relative),
            StandardCharsets.UTF_8);
    }

    private static Path repoRoot() {
        Path path = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (path != null) {
            if (Files.isRegularFile(path.resolve("platforms/android/java/app/msime/android/home/KeyHeatmapView.java"))) return path;
            path = path.getParent();
        }
        throw new IllegalStateException("repository root not found");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
