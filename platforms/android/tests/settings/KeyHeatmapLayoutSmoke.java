import app.msime.android.KeyHeatmapLayout;
import app.msime.android.KeyHeatmapLayout.Key;
import app.msime.android.KeyPressIds;
import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.NineKeyLayout;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.TouchKeyStyle;
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
        styles();
        tallReturn();
        sources();
        System.out.println("Android key heatmap layout matches the touch keyboard");
    }

    private static void whitelisted() {
        for (List<List<Key>> layout : List.of(KeyHeatmapLayout.SOFT, KeyHeatmapLayout.NINE,
                KeyHeatmapLayout.nine(TouchKeyStyle.Row.TALL_RETURN))) {
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
        boolean comma = KeyboardActionRow.usesCommaSecondaryPeriod(touchLayout);
        check(faces(row).equals(comma ? List.of("123", "符", "空格", "，", "中", "↵")
            : List.of("符", "123", "空格", "中", "↵")), "底行键面跟随实际布局");
        List<List<String>> ids = new ArrayList<>(row.size());
        for (Key key : row) ids.add(key.ids());
        check(ids.equals(comma ? List.of(List.of("SoftLayer"), List.of("SoftSymbol"), List.of("Space"),
            List.of("Comma"), List.of("SoftLanguage"), List.of("Enter"))
            : List.of(List.of("SoftSymbol"), List.of("SoftLayer"), List.of("Space"), List.of("SoftLanguage"),
            List.of("Enter"))), "底行按键统计跟随实际布局");
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

    /** 热力图按用户当前的底行样式画（.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md），底行与键盘的 designEntries 一一对应；26 键打开顶部数字行时多画一行 Digit1…Digit0。 */
    private static void styles() {
        check(KeyHeatmapLayout.soft(TouchKeyStyle.Row.STANDARD, false).equals(KeyHeatmapLayout.SOFT)
            && KeyHeatmapLayout.nine(TouchKeyStyle.Row.STANDARD).equals(KeyHeatmapLayout.NINE),
            "the standard style draws today's heatmaps");
        for (TouchKeyStyle.Row style : TouchKeyStyle.TWENTY_SIX_KEY_ROWS) {
            List<List<Key>> rows = KeyHeatmapLayout.soft(style, false);
            styledBottom(rows.get(rows.size() - 1), KeyboardLayout.STANDARD_TOUCH_LAYOUT, style);
        }
        check(faces(last(KeyHeatmapLayout.soft(TouchKeyStyle.Row.COMMA_PERIOD, false)))
                .equals(List.of("123", "符", "，", "空格", "。", "中", "↵"))
            && faces(last(KeyHeatmapLayout.soft(TouchKeyStyle.Row.LONG_SPACE, false))).equals(List.of("123", "，", "空格", "中", "↵"))
            && faces(last(KeyHeatmapLayout.soft(TouchKeyStyle.Row.EMOJI, false))).equals(List.of("123", "😀", "空格", "，", "中", "↵")),
            "26-key bottom rows as each style draws them");
        for (TouchKeyStyle.Row style : TouchKeyStyle.NINE_KEY_ROWS) {
            // 右侧大回车改的是整个外框，见 tallReturn()。
            if (style == TouchKeyStyle.Row.TALL_RETURN) continue;
            List<List<Key>> rows = KeyHeatmapLayout.nine(style);
            check(rows.subList(0, 3).equals(KeyHeatmapLayout.NINE.subList(0, 3)), "the nine-key grid ignores the style");
            styledBottom(rows.get(3), KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, style);
        }
        check(faces(last(KeyHeatmapLayout.nine(TouchKeyStyle.Row.COMMA_PERIOD))).equals(List.of("符", "123", "，", "空格", "。", "中", "↵"))
            && faces(last(KeyHeatmapLayout.nine(TouchKeyStyle.Row.EMOJI))).equals(List.of("符", "123", "😀", "空格", "中", "↵")),
            "nine-key bottom rows as each style draws them");
        check(KeyHeatmapLayout.sharedIds(KeyHeatmapLayout.nine(TouchKeyStyle.Row.COMMA_PERIOD)).equals(Set.of("SoftPunctuation")),
            "the bottom-row comma and period record Comma and Period, not the sidebar's SoftPunctuation");
        List<List<Key>> numbered = KeyHeatmapLayout.soft(TouchKeyStyle.Row.STANDARD, true);
        check(numbered.size() == KeyHeatmapLayout.SOFT.size() + 1 && numbered.subList(1, numbered.size()).equals(KeyHeatmapLayout.SOFT),
            "the number row sits above the unchanged 26-key board");
        check(faces(numbered.get(0)).equals(KeyboardLayout.NUMBER_ROW), "the number row is 1–0");
        for (Key key : numbered.get(0))
            check(key.weight() == 1f && key.ids().equals(List.of("Digit" + key.face())), "digit " + key.face() + " counts as Digit");
        Map<String, Long> counts = Map.of("Digit1", 4L, "SoftEmoji", 2L);
        check(KeyHeatmapLayout.others(counts, KeyHeatmapLayout.SOFT).equals(List.of("Digit1", "SoftEmoji")),
            "without the number row and the emoji style both are listed under 其他键");
        check(KeyHeatmapLayout.others(counts, KeyHeatmapLayout.soft(TouchKeyStyle.Row.EMOJI, true)).isEmpty(),
            "with them both are drawn");
        // 「中/英键放在空格左边」（#10228）：热力图底行与键盘按同一条规则重排，只换次序，记的 id 不变；字母和九宫格不动。
        check(faces(last(KeyHeatmapLayout.soft(TouchKeyStyle.Row.STANDARD, false, false, true)))
                .equals(List.of("123", "符", "中", "空格", "，", "↵"))
            && faces(last(KeyHeatmapLayout.soft(TouchKeyStyle.Row.COMMA_PERIOD, false, true, true)))
                .equals(List.of("符", "123", "中", "，", "空格", "。", "↵"))
            && faces(last(KeyHeatmapLayout.nine(TouchKeyStyle.Row.STANDARD, true))).equals(List.of("符", "123", "中", "空格", "↵"))
            && faces(last(KeyHeatmapLayout.nine(TouchKeyStyle.Row.TALL_RETURN, true))).subList(0, 4)
                .equals(List.of("符", "123", "中", "空格")), "the heatmap bottom rows follow 中/英 on the left");
        for (TouchKeyStyle.Row style : TouchKeyStyle.NINE_KEY_ROWS) {
            List<List<Key>> right = KeyHeatmapLayout.nine(style, false);
            List<List<Key>> left = KeyHeatmapLayout.nine(style, true);
            check(right.equals(KeyHeatmapLayout.nine(style)) && left.subList(0, 3).equals(right.subList(0, 3))
                && last(left).containsAll(last(right)) && last(left).size() == last(right).size()
                && Set.copyOf(KeyHeatmapLayout.ids(left)).equals(Set.copyOf(KeyHeatmapLayout.ids(right))),
                "nine " + style + ": only the bottom row's order changes");
        }
        for (TouchKeyStyle.Row style : TouchKeyStyle.TWENTY_SIX_KEY_ROWS) {
            List<List<Key>> left = KeyHeatmapLayout.soft(style, true, false, true);
            List<List<Key>> right = KeyHeatmapLayout.soft(style, true, false);
            check(left.subList(0, left.size() - 1).equals(right.subList(0, right.size() - 1))
                && last(left).containsAll(last(right)) && last(left).size() == last(right).size(),
                "26 " + style + ": only the bottom row's order changes");
        }
    }

    /**
     * 右侧大回车的九键外框：右列 ⌫、重输，回车（Enter）从第三行跨到底行，底行最左是与左栏同宽的 符，九宫格正下方的 123 | 空格 | 中 恰好与九宫格同宽；底行回车那一格是不画、不记 id 的占位。每行总份额相同、跨行的回车和占位同宽，所以两行的右列左边缘对齐。统计 id 不变。
     */
    private static void tallReturn() {
        List<List<Key>> rows = KeyHeatmapLayout.nine(TouchKeyStyle.Row.TALL_RETURN);
        float side = NineKeyLayout.SIDE_COLUMN_WEIGHT;
        float frame = 2 * side + NineKeyLayout.GRID_WEIGHT;
        check(rows.size() == 4, "three grid rows and the bottom row");
        for (int index = 0; index < 3; index++) {
            List<Key> row = rows.get(index);
            check(row.subList(1, 4).equals(KeyHeatmapLayout.NINE.get(index).subList(1, 4)), "the grid is unchanged in row " + index);
            check(row.get(0).face().equals(NineKeySidebarPolicy.TALL_RETURN_LETTER_SYMBOLS.get(index))
                && row.get(0).ids().equals(List.of("SoftPunctuation")) && row.get(0).weight() == side, "sidebar cell " + index);
        }
        check(faces(rows.get(0)).get(4).equals("⌫") && rows.get(0).get(4).ids().equals(List.of("Backspace"))
            && faces(rows.get(1)).get(4).equals("重输") && rows.get(1).get(4).ids().isEmpty(),
            "the right column keeps ⌫ and the uncounted 重输");
        Key enter = rows.get(2).get(4);
        check(enter.face().equals("↵") && enter.ids().equals(List.of("Enter")) && enter.rows() == 2 && enter.weight() == side,
            "return spans the third row and the bottom row in the right column");
        List<Key> bottom = rows.get(3);
        check(faces(bottom).equals(List.of("符", "123", "空格", "中", "")), "bottom row: 符 under the sidebar, then 123 | space | 中");
        check(bottom.get(0).ids().equals(List.of("SoftSymbol")) && bottom.get(0).weight() == side, "符 is as wide as the sidebar");
        Key covered = bottom.get(bottom.size() - 1);
        check(covered.rows() == 0 && covered.ids().isEmpty() && covered.weight() == side, "the return's lower half is a placeholder");
        check(bottom.get(1).ids().equals(List.of("SoftLayer")) && bottom.get(2).ids().equals(List.of("Space"))
            && bottom.get(3).ids().equals(List.of("SoftLanguage")), "statistics ids are unchanged");
        float strip = 0;
        for (Key key : bottom.subList(1, bottom.size() - 1)) strip += key.weight();
        check(Math.abs(strip - NineKeyLayout.GRID_WEIGHT) < 1e-4f, "the strip under the grid is exactly as wide as the grid");
        for (List<Key> row : rows) {
            float sum = 0;
            for (Key key : row) sum += key.weight();
            check(Math.abs(sum - frame) < 1e-4f && row.size() == 5, "every row spans the frame with five keys");
        }
        // 123 和中/英在整行里的占比与标准九键底行相同，空格占剩下的。
        List<Key> standard = KeyHeatmapLayout.NINE.get(3);
        float standardTotal = 0;
        for (Key key : standard) standardTotal += key.weight();
        check(Math.abs(bottom.get(1).weight() / frame - standard.get(1).weight() / standardTotal) < 1e-5f
            && Math.abs(bottom.get(3).weight() / frame - standard.get(3).weight() / standardTotal) < 1e-5f,
            "123 and 中 keep their width from the standard bottom row");
        check(KeyHeatmapLayout.sharedIds(rows).equals(Set.of("SoftPunctuation")), "only the sidebar shares SoftPunctuation");
        int punctuation = 0;
        for (List<Key> row : rows) for (Key key : row) if (key.ids().contains("SoftPunctuation")) punctuation++;
        check(punctuation == 3, "the right-column ！ is gone; the sidebar draws three of its four symbols");
        check(KeyHeatmapLayout.ids(rows).containsAll(List.of("SoftSymbol", "SoftLayer", "Space", "SoftLanguage", "Enter", "Backspace")),
            "every bottom-row id is drawn");
        check(KeyHeatmapLayout.others(Map.of("SoftSymbol", 3L, "Enter", 2L, "Comma", 1L), rows).equals(List.of("Comma")),
            "only keys the frame does not draw are listed under 其他键");
    }

    private static List<Key> last(List<List<Key>> rows) {
        return rows.get(rows.size() - 1);
    }

    private static void styledBottom(List<Key> row, int touchLayout, TouchKeyStyle.Row style) {
        List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.designEntries(touchLayout, false, false, false, false, style);
        check(row.size() == entries.size(), "the styled action row has the keyboard's keys: " + style);
        for (int index = 0; index < row.size(); index++) {
            check(row.get(index).weight() == entries.get(index).weight(), "styled action key " + index + " width: " + style);
            String id = switch (entries.get(index).slot()) {
                case SYMBOL_PANEL -> "SoftSymbol";
                case LAYER -> "SoftLayer";
                case EMOJI -> "SoftEmoji";
                case COMMA -> "Comma";
                case PERIOD -> "Period";
                case SPACE -> "Space";
                case LANGUAGE -> "SoftLanguage";
                case RETURN -> "Enter";
                default -> throw new AssertionError("unexpected slot " + entries.get(index).slot());
            };
            check(row.get(index).ids().equals(List.of(id)), "styled action key " + index + " counts as " + id + ": " + style);
        }
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
        check(bottomRow.contains("emojiButton = s.keyId(icon, \"SoftEmoji\")"),
            "the emoji style's 😀 records SoftEmoji, the id the heatmap draws for it");
        check(service.contains("shiftButton.setText(separator ? \"分词\"")
            && letterRows.contains("String id = separator ? \"Quote\" : \"ShiftLeft\";"),
            "the 全拼 case key is still 分词 recording Quote, ⇧ recording ShiftLeft otherwise");
        check(layoutRows.contains("weightedMatchParentParams(NineKeyLayout.SIDE_COLUMN_WEIGHT)")
            && layoutRows.contains("else container.addView(grid, LayoutPolicy.weightedMatchParentParams(NineKeyLayout.GRID_WEIGHT));")
            && layoutRows.contains(": KeyPressIds.forNineKeyDigit(key.digit()));")
            && layoutRows.contains("Button rewrite = s.keyboardKey(\"重输\""),
            "the nine-key frame still uses the heatmap's column weights, Nine ids and the uncounted 重输");
        check(layoutRows.contains("s.imeBottomRow.placeKey(cell, KeyboardActionRow.DesignSlot.SYMBOL_PANEL, s.symbolPanelButton,")
            && layoutRows.contains("s.imeBottomRow.placeKey(cell, KeyboardActionRow.DesignSlot.RETURN, s.enterButton,")
            && layoutRows.contains("KeyboardActionRow.tallReturnEntries(gridShare, globe,")
            && layoutRows.contains("else addNineKey(actions, nineKeyRewriteKey());"),
            "the keyboard's tall frame puts the same keys where the heatmap draws them");
        // 右侧大回车的数字层（热力图不画数字层）：右列 ⌫ 下面是小数点，空格在中列 0 的右边（KeyboardActionRow.tallReturnEntries），与 iOS、鸿蒙相同。
        check(layoutRows.contains("if (digits) addNineKey(actions, nineKeyDecimalKey());")
            && !layoutRows.contains("s.imeBottomRow.placeKey(actions, KeyboardActionRow.DesignSlot.SPACE"),
            "the tall frame's number layer puts the decimal point in the right column and leaves space in the strip");
        // 「按键布局」预览的底行取自这里（KeyLayoutStylesPolicy），长空格样式的空格也写「空格」，与 iOS、鸿蒙的预览一致。
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.FOURTEEN_KEY_LAYOUT,
                KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT}) {
            for (TouchKeyStyle.Row style : TouchKeyStyle.Row.values()) {
                for (Key key : KeyHeatmapLayout.bottom(layout, style)) {
                    if (key.ids().contains("Space"))
                        check(key.face().equals("空格"), "the layout preview labels every space key 空格: " + style);
                }
            }
        }
        String view = read("home/KeyHeatmapView.java");
        check(view.contains("if (key.rows() == 0) {")
            && view.contains("float keyHeight = key.rows() * height + (key.rows() - 1) * gap;"),
            "the view draws a spanning return and skips the cell it covers");
        check(view.contains("KeyHeatmapLayout.SOFT") && view.contains("KeyHeatmapLayout.NINE") && !view.contains("new Key("),
            "the view draws the derived layout instead of a hand-written copy");
        check(view.contains("KeyHeatmapLayout.sharedIds(nine)") && view.contains("!containsAny(shared, key.ids())")
            && !view.contains("labelled"),
            "keys sharing an id are coloured alike and none of them prints a percentage");
        String page = read("home/StatisticsFragment.java");
        check(page.contains("KeyHeatmapLayout.others(presses, drawn)")
            && page.contains("ViewPolicy.setVisible(card, !ids.isEmpty());")
            && page.contains("otherKeys(context, others, presses, layouts.get(index == 1 ? 1 : 0));"),
            "the 其他键 card lists the undrawn keys, hides when empty and follows the 26 / 9 switch");
        check(page.contains("heatmap.setLayouts(layouts.get(0), layouts.get(1));")
            && page.contains("KeyHeatmapLayout.soft(styles.twentySix(),")
            && page.contains("KeyHeatmapLayout.nine(styles.nine(), languageLeft)")
            && page.contains("TouchKeyStyle.languageLeft(preferences.opt(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE))"),
            "the heatmap draws the user's bottom-row styles, number row and 中/英 side");
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
