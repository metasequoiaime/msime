import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardActionRow.DesignSlot;
import app.msime.android.KeyboardLayout;
import app.msime.android.KeyboardScheme;
import app.msime.android.TouchKeyStyle;
import app.msime.android.TouchKeyStyle.Row;
import app.msime.android.home.KeyLayoutStylesPolicy;
import app.msime.android.home.KeyLayoutStylesPolicy.Key;
import app.msime.android.home.KeyLayoutStylesPolicy.Kind;
import app.msime.android.home.KeyLayoutStylesPolicy.Preview;
import app.msime.android.home.KeyLayoutStylesPolicy.Tone;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 皮肤页那几块整键盘预览（AI 皮肤页、皮肤列表、社区皮肤卡片和详情，`KeyboardPreview`）画哪一副：按用户自己的偏好选键盘类型，底行跟着所选样式、顶部数字行、「符」与「123」互换和右侧大回车走，本机「交换语言键与数字键」打开时 123 与中/英对调，与键盘的 `KeyboardActionRow` 排的同一行。以前预览写死 `123 | ， | 空格 | 。 | 中 | ↵`，不认用户选的样式。读偏好和画图的那一半在 `KeyboardPreview` 里，要设备，这里跑不了。
 */
public final class KeyboardPreviewLayoutSmoke {
    private static final TouchKeyStyle.Groups ALL = new TouchKeyStyle.Groups(true, true, true);
    private static final String SIX = "twenty_six_key";
    private static final Map<DesignSlot, String> SLOT_IDS = Map.of(
        DesignSlot.LAYER, "SoftLayer", DesignSlot.LANGUAGE, "SoftLanguage", DesignSlot.SYMBOL_PANEL, "SoftSymbol",
        DesignSlot.SPACE, "Space", DesignSlot.COMMA, "Comma", DesignSlot.PERIOD, "Period",
        DesignSlot.EMOJI, "SoftEmoji", DesignSlot.RETURN, "Enter");

    public static void main(String[] arguments) {
        kind();
        standard();
        followsStyle();
        numberRowAndSymbolFirst();
        swapLanguageNumber();
        nineActionColumn();
        nineKeySidebar();
        languageLeft();
        caption();
        System.out.println("keyboard preview layout: passed");
    }

    private static void kind() {
        check(KeyLayoutStylesPolicy.previewKind(ALL, KeyboardScheme.QUANPIN, SIX, SIX) == Kind.TWENTY_SIX, "quanpin draws 26 keys");
        check(KeyLayoutStylesPolicy.previewKind(ALL, KeyboardScheme.QUANPIN_NINE_KEY, SIX, SIX) == Kind.NINE, "nine-key quanpin draws 9 keys");
        check(KeyLayoutStylesPolicy.previewKind(ALL, KeyboardScheme.NINE_KEY_SHUANGPIN, SIX, SIX) == Kind.NINE,
            "nine-key shuangpin draws 9 keys");
        check(KeyLayoutStylesPolicy.previewKind(ALL, KeyboardScheme.QUANPIN_FOURTEEN_KEY, SIX, SIX) == Kind.FOURTEEN,
            "14-key quanpin draws 14/18 keys");
        check(KeyLayoutStylesPolicy.previewKind(ALL, KeyboardScheme.XIAOHE, "eighteen_key", SIX) == Kind.FOURTEEN,
            "shuangpin on 18 keys draws 14/18 keys");
        check(KeyLayoutStylesPolicy.previewKind(ALL, null, SIX, SIX) == Kind.TWENTY_SIX, "an unknown scheme draws 26 keys");
        check(KeyLayoutStylesPolicy.previewKind(new TouchKeyStyle.Groups(false, false, false), KeyboardScheme.QUANPIN_NINE_KEY, SIX, SIX)
            == Kind.TWENTY_SIX, "an edition with no styled kinds still draws 26 keys");
        Preview eighteen = KeyLayoutStylesPolicy.preview(ALL, KeyboardScheme.QUANPIN_EIGHTEEN_KEY, SIX, SIX,
            TouchKeyStyle.Styles.DEFAULT, false, false, false, List.of());
        check(eighteen.kind() == Kind.FOURTEEN && eighteen.eighteen(), "quanpin 18 draws the 18-key grid");
        check(faces(eighteen.rows().get(0)).equals(List.of("，", "Q", "WE", "RT", "Y", "U", "IO", "P")), "18-key first row");
        Preview fourteen = KeyLayoutStylesPolicy.preview(ALL, KeyboardScheme.QUANPIN_FOURTEEN_KEY, SIX, SIX,
            TouchKeyStyle.Styles.DEFAULT, false, false, false, List.of());
        check(!fourteen.eighteen() && faces(fourteen.rows().get(0)).equals(List.of("，", "QW", "ER", "TY", "UI", "OP")),
            "14-key first row");
    }

    private static void standard() {
        List<List<Key>> rows = Preview.STANDARD.rows();
        check(rows.equals(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false)),
            "without preferences the preview is the standard 26-key keyboard");
        check(faces(last(rows)).equals(List.of("123", "符", "空格", "，", "中", "↵")), "standard 26-key bottom row");
        check(!faces(last(rows)).equals(List.of("123", "，", "空格", "。", "中", "↵")), "the old hard-coded bottom row is gone");
        check(faces(rows.get(1)).size() == 9 && faces(rows.get(1)).get(0).equals("A"), "second letter row, Chinese-mode faces");
    }

    private static void followsStyle() {
        TouchKeyStyle.Styles styles = new TouchKeyStyle.Styles(Row.COMMA_PERIOD, Row.LONG_SPACE, Row.EMOJI);
        Preview preview = new Preview(Kind.TWENTY_SIX, styles, false, false, false, false);
        check(faces(last(preview.rows())).equals(List.of("123", "符", "，", "空格", "。", "中", "↵")), "26 keys follow 逗号句号");
        check(faces(last(preview.with(Kind.FOURTEEN).rows())).equals(List.of("123", "，", "空格", "中", "↵")),
            "14/18 keys follow 长空格");
        check(faces(last(preview.with(Kind.NINE).rows())).equals(List.of("符", "123", "😀", "空格", "中", "↵")),
            "9 keys follow 表情键");
        // 每一类、每种样式的底行都与键盘排的那一行逐键相同。
        for (Kind kind : Kind.values()) {
            for (Row style : kind.rows) {
                if (style == Row.TALL_RETURN) continue;
                Preview chosen = new Preview(kind, kind.with(TouchKeyStyle.Styles.DEFAULT, style), false, false, false, false);
                sameEntries(last(chosen.rows()), KeyboardActionRow.designEntries(touchLayout(kind), false, false, false, false,
                    style, false, TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT), kind + " " + style);
            }
        }
        Preview tall = new Preview(Kind.NINE, new TouchKeyStyle.Styles(Row.STANDARD, Row.STANDARD, Row.TALL_RETURN),
            false, false, false, false);
        List<List<Key>> rows = tall.rows();
        Key enter = last(rows.get(2));
        check(enter.id().equals("Enter") && enter.rows() == 2, "右侧大回车 spans the last two rows");
        check(faces(last(rows)).subList(0, 4).equals(List.of("符", "123", "空格", "中")) && last(last(rows)).rows() == 0,
            "右侧大回车 puts 符 under the rail and covers the bottom-right cell");
        // 某一类不提供的样式（26 键没有右侧大回车）按标准画。
        Preview unoffered = new Preview(Kind.TWENTY_SIX, new TouchKeyStyle.Styles(Row.STANDARD, Row.STANDARD, Row.STANDARD),
            false, false, false, false);
        check(unoffered.rows().equals(Preview.STANDARD.rows()), "standard styles draw the standard keyboard");
    }

    private static void numberRowAndSymbolFirst() {
        List<String> digits = List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0");
        Preview numbers = new Preview(Kind.TWENTY_SIX, TouchKeyStyle.Styles.DEFAULT, true, false, false, false);
        check(numbers.rows().size() == 5 && faces(numbers.rows().get(0)).equals(digits), "26 keys draw the number row");
        check(numbers.with(Kind.FOURTEEN).rows().size() == 5, "14/18 keys draw the number row");
        check(numbers.with(Kind.NINE).rows().size() == 4, "9 keys never draw the number row");
        Preview symbolFirst = new Preview(Kind.TWENTY_SIX, TouchKeyStyle.Styles.DEFAULT, false, false, true, false);
        check(faces(last(symbolFirst.rows())).equals(List.of("符", "123", "空格", "，", "中", "↵")), "符 goes before 123");
        check(faces(last(symbolFirst.with(Kind.NINE).rows())).equals(faces(last(Preview.STANDARD.with(Kind.NINE).rows()))),
            "the 26-key swap leaves 9 keys alone");
    }

    private static void swapLanguageNumber() {
        Preview swapped = new Preview(Kind.TWENTY_SIX, TouchKeyStyle.Styles.DEFAULT, false, false, false, true);
        check(faces(last(swapped.rows())).equals(List.of("中", "符", "空格", "，", "123", "↵")), "26 keys swap 123 and 中");
        check(faces(last(new Preview(Kind.TWENTY_SIX, TouchKeyStyle.Styles.DEFAULT, false, false, true, true).rows()))
            .equals(List.of("符", "中", "空格", "，", "123", "↵")), "符 moves first, then 123 and 中 swap");
        check(faces(last(swapped.with(Kind.NINE).rows())).equals(List.of("符", "中", "空格", "123", "↵")), "9 keys swap 123 and 中");
        // 与键盘按同一项设置排的底行逐键相同，宽度跟着键走。
        for (Kind kind : Kind.values()) {
            for (Row style : kind.rows) {
                if (style == Row.TALL_RETURN) continue;
                for (boolean first : new boolean[] {false, true}) {
                    Preview chosen = new Preview(kind, kind.with(TouchKeyStyle.Styles.DEFAULT, style), false, false, first, true);
                    sameEntries(last(chosen.rows()), KeyboardActionRow.designEntries(touchLayout(kind), false, true, false, false,
                        style, kind == Kind.TWENTY_SIX && first, TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT),
                        "swapped " + kind + " " + style + (first ? " symbol first" : ""));
                }
            }
        }
        List<Key> tall = last(new Preview(Kind.NINE, new TouchKeyStyle.Styles(Row.STANDARD, Row.STANDARD, Row.TALL_RETURN),
            false, false, false, true).rows());
        List<String> ids = new ArrayList<>();
        for (KeyboardActionRow.DesignEntry entry : KeyboardActionRow.tallReturnEntries(
                KeyboardActionRow.tallReturnGridShare(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT), false, true, false))
            ids.add(SLOT_IDS.get(entry.slot()));
        List<String> drawn = new ArrayList<>();
        for (Key key : tall.subList(1, tall.size() - 1)) drawn.add(key.id());
        check(drawn.equals(ids), "右侧大回车 swaps 123 and 中 under the grid like the keyboard");
    }

    private static void nineActionColumn() {
        List<List<Key>> nine = Preview.STANDARD.with(Kind.NINE).rows();
        for (int index = 0; index < 3; index++) {
            check(last(nine.get(index)).tone() == Tone.FUNCTION, "nine-key right column is a function key on row " + index);
            check(nine.get(index).get(0).tone() == Tone.CHARACTER, "nine-key symbol rail stays a keycap on row " + index);
        }
        check(last(nine.get(2)).face().equals("！"), "the right column ends with ！");
        List<List<Key>> tall = new Preview(Kind.NINE, new TouchKeyStyle.Styles(Row.STANDARD, Row.STANDARD, Row.TALL_RETURN),
            false, false, false, false).rows();
        check(last(tall.get(2)).tone() == Tone.RETURN, "the tall return keeps the accent");
    }

    /** 九键左侧符号换成用户的表时，皮肤页这些预览与「按键布局」页画同一屏：第一行的左列是跨三行的一整条，里面是表的一屏（最多 NineKeySidebarPolicy.VISIBLE_ROWS 项）；出厂表照热力图逐行画三格。 */
    private static void nineKeySidebar() {
        Preview custom = KeyLayoutStylesPolicy.preview(ALL, KeyboardScheme.QUANPIN_NINE_KEY, SIX, SIX,
            TouchKeyStyle.Styles.DEFAULT, false, false, false, List.of("@", "#", "~", "%"));
        List<List<Key>> rows = custom.rows();
        Key rail = rows.get(0).get(0);
        check(rail.rows() == 3 && rail.stack().equals(List.of("@", "#", "~", "%")), "a custom nine-key sidebar draws one screen of it");
        check(rows.get(1).get(0).rows() == 0 && rows.get(2).get(0).rows() == 0, "the rail covers the next two rows");
        check(custom.with(Kind.TWENTY_SIX).with(Kind.NINE).equals(custom), "switching kinds keeps the sidebar table");
        check(Preview.STANDARD.with(Kind.NINE).rows().get(0).get(0).stack().isEmpty(), "the factory sidebar stays three keys");
    }

    /** 「中/英键放在空格左边」（#10228）打开时皮肤页这些预览与键盘排同一行：三类底行都先把中/英挪到空格左边，本机交换在最后一步做；右侧大回车是九宫格下方那一段；九键左侧符号换成用户的表时左列照样画那一屏，底行不受影响。 */
    private static void languageLeft() {
        Preview left = KeyLayoutStylesPolicy.preview(ALL, KeyboardScheme.QUANPIN, SIX, SIX, TouchKeyStyle.Styles.DEFAULT,
            false, false, false, List.of(), true);
        check(left.languageLeft() && faces(last(left.rows())).equals(List.of("123", "符", "中", "空格", "，", "↵")),
            "26 keys put 中 left of the space");
        check(faces(last(left.with(Kind.NINE).rows())).equals(List.of("符", "123", "中", "空格", "↵")),
            "9 keys put 中 left of the space");
        check(left.with(Kind.TWENTY_SIX).with(Kind.NINE).with(Kind.TWENTY_SIX).equals(left), "switching kinds keeps the side");
        for (Kind kind : Kind.values()) {
            for (Row style : kind.rows) {
                if (style == Row.TALL_RETURN) continue;
                for (boolean swap : new boolean[] {false, true}) {
                    Preview chosen = new Preview(kind, kind.with(TouchKeyStyle.Styles.DEFAULT, style), false, false, false, swap,
                        List.of(), true);
                    sameEntries(last(chosen.rows()), KeyboardActionRow.designEntries(touchLayout(kind), false, swap, false, false,
                        style, false, TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, true),
                        "language left " + kind + " " + style + (swap ? " swapped" : ""));
                }
            }
        }
        List<Key> tall = last(new Preview(Kind.NINE, new TouchKeyStyle.Styles(Row.STANDARD, Row.STANDARD, Row.TALL_RETURN),
            false, false, false, false, List.of(), true).rows());
        check(faces(tall).subList(1, 4).equals(List.of("123", "中", "空格")), "the tall-return strip puts 中 before the space");
        Preview custom = new Preview(Kind.NINE, TouchKeyStyle.Styles.DEFAULT, false, false, false, false,
            List.of("@", "#", "~"), true);
        List<List<Key>> rows = custom.rows();
        check(rows.get(0).get(0).stack().equals(List.of("@", "#", "~")), "a custom sidebar still draws with 中 on the left");
        check(faces(last(rows)).equals(List.of("符", "123", "中", "空格", "↵")), "the custom sidebar leaves the bottom row alone");
    }

    private static void caption() {
        check(Preview.STANDARD.caption().equals("26 键"), "26-key caption");
        check(Preview.STANDARD.with(Kind.NINE).caption().equals("九键"), "nine-key caption");
        check(Preview.STANDARD.with(Kind.FOURTEEN).caption().equals("14 键"), "14-key caption");
        check(new Preview(Kind.FOURTEEN, TouchKeyStyle.Styles.DEFAULT, false, true, false, false).caption().equals("18 键"),
            "18-key caption");
        Preview preview = new Preview(Kind.TWENTY_SIX, new TouchKeyStyle.Styles(Row.EMOJI, Row.STANDARD, Row.STANDARD),
            true, true, true, true);
        check(preview.with(Kind.NINE).equals(new Preview(Kind.NINE, preview.styles(), true, true, true, true)),
            "switching the kind keeps every other preference");
    }

    private static int touchLayout(Kind kind) {
        return switch (kind) {
            case TWENTY_SIX -> KeyboardLayout.STANDARD_TOUCH_LAYOUT;
            case FOURTEEN -> KeyboardLayout.FOURTEEN_KEY_LAYOUT;
            case NINE -> KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        };
    }

    private static <T> T last(List<T> items) {
        return items.get(items.size() - 1);
    }

    private static List<String> faces(List<Key> row) {
        List<String> faces = new ArrayList<>(row.size());
        for (Key key : row) faces.add(key.face());
        return faces;
    }

    private static void sameEntries(List<Key> row, List<KeyboardActionRow.DesignEntry> entries, String message) {
        check(row.size() == entries.size(), message + ": key count");
        for (int index = 0; index < row.size(); index++) {
            check(row.get(index).id().equals(SLOT_IDS.get(entries.get(index).slot())), message + ": key " + index);
            check(Math.abs(row.get(index).weight() - entries.get(index).weight()) < 1e-4f, message + ": weight " + index);
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
